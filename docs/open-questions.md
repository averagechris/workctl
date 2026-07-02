# Open questions

Answer these when they become implementation blockers.

## Resolved decisions

- Initial Rust crate layout is a Cargo workspace with `workctl` (CLI), `workd`
  (control-plane daemon), and `workctl-core` (shared domain types and trait
  seams). Add storage, harness, executor, and integration crates only when their
  boundaries need real implementations.
- Initial local context preparation is behind an async `ContextPreparer` trait in
  `workd`; the first implementation is `LocalDevshellContextPreparer` and still
  records the prepared workspace through `ControlStore`.
- Initial local harness execution is behind an async `AgentHarness` trait in
  `workd`; the first implementation dispatches to the existing `fake-summary`
  and `opencode-acp` harness flows.
- Initial runtime preparation is behind an async `ExecutorBackend` trait in
  `workd`; the first backend is `LocalDevshellExecutorBackend`, which owns the
  per-task `HOME`, XDG, temp directories, and dev-shell command manifest.
- Initial artifact writes are behind an async `ArtifactStore` trait in `workd`.
  The first implementation is filesystem-backed and keeps prompt-adjacent
  artifacts under each task workspace's `artifacts/` directory.
- Initial repo materialization is behind an async `SourceMaterializer` trait in
  `workd`. The first implementation clones Git repos into the task workspace and
  honors optional checkout hints, matching the previous local-devshell behavior.
- Initial submit-time policy is behind an async `PolicyEngine` trait in `workd`.
  The local implementation preserves the current validation rules and assigns
  submitted tasks to the default `local` organization.
- Worker-driven state transitions and summary completion are behind an async
  `TaskLifecycle` trait in `workd`. The local implementation preserves the
  current state sequence and output-created record semantics while centralizing
  transition authority outside the worker loop.
- Worker claims are behind an async `ClaimManager` trait in `workd`. The first
  implementation is in-memory and single-daemon only, preserving existing claim
  behavior while leaving durable leases/actions as a later store-backed change.
- Local worker claim acquisition and release append `task_claimed` and
  `task_released` records with a generated claim ID and node ID. These are
  provenance records for the in-memory claim manager, not durable leases yet.
- Worker action selection is behind an async `ActionQueue` trait in `workd`. The
  first implementation scans for `created` tasks and claims the first available
  task, preserving the embedded worker behavior until durable action records are
  introduced.
- Locally selected work gets a generated `ActionId` and appends `action_started`,
  `action_completed`, or `action_failed` records with claim and node metadata.
  Failure records include the concrete task-processing error. These record the
  embedded worker's action history before actions become durable queue rows.
- Generated artifact pointers are mirrored into replayable `artifact_created`
  task records when context manifests and summary artifacts are registered.
- Generated prompts are written through the artifact store and registered as
  `prompt` artifacts, making task inputs inspectable through artifact records.
- Harness runs return generated artifact pointers in addition to summary text;
  the OpenCode ACP harness registers its protocol and stderr logs as task
  artifacts during summary completion.
- `input_received` records snapshot submitted task inputs, including title,
  intent, repos, harness, and executor spec, so the accepted request can be
  replayed without relying only on the materialized task row.
- Context manifests include a runtime handle. The first local-devshell handle
  uses kind `local-devshell` and the task workspace path as its backend-local ID.
- Context manifests include a generated `ExecutionContextId` with `ctx_` prefix,
  giving prepared contexts a stable control-plane identifier before execution
  contexts have their own normalized store table.
- Context manifests include the preparing node ID. The first local node ID is the
  static `local` value until node registration/configuration exists.
- Submitted tasks carry a submitter user ID. The first local user ID is the
  static `local` value until auth and user resolution exist; `input_received`
  records also snapshot this accepted user scope.
- Local lifecycle transitions append replayable `state_changed` task records so
  task history can reconstruct the worker-driven state path. Worker-driven state
  changes include the responsible `action_id`.
- Local harness attempts have generated `SessionId` values and append
  `session_started`, `session_completed`, or `session_failed` records. Sessions
  are still record-only until normalized session storage is needed.
- `session_started` records include the execution `context_id`, `node_id`, and
  `runtime_handle`, linking each harness attempt back to the prepared context it
  ran in.
- `session_started` records include the launching `action_id`, linking harness
  attempts back to the worker action selected by the local action queue.
- Summary `output_created` records include the producing `session_id`, linking
  durable outputs back to the harness attempt that produced them.
- `artifact_created` records include source provenance: context-generated
  artifacts reference `context_id`, while harness/summary artifacts reference
  the producing `session_id`.
- Context preparation appends a replayable `context_prepared` record containing
  the launching action ID, execution context ID, node ID, runtime handle, repos,
  paths, and manifest path. Execution contexts are still record/artifact-backed
  until normalized storage is needed.

## Core implementation

- CLI-to-`workd` protocol: **decided (M2)** — HTTP+JSON with bearer tokens;
  TLS terminates at a proxy/ingress; `workctl` refuses plaintext to
  non-loopback hosts. Revisit other transports only if this becomes a real
  limitation.
- Auth mechanism: **first pass done (M2)** — static config-declared
  `token:user:org` tokens; anonymous-local only for explicit opt-in or
  tokenless loopback binds. Issued/short-lived credentials, SSO, and mTLS
  remain open.
- Role model: org admins, members, service accounts, workers.
- Control-plane lifecycle: auto-start local `workd`, configured server, or both?
- Bootstrap config/state paths: XDG layout, Nix module shape, secrets wiring.
- Runtime config propagation: how do workers observe config changes?
- Streaming transport beyond offset polling: the artifact content API serves
  byte ranges that watch polls every 500ms; SSE/WebSocket push can replace
  polling if it proves too chatty.

## Context and execution

- How to locate and mount non-Git sources, existing checkouts, jj workspaces,
  snapshots, volumes, or multi-repo context bundles.
- How to inject secrets safely across process/container/isolate/VM/pod runtimes.
- Which harness to implement first: OpenCode, `pi`, ACP, or another.
- How to represent source/context anchors and non-exportable contexts.

## State and policy

- Exact multi-actor task state transition authority beyond the local worker:
  user intent, worker result, integration observation, and administrative action.
- Durable lease/action records beyond the local in-memory claim manager: expiry,
  heartbeat cadence, stale handling, and takeover precedence.
- Durable action queue shape: action kinds, priorities, retries, backoff,
  per-node capability filters, and observation-driven requeueing.
- Policy precedence beyond the local default: org defaults, user overrides, node
  capabilities, task overrides.
- Artifact sync beyond the local filesystem: server storage, object store, SFTP,
  checksums, retention, encryption.
- Snapshot format for explicit context snapshots.

## Avoid early complexity

- Distributed scheduling before action claiming is solid.
- Multiple executors before the trait seam is proven.
- Raw directory sync before explicit context sync and artifact handoff work.
- Rich dashboards before CLI workflows are clear.
- Fully normalized provider schemas before real queries demand them.
- Cleanup without deterministic gates, retention policy, and audit records.
