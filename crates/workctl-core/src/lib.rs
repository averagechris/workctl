//! Shared models and API payloads for the local `workctl` / `workd` slice.

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use uuid::Uuid;

/// Stable product sentence used by bootstrap binaries and docs.
#[must_use]
pub fn product_sentence() -> &'static str {
    "workctl is a Rust control plane for delegated software-development tasks"
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub String);

impl TaskId {
    #[must_use]
    pub fn new() -> Self {
        Self(format!("task_{}", Uuid::new_v4().simple()))
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContextId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputId(pub String);

impl OutputId {
    #[must_use]
    pub fn new() -> Self {
        Self(format!("out_{}", Uuid::new_v4().simple()))
    }
}

impl Default for OutputId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for OutputId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordId(pub String);

impl RecordId {
    #[must_use]
    pub fn new() -> Self {
        Self(format!("rec_{}", Uuid::new_v4().simple()))
    }
}

impl Default for RecordId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSpec {
    pub url: String,
    pub name: String,
    pub checkout: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessSpec {
    pub kind: HarnessKind,
}

impl Default for HarnessSpec {
    fn default() -> Self {
        Self {
            kind: HarnessKind::OpencodeAcp,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessKind {
    OpencodeAcp,
    FakeSummary,
}

impl Display for HarnessKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpencodeAcp => f.write_str("opencode-acp"),
            Self::FakeSummary => f.write_str("fake-summary"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutorSpec {
    pub kind: ExecutorKind,
}

impl Default for ExecutorSpec {
    fn default() -> Self {
        Self {
            kind: ExecutorKind::LocalDevshell,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutorKind {
    LocalDevshell,
}

impl Display for ExecutorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalDevshell => f.write_str("local-devshell"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub repos: Vec<RepoSpec>,
    pub harness: HarnessSpec,
    pub executor: ExecutorSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Created,
    ContextRequested,
    ContextReady,
    Running,
    ReviewReady,
    Failed,
    Done,
}

impl TaskState {
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::ReviewReady | Self::Failed | Self::Done)
    }
}

impl Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(state.trim_matches('"'))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOutput {
    pub id: OutputId,
    pub kind: OutputKind,
    pub title: String,
    pub body: String,
    pub source_artifacts: Vec<String>,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputKind {
    Summary,
    Log,
    Handoff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: RecordId,
    pub kind: TaskRecordKind,
    pub subject: RecordSubject,
    pub body: serde_json::Value,
    pub created_at_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRecordKind {
    InputReceived,
    OutputCreated,
    ArtifactCreated,
    ProjectionRequested,
    ProjectionSucceeded,
    ProjectionFailed,
    IntegrationObserved,
    Acknowledged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "kebab-case")]
pub enum RecordSubject {
    Task,
    Output(OutputId),
    Artifact(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub organization_id: OrganizationId,
    pub state: TaskState,
    pub title: String,
    pub intent: String,
    pub spec: TaskSpec,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub outputs: Vec<TaskOutput>,
    #[serde(default)]
    pub records: Vec<TaskRecord>,
    #[serde(default)]
    pub last_error: Option<String>,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitTaskRequest {
    pub title: String,
    pub intent: String,
    pub repos: Vec<RepoSpec>,
    #[serde(default)]
    pub harness: HarnessSpec,
    #[serde(default)]
    pub executor: ExecutorSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitTaskResponse {
    pub task_id: TaskId,
    pub state: TaskState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub product: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("daemon error: {0}")]
    Daemon(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifest {
    pub task_id: TaskId,
    pub executor: ExecutorKind,
    pub harness: HarnessKind,
    pub workspace_path: String,
    pub repos: Vec<PreparedRepo>,
    pub artifact_dir: String,
    pub prompt_path: String,
    pub devshell: DevshellManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedRepo {
    pub name: String,
    pub url: String,
    pub path: String,
    pub checkout: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevshellManifest {
    pub mode: DevshellMode,
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DevshellMode {
    NixDevelop,
    DirectProcess,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_state_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&TaskState::ContextReady).unwrap(),
            "\"context_ready\""
        );
    }

    #[test]
    fn ids_are_prefixed() {
        assert!(TaskId::new().0.starts_with("task_"));
        assert!(OutputId::new().0.starts_with("out_"));
        assert!(RecordId::new().0.starts_with("rec_"));
    }

    #[test]
    fn task_record_serializes_projection_events() {
        let record = TaskRecord {
            id: RecordId("rec_test".into()),
            kind: TaskRecordKind::ProjectionRequested,
            subject: RecordSubject::Output(OutputId("out_test".into())),
            body: serde_json::json!({
                "integration": "linear",
                "target": {"issue_id": "ABC-123", "surface": "comment"},
                "idempotency_key": "task/out/linear/ABC-123/comment"
            }),
            created_at_ms: 1,
        };

        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["kind"], "projection_requested");
        assert_eq!(json["subject"]["kind"], "output");
        assert_eq!(json["body"]["integration"], "linear");
    }
}
