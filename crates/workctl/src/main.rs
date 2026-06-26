use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use workctl_core::{
    ExecutorKind, ExecutorSpec, HarnessKind, HarnessSpec, HealthResponse, RepoSpec,
    SubmitTaskRequest, SubmitTaskResponse, Task, product_sentence,
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
    Get { task_id: String },
    List,
    Outputs { task_id: String },
    Records { task_id: String },
    Artifacts { task_id: String },
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
            if args.wait {
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

fn print_value<T: serde::Serialize>(json: bool, value: &T) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
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

    #[test]
    fn infers_repo_names_from_ssh_urls() {
        assert_eq!(
            infer_repo_name("git@git.sr.ht:~averagechris/linear-cli"),
            "linear-cli"
        );
        assert_eq!(infer_repo_name("https://example.com/foo.git"), "foo");
    }
}
