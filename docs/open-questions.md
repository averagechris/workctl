# Open questions

Answer these when they become implementation blockers.

## Resolved decisions

- Initial Rust crate layout is a Cargo workspace with `workctl` (CLI), `workd`
  (control-plane daemon), and `workctl-core` (shared domain types and trait
  seams). Add storage, harness, executor, and integration crates only when their
  boundaries need real implementations.

## Core implementation

- CLI-to-`workd` protocol: Unix socket, HTTPS, JSON-RPC, gRPC, SSH tunnel?
- Auth mechanism: local defaults, tokens, mTLS, SSO, SSH tunnel?
- Role model: org admins, members, service accounts, workers.
- Control-plane lifecycle: auto-start local `workd`, configured server, or both?
- Bootstrap config/state paths: XDG layout, Nix module shape, secrets wiring.
- Runtime config propagation: how do workers observe config changes?

## Context and execution

- Initial `ContextPreparer` trait shape.
- How to locate/materialize/mount repos for common workflows.
- How to inject secrets safely across process/container/isolate/VM/pod runtimes.
- Which harness to implement first: OpenCode, `pi`, ACP, or another.
- Which executor backend best validates the model first.
- How to represent source/context anchors and non-exportable contexts.

## State and policy

- Exact task state transition authority: user intent, worker result, observation.
- Lease expiry, heartbeat cadence, stale handling, and takeover precedence.
- Policy precedence: org defaults, user overrides, node capabilities, task
  overrides.
- Artifact storage/sync: local directory, server storage, object store, SFTP,
  checksums, retention, encryption.
- Snapshot format for explicit context snapshots.

## Avoid early complexity

- Distributed scheduling before action claiming is solid.
- Multiple executors before the trait seam is proven.
- Raw directory sync before explicit context sync and artifact handoff work.
- Rich dashboards before CLI workflows are clear.
- Fully normalized provider schemas before real queries demand them.
- Cleanup without deterministic gates, retention policy, and audit records.
