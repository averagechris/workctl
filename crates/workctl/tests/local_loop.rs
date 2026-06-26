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

    kill(&mut workd);
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let task: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(task["state"], "done");
    assert!(task["summary"].as_str().unwrap().contains("README.md"));
    let workspace = task["workspace_path"].as_str().unwrap();
    assert!(Path::new(workspace).join("repos/fixture/.git").exists());
    assert!(
        Path::new(workspace)
            .join("artifacts/context-manifest.json")
            .exists()
    );
    assert!(Path::new(workspace).join("artifacts/summary.md").exists());
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
    let workspace = task["workspace_path"].as_str().unwrap();
    assert!(Path::new(workspace).join("repos/linear-cli/.git").exists());
    assert!(
        Path::new(workspace)
            .join("artifacts/opencode-acp.ndjson")
            .exists()
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
