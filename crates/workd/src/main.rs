use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use clap::Parser;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
    time::timeout,
};
use tracing::{error, info, warn};
use workctl_core::{
    Artifact, ContextManifest, DevshellManifest, DevshellMode, HarnessKind, HealthResponse,
    OrganizationId, OutputId, OutputKind, PreparedRepo, RecordId, RecordSubject, RepoSpec,
    SubmitTaskRequest, SubmitTaskResponse, Task, TaskId, TaskOutput, TaskRecord, TaskRecordKind,
    TaskSpec, TaskState, product_sentence,
};

const DEFAULT_BIND: &str = "127.0.0.1:7878";
const WORKER_TICK_MS: u64 = 500;
const OPENCODE_TIMEOUT_SECS: u64 = 900;

#[derive(Debug, Parser)]
#[command(author, version, about = "workctl control-plane daemon")]
struct Args {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Debug, clap::Subcommand)]
enum CommandKind {
    Serve(ServeArgs),
}

#[derive(Debug, Parser, Clone)]
struct ServeArgs {
    #[arg(long, env = "WORKD_BIND", default_value = DEFAULT_BIND)]
    bind: SocketAddr,
    #[arg(long, env = "WORKD_STATE_DIR")]
    state_dir: Option<PathBuf>,
    #[arg(long, env = "WORKD_WORKER_INTERVAL_MS", default_value_t = WORKER_TICK_MS)]
    worker_interval_ms: u64,
    #[arg(long, env = "WORKD_DISABLE_WORKER")]
    disable_worker: bool,
}

#[derive(Clone)]
struct AppState {
    store: Arc<dyn ControlStore>,
    claims: Claims,
}

type Claims = Arc<Mutex<HashMap<TaskId, ()>>>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    match Args::parse().command {
        CommandKind::Serve(args) => serve(args).await,
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    let state_dir = args.state_dir.unwrap_or_else(default_state_dir);
    fs::create_dir_all(&state_dir).await?;
    let store: Arc<dyn ControlStore> = Arc::new(SqliteStore::new(state_dir).await?);
    let state = AppState {
        store,
        claims: Arc::new(Mutex::new(HashMap::new())),
    };

    if !args.disable_worker {
        let worker_state = state.clone();
        tokio::spawn(async move {
            worker_loop(worker_state, Duration::from_millis(args.worker_interval_ms)).await;
        });
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/tasks", post(submit_task).get(list_tasks))
        .route("/tasks/{task_id}", get(get_task))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    info!(bind = %args.bind, "workd listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        product: product_sentence().to_string(),
    })
}

async fn submit_task(
    State(state): State<AppState>,
    Json(request): Json<SubmitTaskRequest>,
) -> Result<Json<SubmitTaskResponse>, AppError> {
    if request.title.trim().is_empty() {
        return Err(AppError::bad_request("title is required"));
    }
    if request.intent.trim().is_empty() {
        return Err(AppError::bad_request("intent is required"));
    }
    if request.repos.is_empty() {
        return Err(AppError::bad_request("at least one repo is required"));
    }

    let now = now_ms();
    let title = request.title;
    let intent = request.intent;
    let task = Task {
        id: TaskId::new(),
        organization_id: OrganizationId("local".into()),
        state: TaskState::Created,
        title: title.clone(),
        intent: intent.clone(),
        spec: TaskSpec {
            repos: request.repos,
            harness: request.harness,
            executor: request.executor,
        },
        workspace_path: None,
        summary: None,
        artifacts: Vec::new(),
        outputs: Vec::new(),
        records: vec![TaskRecord {
            id: RecordId::new(),
            kind: TaskRecordKind::InputReceived,
            subject: RecordSubject::Task,
            body: json!({"title": title, "intent": intent}),
            created_at_ms: now,
        }],
        last_error: None,
        created_at_ms: now,
        updated_at_ms: now,
    };
    state.store.save(&task).await?;
    Ok(Json(SubmitTaskResponse {
        task_id: task.id,
        state: TaskState::Created,
    }))
}

async fn get_task(
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<Task>, AppError> {
    state
        .store
        .load(&TaskId(task_id.clone()))
        .await?
        .map(Json)
        .ok_or_else(|| AppError::not_found(format!("task {task_id}")))
}

async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<Task>>, AppError> {
    Ok(Json(state.store.list().await?))
}

async fn worker_loop(state: AppState, interval: Duration) {
    loop {
        if let Err(err) = worker_tick(state.store.as_ref(), &state.claims).await {
            error!(?err, "worker tick failed");
        }
        tokio::time::sleep(interval).await;
    }
}

async fn worker_tick(store: &dyn ControlStore, claims: &Claims) -> Result<()> {
    let tasks = store.list().await?;
    for task in tasks {
        if task.state == TaskState::Created && try_claim(claims, &task.id).await {
            let task_id = task.id.clone();
            process_task(store, task).await;
            release_claim(claims, &task_id).await;
            return Ok(());
        }
    }
    Ok(())
}

async fn process_task(store: &dyn ControlStore, mut task: Task) {
    let result = async {
        mark(store, &mut task, TaskState::ContextRequested).await?;
        let prepared = prepare_context(store, &mut task).await?;
        mark(store, &mut task, TaskState::ContextReady).await?;
        mark(store, &mut task, TaskState::Running).await?;
        let summary = run_harness(&task, &prepared).await?;
        let summary_path = prepared.artifact_dir.join("summary.md");
        fs::write(&summary_path, &summary).await?;
        task.summary = Some(summary.clone());
        let summary_artifact_path = summary_path.display().to_string();
        task.artifacts.push(Artifact {
            kind: "summary".into(),
            path: summary_artifact_path.clone(),
        });
        let now = now_ms();
        let output = TaskOutput {
            id: OutputId::new(),
            kind: OutputKind::Summary,
            title: format!("Summary for {}", task.title),
            body: summary,
            source_artifacts: vec![summary_artifact_path],
            created_at_ms: now,
            updated_at_ms: now,
        };
        task.records.push(TaskRecord {
            id: RecordId::new(),
            kind: TaskRecordKind::OutputCreated,
            subject: RecordSubject::Output(output.id.clone()),
            body: json!({
                "kind": output.kind,
                "title": output.title,
                "source_artifacts": output.source_artifacts,
            }),
            created_at_ms: now,
        });
        task.outputs.push(output);
        task.state = TaskState::Done;
        task.updated_at_ms = now;
        store.save(&task).await?;
        Result::<()>::Ok(())
    }
    .await;

    if let Err(err) = result {
        error!(task_id = %task.id, ?err, "task failed");
        task.state = TaskState::Failed;
        task.last_error = Some(format!("{err:#}"));
        task.updated_at_ms = now_ms();
        if let Err(save_err) = store.save(&task).await {
            error!(?save_err, "failed to persist failed task");
        }
    }
}

async fn mark(store: &dyn ControlStore, task: &mut Task, state: TaskState) -> Result<()> {
    task.state = state;
    task.updated_at_ms = now_ms();
    store.save(task).await
}

struct PreparedContext {
    primary_repo: PathBuf,
    artifact_dir: PathBuf,
    prompt_path: PathBuf,
    manifest: ContextManifest,
}

async fn prepare_context(store: &dyn ControlStore, task: &mut Task) -> Result<PreparedContext> {
    let workspace = store.workspace_root().join(task.id.to_string());
    let repos_dir = workspace.join("repos");
    let artifact_dir = workspace.join("artifacts");
    let prompt_dir = workspace.join("prompts");
    for dir in [
        &repos_dir,
        &artifact_dir,
        &prompt_dir,
        &workspace.join("home"),
        &workspace.join("tmp"),
        &workspace.join("xdg-cache"),
        &workspace.join("xdg-config"),
        &workspace.join("xdg-data"),
    ] {
        fs::create_dir_all(dir).await?;
    }

    let mut prepared_repos = Vec::new();
    for repo in &task.spec.repos {
        let destination = repos_dir.join(&repo.name);
        clone_repo(repo, &destination).await?;
        prepared_repos.push(PreparedRepo {
            name: repo.name.clone(),
            url: repo.url.clone(),
            path: destination.display().to_string(),
            checkout: repo.checkout.clone(),
        });
    }
    let primary_repo = PathBuf::from(
        prepared_repos
            .first()
            .context("expected at least one prepared repo")?
            .path
            .clone(),
    );
    let prompt_path = prompt_dir.join("task.md");
    fs::write(&prompt_path, render_prompt(task)).await?;

    let devshell = devshell_manifest(&primary_repo, task.spec.harness.kind).await;
    let manifest = ContextManifest {
        task_id: task.id.clone(),
        executor: task.spec.executor.kind,
        harness: task.spec.harness.kind,
        workspace_path: workspace.display().to_string(),
        repos: prepared_repos,
        artifact_dir: artifact_dir.display().to_string(),
        prompt_path: prompt_path.display().to_string(),
        devshell,
    };
    let manifest_path = artifact_dir.join("context-manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).await?;

    task.workspace_path = Some(workspace.display().to_string());
    task.artifacts.push(Artifact {
        kind: "context-manifest".into(),
        path: manifest_path.display().to_string(),
    });
    task.updated_at_ms = now_ms();
    store.save(task).await?;

    Ok(PreparedContext {
        primary_repo,
        artifact_dir,
        prompt_path,
        manifest,
    })
}

async fn clone_repo(repo: &RepoSpec, destination: &Path) -> Result<()> {
    if fs::try_exists(destination).await? {
        return Ok(());
    }
    let mut command = Command::new("git");
    command.arg("clone").arg(&repo.url).arg(destination);
    run_command(&mut command)
        .await
        .context("git clone failed")?;

    if let Some(checkout) = &repo.checkout {
        let mut checkout_command = Command::new("git");
        checkout_command
            .arg("checkout")
            .arg(checkout)
            .current_dir(destination);
        run_command(&mut checkout_command)
            .await
            .context("git checkout failed")?;
    }
    Ok(())
}

async fn run_command(command: &mut Command) -> Result<()> {
    let output = command.output().await?;
    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "command exited with {status}: stdout={stdout}\nstderr={stderr}",
            status = output.status,
            stdout = String::from_utf8_lossy(&output.stdout),
            stderr = String::from_utf8_lossy(&output.stderr)
        )
    }
}

fn render_prompt(task: &Task) -> String {
    format!(
        "# Task: {title}\n\n{intent}\n\nSummarize the checked-out repository. Include purpose, structure, important commands, dependencies, and notable implementation details. Do not modify files.\n",
        title = task.title,
        intent = task.intent
    )
}

async fn devshell_manifest(primary_repo: &Path, harness: HarnessKind) -> DevshellManifest {
    let uses_nix = fs::try_exists(primary_repo.join("flake.nix"))
        .await
        .unwrap_or(false);
    let mut command = if uses_nix {
        vec![
            "nix".into(),
            "develop".into(),
            primary_repo.display().to_string(),
            "--command".into(),
        ]
    } else {
        Vec::new()
    };

    match harness {
        HarnessKind::OpencodeAcp => {
            command.extend(["opencode".into(), "acp".into(), "--cwd".into()]);
            command.push(primary_repo.display().to_string());
        }
        HarnessKind::FakeSummary => command.push("fake-summary".into()),
    }

    DevshellManifest {
        mode: if uses_nix {
            DevshellMode::NixDevelop
        } else {
            DevshellMode::DirectProcess
        },
        command,
    }
}

async fn run_harness(task: &Task, prepared: &PreparedContext) -> Result<String> {
    match task.spec.harness.kind {
        HarnessKind::FakeSummary => fake_summary(task, prepared).await,
        HarnessKind::OpencodeAcp => run_opencode_acp(task, prepared).await,
    }
}

async fn fake_summary(task: &Task, prepared: &PreparedContext) -> Result<String> {
    let entries = list_top_level_entries(&prepared.primary_repo).await?;
    Ok(format!(
        "# Summary for {title}\n\nRepository: `{repo}`\n\nIntent: {intent}\n\nTop-level entries:\n{entries}\n",
        title = task.title,
        repo = prepared.primary_repo.display(),
        intent = task.intent,
        entries = entries
            .into_iter()
            .map(|entry| format!("- {entry}"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

async fn list_top_level_entries(path: &Path) -> Result<Vec<String>> {
    let mut entries = fs::read_dir(path).await?;
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(names)
}

async fn run_opencode_acp(task: &Task, prepared: &PreparedContext) -> Result<String> {
    let log_path = prepared.artifact_dir.join("opencode-acp.ndjson");
    let mut log = fs::File::create(&log_path).await?;
    let (program, args) = harness_command(&prepared.manifest)?;
    let workspace = prepared
        .primary_repo
        .parent()
        .and_then(Path::parent)
        .context("prepared repo is not under workspace/repos")?;
    mount_opencode_state(workspace).await?;

    let mut child = Command::new(program)
        .args(args)
        .current_dir(&prepared.primary_repo)
        .env("HOME", workspace.join("home"))
        .env("TMPDIR", workspace.join("tmp"))
        .env("XDG_CACHE_HOME", workspace.join("xdg-cache"))
        .env("XDG_CONFIG_HOME", workspace.join("xdg-config"))
        .env("XDG_DATA_HOME", workspace.join("xdg-data"))
        .env("WORKCTL_TASK_ID", task.id.to_string())
        .env("WORKCTL_ARTIFACT_DIR", &prepared.artifact_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn opencode ACP")?;

    let stderr = child.stderr.take().context("missing ACP stderr")?;
    let stderr_path = prepared.artifact_dir.join("opencode-acp.stderr.log");
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut data = String::new();
        while let Ok(Some(line)) = reader.next_line().await {
            data.push_str(&line);
            data.push('\n');
        }
        if let Err(err) = fs::write(stderr_path, data).await {
            warn!(?err, "failed to write ACP stderr log");
        }
    });

    let mut stdin = child.stdin.take().context("missing ACP stdin")?;
    let stdout = child.stdout.take().context("missing ACP stdout")?;
    let mut reader = BufReader::new(stdout).lines();
    let mut next_id = 0_u64;

    let init_id = send_request(
        &mut stdin,
        &mut log,
        &mut next_id,
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientCapabilities": {},
            "clientInfo": {"name": "workctl", "title": "workctl", "version": env!("CARGO_PKG_VERSION")}
        }),
    )
    .await?;
    read_until_response(&mut reader, &mut stdin, &mut log, init_id)
        .await?
        .context("initialize returned no response")?;

    let session_id_req = send_request(
        &mut stdin,
        &mut log,
        &mut next_id,
        "session/new",
        json!({"cwd": prepared.primary_repo, "mcpServers": []}),
    )
    .await?;
    let session_response = read_until_response(&mut reader, &mut stdin, &mut log, session_id_req)
        .await?
        .context("session/new returned no response")?;
    let session_id = session_response
        .get("result")
        .and_then(|result| result.get("sessionId"))
        .and_then(Value::as_str)
        .context("session/new response did not include sessionId")?
        .to_string();

    let prompt = fs::read_to_string(&prepared.prompt_path).await?;
    let prompt_id = send_request(
        &mut stdin,
        &mut log,
        &mut next_id,
        "session/prompt",
        json!({"sessionId": session_id, "prompt": [{"type": "text", "text": prompt}]}),
    )
    .await?;

    let mut chunks = Vec::new();
    timeout(
        Duration::from_secs(OPENCODE_TIMEOUT_SECS),
        read_prompt_response(&mut reader, &mut stdin, &mut log, prompt_id, &mut chunks),
    )
    .await
    .map_err(|_| anyhow!("opencode ACP timed out"))??;

    let _ = child.start_kill();
    let summary = chunks.join("");
    if summary.trim().is_empty() {
        bail!("opencode ACP completed without agent text");
    }
    Ok(summary)
}

async fn mount_opencode_state(workspace: &Path) -> Result<()> {
    mount_host_dir(
        &host_xdg_dir("XDG_CONFIG_HOME", ".config").join("opencode"),
        &workspace.join("xdg-config/opencode"),
    )
    .await?;
    mount_host_dir(
        &host_xdg_dir("XDG_DATA_HOME", ".local/share").join("opencode"),
        &workspace.join("xdg-data/opencode"),
    )
    .await?;
    Ok(())
}

async fn mount_host_dir(source: &Path, destination: &Path) -> Result<()> {
    if !fs::try_exists(source).await? || fs::try_exists(destination).await? {
        return Ok(());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, destination)?;
    }
    #[cfg(not(unix))]
    {
        let _ = (source, destination);
        bail!("opencode state mounting is only implemented for unix platforms");
    }
    Ok(())
}

fn host_xdg_dir(variable: &str, fallback: &str) -> PathBuf {
    if let Some(value) = env::var_os(variable) {
        return PathBuf::from(value);
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(fallback)
}

fn harness_command(manifest: &ContextManifest) -> Result<(&str, Vec<&str>)> {
    let Some((program, args)) = manifest.devshell.command.split_first() else {
        bail!("empty harness command")
    };
    Ok((program.as_str(), args.iter().map(String::as_str).collect()))
}

async fn send_request(
    stdin: &mut tokio::process::ChildStdin,
    log: &mut fs::File,
    next_id: &mut u64,
    method: &str,
    params: Value,
) -> Result<u64> {
    let id = *next_id;
    *next_id += 1;
    let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    write_json_line(stdin, log, "client", &message).await?;
    Ok(id)
}

async fn read_prompt_response(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    stdin: &mut tokio::process::ChildStdin,
    log: &mut fs::File,
    target_id: u64,
    chunks: &mut Vec<String>,
) -> Result<Value> {
    loop {
        let Some(line) = reader.next_line().await? else {
            bail!("ACP process closed stdout before response {target_id}");
        };
        log_line(log, "agent", &line).await?;
        let message: Value = serde_json::from_str(&line)?;
        if is_agent_text_chunk(&message)
            && let Some(text) = message
                .pointer("/params/update/content/text")
                .and_then(Value::as_str)
        {
            chunks.push(text.to_string());
        }
        if let Some(response) = handle_message(stdin, log, &message, target_id).await? {
            return Ok(response);
        }
    }
}

async fn read_until_response(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    stdin: &mut tokio::process::ChildStdin,
    log: &mut fs::File,
    target_id: u64,
) -> Result<Option<Value>> {
    loop {
        let Some(line) = reader.next_line().await? else {
            return Ok(None);
        };
        log_line(log, "agent", &line).await?;
        let message: Value = serde_json::from_str(&line)?;
        if let Some(response) = handle_message(stdin, log, &message, target_id).await? {
            return Ok(Some(response));
        }
    }
}

async fn handle_message(
    stdin: &mut tokio::process::ChildStdin,
    log: &mut fs::File,
    message: &Value,
    target_id: u64,
) -> Result<Option<Value>> {
    if message.get("id").and_then(Value::as_u64) == Some(target_id)
        && (message.get("result").is_some() || message.get("error").is_some())
    {
        if let Some(error) = message.get("error") {
            bail!("ACP request {target_id} failed: {error}");
        }
        return Ok(Some(message.clone()));
    }

    if message.get("method").is_some() && message.get("id").is_some() {
        let response = callback_response(message);
        write_json_line(stdin, log, "client", &response).await?;
    }
    Ok(None)
}

fn callback_response(message: &Value) -> Value {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    match message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "session/request_permission" => {
            json!({"jsonrpc":"2.0", "id": id, "result": {"outcome": {"outcome": "cancelled"}}})
        }
        _ => {
            json!({"jsonrpc":"2.0", "id": id, "error": {"code": -32601, "message": "workctl ACP client does not implement this method"}})
        }
    }
}

fn is_agent_text_chunk(message: &Value) -> bool {
    message
        .pointer("/params/update/sessionUpdate")
        .and_then(Value::as_str)
        == Some("agent_message_chunk")
}

async fn write_json_line(
    stdin: &mut tokio::process::ChildStdin,
    log: &mut fs::File,
    direction: &str,
    message: &Value,
) -> Result<()> {
    let line = serde_json::to_string(message)?;
    log_line(log, direction, &line).await?;
    stdin.write_all(line.as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    Ok(())
}

async fn log_line(log: &mut fs::File, direction: &str, line: &str) -> Result<()> {
    log.write_all(format!("{direction} {line}\n").as_bytes())
        .await?;
    Ok(())
}

#[async_trait]
trait ControlStore: Send + Sync {
    fn workspace_root(&self) -> PathBuf;
    async fn save(&self, task: &Task) -> Result<()>;
    async fn load(&self, id: &TaskId) -> Result<Option<Task>>;
    async fn list(&self) -> Result<Vec<Task>>;
}

#[derive(Clone)]
struct SqliteStore {
    root: PathBuf,
    db: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    async fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("workspaces")).await?;
        let db_path = root.join("workd.sqlite3");
        let connection = Connection::open(db_path)?;
        migrate(&connection)?;
        Ok(Self {
            root,
            db: Arc::new(Mutex::new(connection)),
        })
    }
}

#[async_trait]
impl ControlStore for SqliteStore {
    fn workspace_root(&self) -> PathBuf {
        self.root.join("workspaces")
    }

    async fn save(&self, task: &Task) -> Result<()> {
        let mut db = self.db.lock().await;
        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO tasks (
                id, organization_id, state, title, intent, spec_json, workspace_path,
                summary, last_error, created_at_ms, updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
                organization_id = excluded.organization_id,
                state = excluded.state,
                title = excluded.title,
                intent = excluded.intent,
                spec_json = excluded.spec_json,
                workspace_path = excluded.workspace_path,
                summary = excluded.summary,
                last_error = excluded.last_error,
                created_at_ms = excluded.created_at_ms,
                updated_at_ms = excluded.updated_at_ms",
            params![
                task.id.0,
                task.organization_id.0,
                enum_to_db(&task.state)?,
                task.title,
                task.intent,
                serde_json::to_string(&task.spec)?,
                task.workspace_path,
                task.summary,
                task.last_error,
                ms_to_i64(task.created_at_ms)?,
                ms_to_i64(task.updated_at_ms)?,
            ],
        )?;

        tx.execute(
            "DELETE FROM artifacts WHERE task_id = ?1",
            params![task.id.0],
        )?;
        for (position, artifact) in task.artifacts.iter().enumerate() {
            tx.execute(
                "INSERT INTO artifacts (task_id, position, kind, path) VALUES (?1, ?2, ?3, ?4)",
                params![task.id.0, position as i64, artifact.kind, artifact.path],
            )?;
        }

        tx.execute("DELETE FROM outputs WHERE task_id = ?1", params![task.id.0])?;
        for (position, output) in task.outputs.iter().enumerate() {
            tx.execute(
                "INSERT INTO outputs (
                    id, task_id, position, kind, title, body, source_artifacts_json,
                    created_at_ms, updated_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    output.id.0,
                    task.id.0,
                    position as i64,
                    enum_to_db(&output.kind)?,
                    output.title,
                    output.body,
                    serde_json::to_string(&output.source_artifacts)?,
                    ms_to_i64(output.created_at_ms)?,
                    ms_to_i64(output.updated_at_ms)?,
                ],
            )?;
        }

        tx.execute("DELETE FROM records WHERE task_id = ?1", params![task.id.0])?;
        for (position, record) in task.records.iter().enumerate() {
            tx.execute(
                "INSERT INTO records (
                    id, task_id, position, kind, subject_json, body_json, created_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    record.id.0,
                    task.id.0,
                    position as i64,
                    enum_to_db(&record.kind)?,
                    serde_json::to_string(&record.subject)?,
                    serde_json::to_string(&record.body)?,
                    ms_to_i64(record.created_at_ms)?,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    async fn load(&self, id: &TaskId) -> Result<Option<Task>> {
        let db = self.db.lock().await;
        load_task(&db, id)
    }

    async fn list(&self) -> Result<Vec<Task>> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare("SELECT id FROM tasks ORDER BY created_at_ms")?;
        let ids = stmt
            .query_map([], |row| Ok(TaskId(row.get::<_, String>(0)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut tasks = Vec::new();
        for id in ids {
            if let Some(task) = load_task(&db, &id)? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }
}

async fn try_claim(claims: &Claims, id: &TaskId) -> bool {
    let mut claims = claims.lock().await;
    if claims.contains_key(id) {
        false
    } else {
        claims.insert(id.clone(), ());
        true
    }
}

async fn release_claim(claims: &Claims, id: &TaskId) {
    claims.lock().await.remove(id);
}

fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch(
        "PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            organization_id TEXT NOT NULL,
            state TEXT NOT NULL,
            title TEXT NOT NULL,
            intent TEXT NOT NULL,
            spec_json TEXT NOT NULL,
            workspace_path TEXT,
            summary TEXT,
            last_error TEXT,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS artifacts (
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            kind TEXT NOT NULL,
            path TEXT NOT NULL,
            PRIMARY KEY (task_id, position)
        );
        CREATE TABLE IF NOT EXISTS outputs (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            source_artifacts_json TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_outputs_task_position ON outputs(task_id, position);
        CREATE TABLE IF NOT EXISTS records (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            kind TEXT NOT NULL,
            subject_json TEXT NOT NULL,
            body_json TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_records_task_position ON records(task_id, position);",
    )?;
    Ok(())
}

fn load_task(db: &Connection, id: &TaskId) -> Result<Option<Task>> {
    let Some(mut task) = db
        .query_row(
            "SELECT organization_id, state, title, intent, spec_json, workspace_path,
                summary, last_error, created_at_ms, updated_at_ms
             FROM tasks WHERE id = ?1",
            params![id.0],
            |row| {
                let state: String = row.get(1)?;
                let spec_json: String = row.get(4)?;
                Ok(Task {
                    id: id.clone(),
                    organization_id: OrganizationId(row.get(0)?),
                    state: db_enum(&state).map_err(rusqlite::Error::ToSqlConversionFailure)?,
                    title: row.get(2)?,
                    intent: row.get(3)?,
                    spec: serde_json::from_str(&spec_json)
                        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
                    workspace_path: row.get(5)?,
                    summary: row.get(6)?,
                    artifacts: Vec::new(),
                    outputs: Vec::new(),
                    records: Vec::new(),
                    last_error: row.get(7)?,
                    created_at_ms: i64_to_ms(row.get(8)?),
                    updated_at_ms: i64_to_ms(row.get(9)?),
                })
            },
        )
        .optional()?
    else {
        return Ok(None);
    };

    task.artifacts = load_artifacts(db, id)?;
    task.outputs = load_outputs(db, id)?;
    task.records = load_records(db, id)?;
    Ok(Some(task))
}

fn load_artifacts(db: &Connection, id: &TaskId) -> Result<Vec<Artifact>> {
    let mut stmt =
        db.prepare("SELECT kind, path FROM artifacts WHERE task_id = ?1 ORDER BY position")?;
    Ok(stmt
        .query_map(params![id.0], |row| {
            Ok(Artifact {
                kind: row.get(0)?,
                path: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

fn load_outputs(db: &Connection, id: &TaskId) -> Result<Vec<TaskOutput>> {
    let mut stmt = db.prepare(
        "SELECT id, kind, title, body, source_artifacts_json, created_at_ms, updated_at_ms
         FROM outputs WHERE task_id = ?1 ORDER BY position",
    )?;
    Ok(stmt
        .query_map(params![id.0], |row| {
            let kind: String = row.get(1)?;
            let source_artifacts_json: String = row.get(4)?;
            Ok(TaskOutput {
                id: OutputId(row.get(0)?),
                kind: db_enum(&kind).map_err(rusqlite::Error::ToSqlConversionFailure)?,
                title: row.get(2)?,
                body: row.get(3)?,
                source_artifacts: serde_json::from_str(&source_artifacts_json)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
                created_at_ms: i64_to_ms(row.get(5)?),
                updated_at_ms: i64_to_ms(row.get(6)?),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

fn load_records(db: &Connection, id: &TaskId) -> Result<Vec<TaskRecord>> {
    let mut stmt = db.prepare(
        "SELECT id, kind, subject_json, body_json, created_at_ms
         FROM records WHERE task_id = ?1 ORDER BY position",
    )?;
    Ok(stmt
        .query_map(params![id.0], |row| {
            let kind: String = row.get(1)?;
            let subject_json: String = row.get(2)?;
            let body_json: String = row.get(3)?;
            Ok(TaskRecord {
                id: RecordId(row.get(0)?),
                kind: db_enum(&kind).map_err(rusqlite::Error::ToSqlConversionFailure)?,
                subject: serde_json::from_str(&subject_json)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
                body: serde_json::from_str(&body_json)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?,
                created_at_ms: i64_to_ms(row.get(4)?),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

fn enum_to_db<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_string())
}

fn db_enum<T: serde::de::DeserializeOwned>(
    value: &str,
) -> std::result::Result<T, Box<dyn std::error::Error + Send + Sync>> {
    Ok(serde_json::from_str(&format!("\"{value}\""))?)
}

fn ms_to_i64(value: u128) -> Result<i64> {
    i64::try_from(value).context("timestamp does not fit in sqlite INTEGER")
}

fn i64_to_ms(value: i64) -> u128 {
    u128::try_from(value).unwrap_or_default()
}

#[derive(Debug)]
struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{value:#}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error": {"message": self.message}})),
        )
            .into_response()
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn default_state_dir() -> PathBuf {
    if let Some(value) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(value).join("workctl");
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/share/workctl")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_summary_lists_repository_entries() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("README.md"), "hello")
            .await
            .unwrap();
        let prepared = PreparedContext {
            primary_repo: temp.path().to_path_buf(),
            artifact_dir: temp.path().to_path_buf(),
            prompt_path: temp.path().join("prompt.md"),
            manifest: ContextManifest {
                task_id: TaskId("task_test".into()),
                executor: workctl_core::ExecutorKind::LocalDevshell,
                harness: HarnessKind::FakeSummary,
                workspace_path: temp.path().display().to_string(),
                repos: Vec::new(),
                artifact_dir: temp.path().display().to_string(),
                prompt_path: temp.path().join("prompt.md").display().to_string(),
                devshell: DevshellManifest {
                    mode: DevshellMode::DirectProcess,
                    command: vec!["fake-summary".into()],
                },
            },
        };
        let task = Task {
            id: TaskId("task_test".into()),
            organization_id: OrganizationId("local".into()),
            state: TaskState::Running,
            title: "test".into(),
            intent: "summarize".into(),
            spec: TaskSpec {
                repos: vec![],
                harness: workctl_core::HarnessSpec {
                    kind: HarnessKind::FakeSummary,
                },
                executor: workctl_core::ExecutorSpec::default(),
            },
            workspace_path: None,
            summary: None,
            artifacts: Vec::new(),
            outputs: Vec::new(),
            records: Vec::new(),
            last_error: None,
            created_at_ms: 0,
            updated_at_ms: 0,
        };

        let summary = fake_summary(&task, &prepared).await.unwrap();
        assert!(summary.contains("README.md"));
    }
}
