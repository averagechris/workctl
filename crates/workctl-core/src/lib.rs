//! Core domain concepts and trait seams for `workctl`.

/// Stable product sentence used by bootstrap binaries and docs.
#[must_use]
pub fn product_sentence() -> &'static str {
    "workctl is a Rust control plane for delegated software-development tasks"
}

/// Tenant boundary for users, tasks, repos, nodes, config, artifacts, and policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organization {
    pub id: OrganizationId,
    pub name: String,
}

/// Human or service identity authenticated to the control plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub organization_id: OrganizationId,
    pub display_name: String,
}

/// Machine or execution environment that can prepare contexts or run sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: NodeId,
    pub organization_id: OrganizationId,
    pub name: String,
}

/// Durable record of delegated development work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub organization_id: OrganizationId,
    pub state: TaskState,
    pub title: String,
    pub intent: String,
}

/// Minimal task lifecycle from creation through completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Created,
    MetadataReady,
    ContextRequested,
    ContextReady,
    Running,
    ReviewReady,
    NeedsUser,
    Failed,
    Done,
}

/// Persisted record for a prepared task environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContext {
    pub id: ExecutionContextId,
    pub task_id: TaskId,
    pub runtime_handle: RuntimeHandle,
    pub manifest_artifact_id: ArtifactId,
}

/// Backend-specific reference to an allocated runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHandle {
    pub backend: String,
    pub handle: String,
}

/// Large file/object stored outside the DB with a pointer/checksum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub id: ArtifactId,
    pub uri: String,
    pub checksum: Option<String>,
}

/// Canonical state storage boundary.
pub trait ControlStore {
    type Error;

    fn create_task(&mut self, task: Task) -> Result<(), Self::Error>;
    fn task(&self, id: &TaskId) -> Result<Option<Task>, Self::Error>;
}

/// External issue or work item provider boundary.
pub trait IssueTracker {
    type Error;
}

/// Orchestrates runtime allocation, mounts, context files, and manifests.
pub trait ContextPreparer {
    type Error;
}

/// Stores and retrieves artifacts outside the control-plane database.
pub trait ArtifactStore {
    type Error;
}

/// Adapter for an agent protocol or coding loop.
pub trait AgentHarness {
    type Error;
}

/// Runtime implementation boundary.
pub trait ExecutorBackend {
    type Error;
}

/// Dynamic config and cleanup/safety policy boundary.
pub trait PolicyEngine {
    type Error;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContextId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactId(pub String);
