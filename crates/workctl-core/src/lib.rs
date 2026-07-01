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
#[serde(transparent)]
pub struct UserId(pub String);

impl Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub String);

impl Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExecutionContextId(pub String);

impl ExecutionContextId {
    #[must_use]
    pub fn new() -> Self {
        Self(format!("ctx_{}", Uuid::new_v4().simple()))
    }
}

impl Default for ExecutionContextId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ExecutionContextId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

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
pub struct SessionId(pub String);

impl SessionId {
    #[must_use]
    pub fn new() -> Self {
        Self(format!("sess_{}", Uuid::new_v4().simple()))
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActionId(pub String);

impl ActionId {
    #[must_use]
    pub fn new() -> Self {
        Self(format!("action_{}", Uuid::new_v4().simple()))
    }
}

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ActionId {
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
    TaskClaimed,
    TaskReleased,
    ActionStarted,
    ActionCompleted,
    ActionFailed,
    ContextPrepared,
    StateChanged,
    SessionStarted,
    SessionCompleted,
    SessionFailed,
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
    Action(ActionId),
    Session(SessionId),
    Output(OutputId),
    Artifact(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub organization_id: OrganizationId,
    pub user_id: UserId,
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
    pub context_id: ExecutionContextId,
    pub task_id: TaskId,
    pub node_id: NodeId,
    pub executor: ExecutorKind,
    pub harness: HarnessKind,
    pub runtime_handle: RuntimeHandle,
    pub workspace_path: String,
    pub repos: Vec<PreparedRepo>,
    pub artifact_dir: String,
    pub prompt_path: String,
    pub devshell: DevshellManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHandle {
    pub kind: String,
    pub id: String,
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
        assert!(ExecutionContextId::new().0.starts_with("ctx_"));
        assert!(OutputId::new().0.starts_with("out_"));
        assert!(SessionId::new().0.starts_with("sess_"));
        assert!(ActionId::new().0.starts_with("action_"));
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

    #[test]
    fn task_record_serializes_state_changes() {
        let record = TaskRecord {
            id: RecordId("rec_state".into()),
            kind: TaskRecordKind::StateChanged,
            subject: RecordSubject::Task,
            body: serde_json::json!({"from": TaskState::Running, "to": TaskState::Done}),
            created_at_ms: 1,
        };

        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["kind"], "state_changed");
        assert_eq!(json["body"]["from"], "running");
        assert_eq!(json["body"]["to"], "done");
    }

    #[test]
    fn task_record_serializes_sessions() {
        let record = TaskRecord {
            id: RecordId("rec_session".into()),
            kind: TaskRecordKind::SessionStarted,
            subject: RecordSubject::Session(SessionId("sess_test".into())),
            body: serde_json::json!({"harness": HarnessKind::FakeSummary}),
            created_at_ms: 1,
        };

        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["kind"], "session_started");
        assert_eq!(json["subject"]["kind"], "session");
        assert_eq!(json["subject"]["id"], "sess_test");
        assert_eq!(json["body"]["harness"], "fake-summary");
    }

    #[test]
    fn task_record_serializes_context_prepared() {
        let record = TaskRecord {
            id: RecordId("rec_context".into()),
            kind: TaskRecordKind::ContextPrepared,
            subject: RecordSubject::Task,
            body: serde_json::json!({
                "action_id": ActionId("action_test".into()),
                "context_id": ExecutionContextId("ctx_test".into()),
                "runtime_handle": {"kind": "local-devshell", "id": "/tmp/workspace"}
            }),
            created_at_ms: 1,
        };

        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["kind"], "context_prepared");
        assert_eq!(json["body"]["action_id"], "action_test");
        assert_eq!(json["body"]["context_id"], "ctx_test");
    }

    #[test]
    fn task_record_serializes_claims() {
        let record = TaskRecord {
            id: RecordId("rec_claim".into()),
            kind: TaskRecordKind::TaskClaimed,
            subject: RecordSubject::Task,
            body: serde_json::json!({"claim_id": "claim_test", "node_id": NodeId("local".into())}),
            created_at_ms: 1,
        };

        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["kind"], "task_claimed");
        assert_eq!(json["body"]["claim_id"], "claim_test");
        assert_eq!(json["body"]["node_id"], "local");
    }

    #[test]
    fn task_record_serializes_actions() {
        let record = TaskRecord {
            id: RecordId("rec_action".into()),
            kind: TaskRecordKind::ActionStarted,
            subject: RecordSubject::Action(ActionId("action_test".into())),
            body: serde_json::json!({"kind": "process-created-task"}),
            created_at_ms: 1,
        };

        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["kind"], "action_started");
        assert_eq!(json["subject"]["kind"], "action");
        assert_eq!(json["subject"]["id"], "action_test");
    }

    #[test]
    fn context_manifest_serializes_runtime_handle() {
        let manifest = ContextManifest {
            context_id: ExecutionContextId("ctx_test".into()),
            task_id: TaskId("task_test".into()),
            node_id: NodeId("local".into()),
            executor: ExecutorKind::LocalDevshell,
            harness: HarnessKind::FakeSummary,
            runtime_handle: RuntimeHandle {
                kind: "local-devshell".into(),
                id: "/tmp/workspace".into(),
            },
            workspace_path: "/tmp/workspace".into(),
            repos: Vec::new(),
            artifact_dir: "/tmp/workspace/artifacts".into(),
            prompt_path: "/tmp/workspace/prompts/task.md".into(),
            devshell: DevshellManifest {
                mode: DevshellMode::DirectProcess,
                command: vec!["fake-summary".into()],
            },
        };

        let json = serde_json::to_value(manifest).unwrap();
        assert_eq!(json["context_id"], "ctx_test");
        assert_eq!(json["node_id"], "local");
        assert_eq!(json["runtime_handle"]["kind"], "local-devshell");
        assert_eq!(json["runtime_handle"]["id"], "/tmp/workspace");
    }
}
