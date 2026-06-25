# Key concepts

## Control flow

```text
user intent
  -> workctl CLI
  -> workd control plane
  -> worker claims action
  -> ContextPreparer creates ExecutionContext
  -> AgentHarness runs inside that context
  -> observations, artifacts, outputs, leases, audit events
  -> automatic cleanup when gates pass
```

## Core entities

### Organization / tenant

Isolation boundary for users, tasks, repos, nodes, config, artifacts, and policy.
Personal local use still creates a default organization.

### User

Human or service identity authenticated to the control plane. Users access tasks
through organization memberships and roles.

### Node

Machine or execution environment that can prepare contexts, run sessions, report
observations, or clean local resources. Laptops, servers, and clusters are nodes.

### Task

Primary product object. Records delegated work intent, context requirements,
lifecycle state, desired harness/executor, ownership, outputs, artifacts, and
handoff history.

Minimal lifecycle:

```text
created -> metadata_ready -> context_requested -> context_ready
        -> running -> review_ready | needs_user | failed -> done
```

External workflow details such as GitHub PRs, direct merges, patch emails, CI, or
Linear state are observations/outputs, not core task states.

### ExecutionContext

Persisted record for a prepared task environment:

- runtime handle;
- mounted/materialized repos;
- mounted/injected secrets;
- generated prompt/context files;
- tools and resource limits;
- artifact output locations.

### RuntimeHandle

Backend-specific reference to the allocated runtime: process group, isolate ID,
container ID, VM ID, pod name, etc.

### ContextManifest

Artifact describing what was mounted, injected, generated, and exposed inside an
execution context. Secret values are never stored in the manifest.

### AgentHarness

Adapter for the coding loop: OpenCode, `pi`, ACP, or another agent interface.

### ExecutorBackend

Runtime implementation: process runtime/dev shell, container, isolate, VM,
Kubernetes job/pod, etc.

### Session

Bounded attempt to work on a task inside an execution context. Usually an agent
run. Session state may be `completed`; task state uses `done`.

### Task output

Review or integration result produced by a task: local change, direct commit, PR,
patch email, review summary, release note, migration plan, etc. A task may have
many outputs across many mounted repos.

### Lease

Explicit ownership for tasks, contexts, sessions, runtime resources, and actions.
Leases enable safe takeover and block automatic cleanup.

### Observation

Refreshable fact from workers or integrations: repo status, runtime state,
filesystem state, Linear/GitHub state, CI, review state, etc.

### Artifact

Large file/object stored outside the DB with a pointer/checksum: prompts, logs,
summaries, test output, context manifests, cleanup reports, snapshots.

## Trait seams

Model these boundaries as Rust traits:

- `ControlStore` — in-memory test store, SQLite, Postgres.
- `IssueTracker` — Linear, GitHub, manual tasks, other providers.
- `ContextPreparer` — orchestrates runtime allocation, repo mounts, secret
  mounts, context files, artifact paths.
- `ArtifactStore` — stores/retrieves artifacts outside the DB.
- `AgentHarness` — OpenCode, `pi`, ACP, other agents.
- `ExecutorBackend` — process runtime, container, isolate, VM, Kubernetes.
- `PolicyEngine` — evaluates dynamic config and policy.

## Context preparation boundary

`ContextPreparer` orchestrates:

- choosing/validating node;
- deciding required repos, secrets, tools, files, artifact paths;
- applying dynamic config and policy;
- asking `ExecutorBackend` to allocate/prepare runtime;
- mounting/materializing repos;
- mounting/injecting secrets;
- writing `ContextManifest`;
- recording `ExecutionContext` and `RuntimeHandle`.

`ExecutorBackend` owns runtime primitives: allocate, status, stop, cleanup.

`AgentHarness` owns agent protocol semantics and output classification.

## Cleanup gates

Completed-task resources may be archived/deleted automatically when gates pass:

- task is `done` or `abandoned` according to policy;
- no active lease;
- no running session;
- resource is not protected;
- source/context state is clean, exported, archived, or intentionally abandoned;
- required artifacts have been retained or archived;
- age/retention threshold is satisfied;
- node/policy allows cleanup.

When gates fail, cleanup is skipped or surfaced as attention if user input is
useful.
