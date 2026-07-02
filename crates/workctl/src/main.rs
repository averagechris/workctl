use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use serde_json::Value;
use std::io::Write as _;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use workctl_core::{
    ExecutorKind, ExecutorSpec, HarnessKind, HarnessSpec, HealthResponse, RepoSpec,
    SubmitTaskRequest, SubmitTaskResponse, Task, TaskRecord, TaskRecordKind, product_sentence,
};

const DEFAULT_SERVER: &str = "http://127.0.0.1:7878";

#[derive(Debug, Parser)]
#[command(author, version, about = product_sentence())]
struct Cli {
    #[arg(long, env = "WORKD_URL", global = true, default_value = DEFAULT_SERVER)]
    server: String,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    Health,
    Submit(SubmitArgs),
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}

#[derive(Debug, clap::Subcommand)]
enum TaskCommand {
    Submit(SubmitArgs),
    Get {
        task_id: String,
    },
    List,
    Watch {
        task_id: String,
        #[arg(long, default_value_t = 1800)]
        timeout_secs: u64,
    },
    Review {
        task_id: String,
    },
    Outputs {
        task_id: String,
    },
    Records {
        task_id: String,
    },
    Artifacts {
        task_id: String,
    },
}

#[derive(Debug, Parser)]
struct SubmitArgs {
    #[arg(long)]
    title: String,
    #[arg(long)]
    intent: String,
    #[arg(long = "repo")]
    repos: Vec<String>,
    #[arg(long = "repo-name")]
    repo_names: Vec<String>,
    #[arg(long, value_enum, default_value_t = HarnessArg::OpencodeAcp)]
    harness: HarnessArg,
    #[arg(long, value_enum, default_value_t = ExecutorArg::LocalDevshell)]
    executor: ExecutorArg,
    #[arg(long)]
    wait: bool,
    #[arg(long)]
    watch: bool,
    #[arg(long, default_value_t = 1800)]
    timeout_secs: u64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum HarnessArg {
    OpencodeAcp,
    FakeSummary,
}

impl From<HarnessArg> for HarnessKind {
    fn from(value: HarnessArg) -> Self {
        match value {
            HarnessArg::OpencodeAcp => Self::OpencodeAcp,
            HarnessArg::FakeSummary => Self::FakeSummary,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExecutorArg {
    LocalDevshell,
}

impl From<ExecutorArg> for ExecutorKind {
    fn from(value: ExecutorArg) -> Self {
        match value {
            ExecutorArg::LocalDevshell => Self::LocalDevshell,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::new(cli.server.clone())?;
    match cli.command {
        Command::Health => {
            let health = client.health().await?;
            print_value(cli.json, &health)?;
        }
        Command::Submit(args)
        | Command::Task {
            command: TaskCommand::Submit(args),
        } => {
            let response = client.submit(&submit_request(&args)?).await?;
            if args.watch {
                if !cli.json {
                    println!("task {} {}", response.task_id, response.state);
                }
                let task = watch_task(
                    &client,
                    cli.json,
                    &response.task_id.to_string(),
                    args.timeout_secs,
                )
                .await?;
                print_watch_final(cli.json, &task)?;
            } else if args.wait {
                let task =
                    wait_for_task(&client, response.task_id.to_string(), args.timeout_secs).await?;
                print_task(cli.json, &task)?;
            } else {
                print_value(cli.json, &response)?;
                if !cli.json {
                    println!("task {} {}", response.task_id, response.state);
                }
            }
        }
        Command::Task {
            command:
                TaskCommand::Watch {
                    task_id,
                    timeout_secs,
                },
        } => {
            let task = watch_task(&client, cli.json, &task_id, timeout_secs).await?;
            print_watch_final(cli.json, &task)?;
        }
        Command::Task {
            command: TaskCommand::Review { task_id },
        } => {
            let task = client.get(&task_id).await?;
            review_task(&client, cli.json, &task).await?;
        }
        Command::Task {
            command: TaskCommand::Get { task_id },
        } => {
            let task = client.get(&task_id).await?;
            print_task(cli.json, &task)?;
        }
        Command::Task {
            command: TaskCommand::List,
        } => {
            let tasks = client.list().await?;
            if cli.json {
                print_value(true, &tasks)?;
            } else {
                for task in tasks {
                    println!("{}\t{}\t{}", task.id, task.state, task.title);
                }
            }
        }
        Command::Task {
            command: TaskCommand::Outputs { task_id },
        } => {
            let task = client.get(&task_id).await?;
            if cli.json {
                print_value(true, &task.outputs)?;
            } else {
                for output in task.outputs {
                    println!("{}\t{:?}\t{}", output.id, output.kind, output.title);
                }
            }
        }
        Command::Task {
            command: TaskCommand::Records { task_id },
        } => {
            let task = client.get(&task_id).await?;
            if cli.json {
                print_value(true, &task.records)?;
            } else {
                for record in task.records {
                    println!("{}\t{:?}\t{:?}", record.id, record.kind, record.subject);
                }
            }
        }
        Command::Task {
            command: TaskCommand::Artifacts { task_id },
        } => {
            let task = client.get(&task_id).await?;
            if cli.json {
                print_value(true, &task.artifacts)?;
            } else {
                for artifact in task.artifacts {
                    println!("{}\t{}", artifact.kind, artifact.path);
                }
            }
        }
    }
    Ok(())
}

fn submit_request(args: &SubmitArgs) -> Result<SubmitTaskRequest> {
    if args.repos.is_empty() {
        bail!("at least one --repo is required");
    }
    if !args.repo_names.is_empty() && args.repo_names.len() != args.repos.len() {
        bail!("--repo-name count must match --repo count when provided");
    }
    let repos = args
        .repos
        .iter()
        .enumerate()
        .map(|(idx, url)| RepoSpec {
            url: url.clone(),
            name: args
                .repo_names
                .get(idx)
                .cloned()
                .unwrap_or_else(|| infer_repo_name(url)),
            checkout: None,
        })
        .collect();

    Ok(SubmitTaskRequest {
        title: args.title.clone(),
        intent: args.intent.clone(),
        repos,
        harness: HarnessSpec {
            kind: args.harness.into(),
        },
        executor: ExecutorSpec {
            kind: args.executor.into(),
        },
    })
}

fn infer_repo_name(url: &str) -> String {
    url.rsplit(['/', ':'])
        .next()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_string()
}

async fn wait_for_task(client: &Client, task_id: String, timeout_secs: u64) -> Result<Task> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let task = client.get(&task_id).await?;
        if task.state.is_terminal() {
            return Ok(task);
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out waiting for task {task_id}; last state was {}",
                task.state
            );
        }
        sleep(Duration::from_millis(500)).await;
    }
}

/// Follow a task live: stream new task records as they are appended and tail
/// the harness protocol log through the artifact content API. Returns the
/// final task once it reaches a terminal state.
async fn watch_task(client: &Client, json: bool, task_id: &str, timeout_secs: u64) -> Result<Task> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut printer = WatchPrinter::new(json);
    let mut seen_records = 0_usize;
    let mut log_tail: Option<HarnessLogTail> = None;

    loop {
        let task = client.get(task_id).await?;
        if log_tail.is_none() {
            log_tail = HarnessLogTail::discover(&task);
        }
        if let Some(tail) = &mut log_tail {
            for event in tail.drain(client, task_id).await? {
                printer.harness_event(&event);
            }
        }
        for record in task.records.iter().skip(seen_records) {
            printer.record(record, task.created_at_ms)?;
        }
        seen_records = task.records.len();

        if task.state.is_terminal() {
            if let Some(tail) = &mut log_tail {
                for event in tail.drain(client, task_id).await? {
                    printer.harness_event(&event);
                }
            }
            printer.finish();
            return Ok(task);
        }
        if Instant::now() >= deadline {
            printer.finish();
            bail!(
                "timed out watching task {task_id}; last state was {}",
                task.state
            );
        }
        sleep(Duration::from_millis(500)).await;
    }
}

struct WatchPrinter {
    json: bool,
    /// True while the last thing printed was a streaming agent chunk without a
    /// trailing newline, so structured lines can restore column zero first.
    mid_stream: bool,
}

impl WatchPrinter {
    fn new(json: bool) -> Self {
        Self {
            json,
            mid_stream: false,
        }
    }

    fn record(&mut self, record: &TaskRecord, task_created_at_ms: u128) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string(record)?);
            return Ok(());
        }
        self.break_stream();
        let elapsed_ms = record.created_at_ms.saturating_sub(task_created_at_ms);
        #[allow(clippy::cast_precision_loss)]
        let elapsed_secs = elapsed_ms as f64 / 1000.0;
        println!("[+{elapsed_secs:7.1}s] {}", render_record(record));
        Ok(())
    }

    fn harness_event(&mut self, event: &HarnessEvent) {
        if self.json {
            return;
        }
        match event {
            HarnessEvent::AgentText(text) => {
                print!("{text}");
                let _ = std::io::stdout().flush();
                self.mid_stream = !text.ends_with('\n');
            }
            HarnessEvent::ToolCall(title) => {
                self.break_stream();
                println!("[harness] tool: {title}");
            }
        }
    }

    fn break_stream(&mut self) {
        if self.mid_stream {
            println!();
            self.mid_stream = false;
        }
    }

    fn finish(&mut self) {
        self.break_stream();
    }
}

fn render_record(record: &TaskRecord) -> String {
    let kind = record_kind_label(record.kind);
    let body = &record.body;
    let detail = match record.kind {
        TaskRecordKind::StateChanged => format!(
            "{} -> {}",
            body_str(body, "from").unwrap_or("?"),
            body_str(body, "to").unwrap_or("?")
        ),
        TaskRecordKind::InputReceived => format!(
            "title={:?} user={}",
            body_str(body, "title").unwrap_or("?"),
            body_str(body, "user_id").unwrap_or("?")
        ),
        TaskRecordKind::TaskClaimed | TaskRecordKind::TaskReleased => format!(
            "claim={} node={}",
            body_str(body, "claim_id").unwrap_or("?"),
            body_str(body, "node_id").unwrap_or("?")
        ),
        TaskRecordKind::ContextPrepared => format!(
            "context={} workspace={}",
            body_str(body, "context_id").unwrap_or("?"),
            body_str(body, "workspace_path").unwrap_or("?")
        ),
        TaskRecordKind::ArtifactCreated => format!(
            "{} {}",
            body_str(body, "kind").unwrap_or("?"),
            body_str(body, "path").unwrap_or("?")
        ),
        TaskRecordKind::OutputCreated => body_str(body, "title").unwrap_or("?").to_string(),
        TaskRecordKind::ActionFailed | TaskRecordKind::SessionFailed => {
            format!("error: {}", body_str(body, "error").unwrap_or("?"))
        }
        _ => subject_label(record),
    };
    if detail.is_empty() {
        kind
    } else {
        format!("{kind} {detail}")
    }
}

fn record_kind_label(kind: TaskRecordKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn subject_label(record: &TaskRecord) -> String {
    match serde_json::to_value(&record.subject) {
        Ok(value) => value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        Err(_) => String::new(),
    }
}

fn body_str<'a>(body: &'a Value, key: &str) -> Option<&'a str> {
    body.get(key).and_then(Value::as_str)
}

enum HarnessEvent {
    AgentText(String),
    ToolCall(String),
}

/// Incrementally fetches the harness protocol log (`opencode-acp-log`
/// artifact) through the daemon's artifact content API and turns agent
/// message chunks and tool calls into printable events. Works identically for
/// local and remote daemons.
struct HarnessLogTail {
    artifact_position: usize,
    offset: u64,
    partial: String,
}

impl HarnessLogTail {
    fn discover(task: &Task) -> Option<Self> {
        let artifact_position = task
            .artifacts
            .iter()
            .position(|artifact| artifact.kind == "opencode-acp-log")?;
        Some(Self {
            artifact_position,
            offset: 0,
            partial: String::new(),
        })
    }

    async fn drain(&mut self, client: &Client, task_id: &str) -> Result<Vec<HarnessEvent>> {
        let bytes = client
            .artifact_content(task_id, self.artifact_position, self.offset)
            .await?;
        self.offset += bytes.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&bytes));

        let mut events = Vec::new();
        while let Some(newline) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=newline).collect();
            if let Some(event) = parse_harness_log_line(line.trim_end()) {
                events.push(event);
            }
        }
        Ok(events)
    }
}

fn parse_harness_log_line(line: &str) -> Option<HarnessEvent> {
    let payload = line.strip_prefix("agent ")?;
    let message: Value = serde_json::from_str(payload).ok()?;
    let update = message.pointer("/params/update")?;
    match update.get("sessionUpdate").and_then(Value::as_str)? {
        "agent_message_chunk" => update
            .pointer("/content/text")
            .and_then(Value::as_str)
            .map(|text| HarnessEvent::AgentText(text.to_string())),
        "tool_call" => {
            let title = update
                .get("title")
                .and_then(Value::as_str)
                .or_else(|| update.get("kind").and_then(Value::as_str))
                .unwrap_or("unnamed tool call");
            let location = update
                .pointer("/locations/0/path")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty());
            let label = match location {
                Some(path) => format!("{title} {path}"),
                None => title.to_string(),
            };
            Some(HarnessEvent::ToolCall(label))
        }
        _ => None,
    }
}

fn print_value<T: serde::Serialize>(json: bool, value: &T) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    }
    Ok(())
}

/// Review surface for a finished task: the summary output plus any captured
/// repo diffs, fetched through the daemon's artifact content API so it works
/// for local and remote daemons alike.
async fn review_task(client: &Client, json: bool, task: &Task) -> Result<()> {
    let mut diffs = Vec::new();
    for (position, artifact) in task.artifacts.iter().enumerate() {
        if artifact.kind != "repo-diff" {
            continue;
        }
        let content = client
            .artifact_content(&task.id.to_string(), position, 0)
            .await
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        diffs.push((artifact.path.clone(), content));
    }

    if json {
        let value = serde_json::json!({
            "task": task,
            "diffs": diffs
                .iter()
                .map(|(path, content)| serde_json::json!({"path": path, "content": content}))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    println!("task {} {}", task.id, task.state);
    if let Some(summary) = &task.summary {
        println!("\n## Summary\n\n{summary}");
    }
    if let Some(error) = &task.last_error {
        println!("\n## Error\n\n{error}");
    }
    if diffs.is_empty() {
        println!("\n(no repo diffs were captured for this task)");
    }
    for (path, content) in &diffs {
        match content {
            Some(diff) => println!("\n## Diff: {path}\n\n{diff}"),
            None => println!("\n## Diff: {path}\n\n(content could not be fetched)"),
        }
    }
    Ok(())
}

fn print_task(json: bool, task: &Task) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(task)?);
    } else {
        println!("task {} {}", task.id, task.state);
        if let Some(summary) = &task.summary {
            println!("\n{summary}");
        }
        if let Some(error) = &task.last_error {
            println!("\nerror: {error}");
        }
    }
    Ok(())
}

/// Final line after a watch stream. In JSON mode the stream is NDJSON records,
/// so the final task is emitted as a single compact JSON line rather than the
/// pretty form used by `task get`.
fn print_watch_final(json: bool, task: &Task) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(task)?);
        Ok(())
    } else {
        print_task(false, task)
    }
}

struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    fn new(base: String) -> Result<Self> {
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http: reqwest::Client::builder().build()?,
        })
    }

    async fn health(&self) -> Result<HealthResponse> {
        self.get_json("/health").await
    }

    async fn submit(&self, request: &SubmitTaskRequest) -> Result<SubmitTaskResponse> {
        self.post_json("/tasks", request).await
    }

    async fn get(&self, task_id: &str) -> Result<Task> {
        self.get_json(&format!("/tasks/{task_id}")).await
    }

    async fn list(&self) -> Result<Vec<Task>> {
        self.get_json("/tasks").await
    }

    /// Fetch artifact bytes from a byte offset via the daemon API.
    async fn artifact_content(
        &self,
        task_id: &str,
        position: usize,
        offset: u64,
    ) -> Result<Vec<u8>> {
        let response = self
            .http
            .get(format!(
                "{}/tasks/{task_id}/artifacts/{position}/content?offset={offset}",
                self.base
            ))
            .send()
            .await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if status.is_success() {
            Ok(bytes.to_vec())
        } else {
            let body = String::from_utf8_lossy(&bytes);
            bail!("workd returned {status}: {body}");
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self.http.get(format!("{}{path}", self.base)).send().await?;
        decode_response(response).await
    }

    async fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let response = self
            .http
            .post(format!("{}{path}", self.base))
            .json(body)
            .send()
            .await?;
        decode_response(response).await
    }
}

async fn decode_response<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    let status = response.status();
    let bytes = response.bytes().await?;
    if status.is_success() {
        Ok(serde_json::from_slice(&bytes)?)
    } else {
        let body = String::from_utf8_lossy(&bytes);
        bail!("workd returned {status}: {body}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use workctl_core::{RecordId, RecordSubject, TaskState};

    #[test]
    fn infers_repo_names_from_ssh_urls() {
        assert_eq!(
            infer_repo_name("git@git.sr.ht:~averagechris/linear-cli"),
            "linear-cli"
        );
        assert_eq!(infer_repo_name("https://example.com/foo.git"), "foo");
    }

    #[test]
    fn parses_agent_message_chunks_from_harness_log() {
        let line = r#"agent {"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hello"}}}}"#;
        match parse_harness_log_line(line) {
            Some(HarnessEvent::AgentText(text)) => assert_eq!(text, "hello"),
            other => panic!(
                "expected agent text, got {:?}",
                other.map(|event| match event {
                    HarnessEvent::AgentText(text) => format!("text:{text}"),
                    HarnessEvent::ToolCall(title) => format!("tool:{title}"),
                })
            ),
        }
    }

    #[test]
    fn parses_tool_calls_and_ignores_client_lines() {
        let tool = r#"agent {"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"tool_call","title":"read file"}}}"#;
        assert!(matches!(
            parse_harness_log_line(tool),
            Some(HarnessEvent::ToolCall(title)) if title == "read file"
        ));
        let client = r#"client {"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#;
        assert!(parse_harness_log_line(client).is_none());
        assert!(parse_harness_log_line("not json").is_none());
    }

    #[test]
    fn parses_tool_call_with_location_path() {
        let line = r#"agent {"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"tool_call","title":"read","locations":[{"path":"src/main.rs"}]}}}"#;
        assert!(matches!(
            parse_harness_log_line(line),
            Some(HarnessEvent::ToolCall(title)) if title == "read src/main.rs"
        ));
    }

    #[test]
    fn renders_state_change_records() {
        let record = TaskRecord {
            id: RecordId::new(),
            kind: TaskRecordKind::StateChanged,
            subject: RecordSubject::Task,
            body: serde_json::json!({"from": TaskState::Running, "to": TaskState::Done}),
            created_at_ms: 1,
        };
        assert_eq!(render_record(&record), "state_changed running -> done");
    }

    #[test]
    fn renders_failure_records_with_error_detail() {
        let record = TaskRecord {
            id: RecordId::new(),
            kind: TaskRecordKind::SessionFailed,
            subject: RecordSubject::Task,
            body: serde_json::json!({"error": "boom"}),
            created_at_ms: 1,
        };
        assert_eq!(render_record(&record), "session_failed error: boom");
    }
}
