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

Outputs are stored as durable control-plane records, not just files on disk,
because the same output may need to be projected to external surfaces such as
Linear comments, Slack threads, GitHub reviews, or manual handoff notes.

Publication state is modeled as a replayable task record stream, not as nested
state on the output itself. Integrations append projection-requested,
projection-succeeded, projection-failed, integration-observed, and acknowledged
records. Current views such as “has this summary been posted to Linear?” or
“which Slack thread still needs attention?” should be derived from the input,
output, artifact, and task-record history so they can be rehydrated from scratch.

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

## First local implementation pass

The current working slice intentionally favors a complete local task flow over a
complete distributed control plane:

```text
workctl submit
  -> HTTP request to local workd
  -> SQLite task/output/record rows under the daemon state dir
  -> embedded workd worker loop claims created task
  -> local-devshell context preparation clones repos and writes prompt/manifest
  -> fake-summary or opencode-acp harness runs in the prepared context
  -> summary output, output-created record, and protocol/context artifacts are written back to the task
```

This preserves the architectural seams from the north-star docs while keeping the
first pass inspectable. The single-node HTTP transport and embedded worker are
not the final deployment model; the durable semantics that should survive
replacement are task submission through `workd`, explicit context preparation,
executor/harness separation, durable outputs, replayable task records, and
artifact-backed large data.

`workd` accesses persistence through a `ControlStore` trait. The local
implementation is SQLite-backed, while worker claims remain an in-memory
single-daemon mechanism until leases/actions become durable store records.

Context preparation and harness execution are also represented as daemon-local
trait seams in the first pass. `LocalDevshellContextPreparer` owns the current
workspace/repo/prompt/manifest preparation path, while the local `AgentHarness`
implementation dispatches to `fake-summary` or `opencode-acp` based on the task
spec. `LocalDevshellExecutorBackend` owns the current process-runtime setup:
per-task `HOME`, XDG, temp directories, and the dev-shell command manifest.
`SourceMaterializer` owns source preparation; the first implementation clones Git
repos into the task workspace and applies optional checkout hints.

Prepared contexts include a backend-specific runtime handle in the context
manifest. For the first `local-devshell` backend, the handle kind is
`local-devshell` and the handle ID is the task workspace path; future executors
can replace this with process groups, container IDs, pods, or VM identifiers.
Each manifest also has a generated `ExecutionContextId` (`ctx_...`) so prepared
contexts have stable control-plane identity before execution contexts are split
into their own normalized store records.
The initial local implementation records node identity as static node ID `local`
in each context manifest. Future node registration/configuration can replace
this with durable node records, capabilities, and ownership/heartbeat metadata.
Context preparation also appends a `context_prepared` task record containing the
launching `action_id`, execution context ID, node ID, runtime handle, prepared
repos, key paths, and manifest path. This makes prepared context history
replayable even before execution contexts have their own store table.

Accepted task inputs are snapshotted into an `input_received` record containing
the submitted title, intent, repos, harness, and executor spec. The task row is
the queryable current view; the record preserves the accepted request for replay,
audit, and future projections.

Live observation in the local slice is `workctl task watch <id>` (or `submit
--watch`). The CLI polls the task and prints each newly appended task record,
so state changes, context preparation, sessions, outputs, and artifacts stream
as they happen; `--json` emits the records as NDJSON. When the opencode-acp
protocol log is visible on the local filesystem, watch also tails it and
streams agent message text and tool-call titles between records. Reading the
workspace log file directly is a local-milestone convenience that works because
the CLI and daemon share a machine; a remote deployment needs an observation
endpoint or artifact streaming instead.

The first local identity model assigns submitted tasks to organization `local`
and user `local`. The user ID is stored on the task row and included in the
`input_received` snapshot so later auth/user resolution can replace the local
default without losing audit semantics.

Submit-time validation and default scoping run through a `PolicyEngine` trait.
The first local policy preserves the existing CLI/API behavior: title, intent,
and at least one repo are required, and accepted tasks are assigned to the
default `local` organization. Dynamic configuration and richer policy precedence
can attach at this boundary without moving validation back into HTTP handlers.

Worker-driven state updates run through a `TaskLifecycle` trait. The initial
implementation keeps the local sequence (`created -> context_requested ->
context_ready -> running -> done|failed`) and centralizes summary output creation
plus failure persistence outside the worker loop. Future user, integration, or
admin-driven transitions should attach to this lifecycle authority rather than
mutating task state ad hoc. Each lifecycle transition also appends a
`state_changed` task record with `from` and `to` states so task history is
replayable instead of only materialized in the current task row. Worker-driven
state changes include the responsible `action_id`.

Harness attempts are represented as sessions in the task record stream. The
first local implementation generates a `SessionId` (`sess_...`) when the harness
starts and appends `session_started`, `session_completed`, or `session_failed`
records. `session_started` records include the launching `action_id` plus the
`context_id`, `node_id`, and `runtime_handle` for the prepared context where the
attempt ran. Session state is not normalized yet; the record stream is the
durable history for the local slice. Summary `output_created` records include
the producing `session_id`, preserving output provenance back to the exact
harness attempt.

Worker task ownership is represented by a `ClaimManager` trait. The first
implementation is an in-memory single-daemon claim map, matching the local worker
loop behavior. Durable leases, heartbeats, expiry, and takeover rules should
replace this implementation without changing worker orchestration. The local
worker appends `task_claimed` and `task_released` records with a generated claim
ID and node ID so ownership history is visible even before leases are durable.

Worker action selection is represented by an `ActionQueue` trait. The first local
queue scans for `created` tasks and claims the first available one, matching the
current embedded worker. Durable action records, priorities, retries, capability
filters, and observation-driven requeueing should replace this implementation
without changing task processing. Locally selected work is assigned an `ActionId`
(`action_...`) and recorded with `action_started`, `action_completed`, or
`action_failed` task records that include claim and node metadata. Failed action
records include the concrete task-processing error for debugging.

Generated context files, harness logs, and large generated data are written
through an `ArtifactStore` trait. The first implementation is local filesystem
storage in each task workspace: generated prompts live under `prompts/`, while
context manifests, summaries, and protocol logs live under `artifacts/`. Harness
runs return summary text plus generated artifact pointers, so protocol-level logs
such as OpenCode ACP NDJSON/stderr can be registered with the task. The store
boundary is intentionally small so later backends can add checksums, retention,
encryption, or remote object storage without changing task output records.
Artifact pointers are also mirrored into append-only `artifact_created` task
records so current artifact state can be rebuilt alongside outputs and
projections. Artifact records include source provenance: context-generated
artifacts point at the `context_id`, while harness and summary artifacts point at
the producing `session_id`.
