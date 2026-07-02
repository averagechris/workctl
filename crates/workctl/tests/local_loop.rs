use serde_json::Value;
use std::{
    net::TcpListener,
    path::Path,
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

#[test]
fn cli_submits_task_to_workd_loop_with_fake_harness() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("source-repo");
    create_git_repo(&repo);

    let bind = free_addr();
    let state_dir = temp.path().join("state");
    let mut workd_command = cargo_run("workd");
    let mut workd = workd_command
        .arg("serve")
        .arg("--bind")
        .arg(&bind)
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("--worker-interval-ms")
        .arg("100")
        .spawn()
        .unwrap();

    wait_for_workd(&bind);

    let mut workctl_command = cargo_run("workctl");
    let output = workctl_command
        .arg("--server")
        .arg(format!("http://{bind}"))
        .arg("--json")
        .arg("submit")
        .arg("--title")
        .arg("Summarize fixture")
        .arg("--intent")
        .arg("Summarize this fixture repository")
        .arg("--repo")
        .arg(repo.display().to_string())
        .arg("--repo-name")
        .arg("fixture")
        .arg("--harness")
        .arg("fake-summary")
        .arg("--wait")
        .arg("--timeout-secs")
        .arg("30")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let task: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(task["state"], "done");
    assert_eq!(task["user_id"], "local");
    assert!(task["summary"].as_str().unwrap().contains("README.md"));
    assert_eq!(task["outputs"][0]["kind"], "summary");
    assert!(
        task["outputs"][0]["body"]
            .as_str()
            .unwrap()
            .contains("README.md")
    );
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "input_received" && record["subject"]["kind"] == "task"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "input_received"
            && record["body"]["spec"]["repos"][0]["name"] == "fixture"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "input_received" && record["body"]["user_id"] == "local"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "task_claimed"
            && record["body"]["claim_id"]
                .as_str()
                .unwrap()
                .starts_with("claim_")
            && record["body"]["node_id"] == "local"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "task_released"
            && record["body"]["claim_id"]
                .as_str()
                .unwrap()
                .starts_with("claim_")
            && record["body"]["node_id"] == "local"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "action_started"
            && record["subject"]["kind"] == "action"
            && record["subject"]["id"]
                .as_str()
                .unwrap()
                .starts_with("action_")
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "action_completed"
            && record["subject"]["kind"] == "action"
            && record["body"]["kind"] == "process-created-task"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "output_created" && record["subject"]["kind"] == "output"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "output_created"
            && record["body"]["session_id"]
                .as_str()
                .unwrap()
                .starts_with("sess_")
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "session_started" && record["subject"]["kind"] == "session"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "session_started"
            && record["body"]["action_id"]
                .as_str()
                .unwrap()
                .starts_with("action_")
            && record["body"]["context_id"]
                .as_str()
                .unwrap()
                .starts_with("ctx_")
            && record["body"]["runtime_handle"]["kind"] == "local-devshell"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "session_completed" && record["subject"]["kind"] == "session"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "context_prepared"
            && record["body"]["action_id"]
                .as_str()
                .unwrap()
                .starts_with("action_")
            && record["body"]["runtime_handle"]["kind"] == "local-devshell"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "artifact_created" && record["subject"]["kind"] == "artifact"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "artifact_created" && record["body"]["kind"] == "prompt"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "artifact_created"
            && record["body"]["kind"] == "prompt"
            && record["body"]["source"]["kind"] == "context"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "artifact_created"
            && record["body"]["kind"] == "summary"
            && record["body"]["source"]["kind"] == "session"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "state_changed"
            && record["body"]["to"] == "done"
            && record["body"]["action_id"]
                .as_str()
                .unwrap()
                .starts_with("action_")
    }));
    let workspace = task["workspace_path"].as_str().unwrap();
    assert!(Path::new(workspace).join("repos/fixture/.git").exists());
    assert!(
        Path::new(workspace)
            .join("artifacts/context-manifest.json")
            .exists()
    );
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(Path::new(workspace).join("artifacts/context-manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(manifest["context_id"].as_str().unwrap().starts_with("ctx_"));
    assert_eq!(manifest["node_id"], "local");
    assert_eq!(manifest["runtime_handle"]["kind"], "local-devshell");
    assert!(Path::new(workspace).join("prompts/task.md").exists());
    assert!(Path::new(workspace).join("artifacts/summary.md").exists());

    // Artifact content is served through the API with offset support.
    let task_id = task["id"].as_str().unwrap();
    let artifacts = task["artifacts"].as_array().unwrap();
    let summary_position = artifacts
        .iter()
        .position(|artifact| artifact["kind"] == "summary")
        .unwrap();
    let content_url = format!("http://{bind}/tasks/{task_id}/artifacts/{summary_position}/content");
    let full = reqwest::blocking::get(&content_url).unwrap();
    assert!(full.status().is_success());
    let full_body = full.text().unwrap();
    assert!(full_body.contains("README.md"), "body={full_body}");

    let tail = reqwest::blocking::get(format!("{content_url}?offset=5"))
        .unwrap()
        .text()
        .unwrap();
    assert_eq!(tail, full_body[5..], "offset read must skip bytes");

    let missing = reqwest::blocking::get(format!(
        "http://{bind}/tasks/{task_id}/artifacts/999/content"
    ))
    .unwrap();
    assert_eq!(missing.status().as_u16(), 404);

    kill(&mut workd);
}

#[test]
#[ignore = "requires SSH access to sourcehut plus configured opencode credentials"]
fn e2e_opencode_summarizes_linear_cli() {
    if std::env::var("WORKCTL_E2E_OPENCODE").ok().as_deref() != Some("1") {
        eprintln!("set WORKCTL_E2E_OPENCODE=1 to run the real opencode/sourcehut E2E");
        return;
    }

    let temp = tempfile::tempdir().unwrap();
    let bind = free_addr();
    let state_dir = temp.path().join("state");
    let mut workd_command = cargo_run("workd");
    let mut workd = workd_command
        .arg("serve")
        .arg("--bind")
        .arg(&bind)
        .arg("--state-dir")
        .arg(&state_dir)
        .spawn()
        .unwrap();
    wait_for_workd(&bind);

    let mut workctl_command = cargo_run("workctl");
    let output = workctl_command
        .arg("--server")
        .arg(format!("http://{bind}"))
        .arg("--json")
        .arg("submit")
        .arg("--title")
        .arg("Summarize linear-cli")
        .arg("--intent")
        .arg("Clone git@git.sr.ht:~averagechris/linear-cli and summarize its purpose, structure, commands, dependencies, and notable implementation details using opencode ACP.")
        .arg("--repo")
        .arg("git@git.sr.ht:~averagechris/linear-cli")
        .arg("--repo-name")
        .arg("linear-cli")
        .arg("--harness")
        .arg("opencode-acp")
        .arg("--wait")
        .arg("--timeout-secs")
        .arg("1800")
        .output()
        .unwrap();

    kill(&mut workd);
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let task: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(task["state"], "done");
    let summary = task["summary"].as_str().unwrap();
    assert!(summary.to_lowercase().contains("linear"));
    assert_eq!(task["outputs"][0]["kind"], "summary");
    assert!(
        task["outputs"][0]["body"]
            .as_str()
            .unwrap()
            .contains(summary)
    );
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "output_created" && record["subject"]["kind"] == "output"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "artifact_created" && record["subject"]["kind"] == "artifact"
    }));
    assert!(task["records"].as_array().unwrap().iter().any(|record| {
        record["kind"] == "artifact_created" && record["body"]["kind"] == "prompt"
    }));
    assert!(
        task["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|record| { record["kind"] == "state_changed" && record["body"]["to"] == "done" })
    );
    let workspace = task["workspace_path"].as_str().unwrap();
    assert!(Path::new(workspace).join("repos/linear-cli/.git").exists());
    assert!(
        Path::new(workspace)
            .join("artifacts/opencode-acp.ndjson")
            .exists()
    );
}

#[test]
fn cli_watch_streams_records_until_terminal_state() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("source-repo");
    create_git_repo(&repo);

    let bind = free_addr();
    let state_dir = temp.path().join("state");
    let mut workd_command = cargo_run("workd");
    let mut workd = workd_command
        .arg("serve")
        .arg("--bind")
        .arg(&bind)
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("--worker-interval-ms")
        .arg("100")
        .spawn()
        .unwrap();

    wait_for_workd(&bind);

    let mut submit_command = cargo_run("workctl");
    let submit_output = submit_command
        .arg("--server")
        .arg(format!("http://{bind}"))
        .arg("--json")
        .arg("submit")
        .arg("--title")
        .arg("Watch fixture")
        .arg("--intent")
        .arg("Summarize this fixture repository")
        .arg("--repo")
        .arg(repo.display().to_string())
        .arg("--harness")
        .arg("fake-summary")
        .output()
        .unwrap();
    assert!(
        submit_output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&submit_output.stderr)
    );
    let submitted: Value = serde_json::from_slice(&submit_output.stdout).unwrap();
    let task_id = submitted["task_id"].as_str().unwrap();

    let mut watch_command = cargo_run("workctl");
    let watch_output = watch_command
        .arg("--server")
        .arg(format!("http://{bind}"))
        .arg("task")
        .arg("watch")
        .arg(task_id)
        .arg("--timeout-secs")
        .arg("30")
        .output()
        .unwrap();

    let mut review_command = cargo_run("workctl");
    let review_output = review_command
        .arg("--server")
        .arg(format!("http://{bind}"))
        .arg("task")
        .arg("review")
        .arg(task_id)
        .output()
        .unwrap();

    kill(&mut workd);
    assert!(
        watch_output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&watch_output.stdout),
        String::from_utf8_lossy(&watch_output.stderr)
    );
    let stdout = String::from_utf8_lossy(&watch_output.stdout);
    assert!(stdout.contains("input_received"), "stdout={stdout}");
    assert!(
        stdout.contains("state_changed created -> context_requested"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("state_changed running -> done"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("context_prepared"), "stdout={stdout}");
    assert!(stdout.contains("output_created"), "stdout={stdout}");
    assert!(
        stdout.contains(&format!("task {task_id} done")),
        "stdout={stdout}"
    );
    assert!(stdout.contains("README.md"), "stdout={stdout}");

    assert!(
        review_output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&review_output.stdout),
        String::from_utf8_lossy(&review_output.stderr)
    );
    let review_stdout = String::from_utf8_lossy(&review_output.stdout);
    assert!(
        review_stdout.contains("## Summary"),
        "stdout={review_stdout}"
    );
    assert!(
        review_stdout.contains("README.md"),
        "stdout={review_stdout}"
    );
    assert!(
        review_stdout.contains("no repo diffs were captured"),
        "stdout={review_stdout}"
    );
}

fn create_git_repo(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join("README.md"), "# Fixture\n").unwrap();
    std::fs::write(path.join("main.rs"), "fn main() {}\n").unwrap();
    run(Command::new("git").arg("init").arg(path));
    run(Command::new("git").arg("-C").arg(path).arg("add").arg("."));
    run(Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("-c")
        .arg("user.name=workctl test")
        .arg("-c")
        .arg("user.email=workctl@example.invalid")
        .arg("-c")
        .arg("commit.gpgsign=false")
        .arg("commit")
        .arg("-m")
        .arg("initial"));
}

fn run(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "command failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}

fn cargo_run(package: &str) -> Command {
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .arg("run")
        .arg("--quiet")
        .arg("-p")
        .arg(package)
        .arg("--");
    command
}

fn wait_for_workd(bind: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(response) = reqwest::blocking::get(format!("http://{bind}/health"))
            && response.status().is_success()
        {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("workd did not become healthy at {bind}");
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
