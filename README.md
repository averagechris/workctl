# workctl

`workctl` is a Rust control plane for delegated software-development tasks.

It makes subagent-sized coding tasks durable, inspectable, transferable, safe to
run, and easy for humans to claim or review.

## Core shape

```text
workctl CLI -> workd control plane -> worker loops
                                  -> prepared execution contexts
                                  -> agent harnesses inside executor runtimes
```

The central object is a **task**. A task records intent, context requirements,
ownership, sessions, outputs, artifacts, cleanup policy, and handoff history.

## Main decisions

- Written in Rust.
- The CLI records intent through an authenticated `workd`; workers reconcile.
- `workd` may run locally or on a server.
- Multi-user, organization/tenant, auth, and membership are baseline concepts.
  A personal single-node setup is just one user in one organization.
- Dynamic runtime config lives in the control plane. Static/Nix config is only
  bootstrap.
- A task is not single-repo. Context prep can mount one repo, many repos, or no
  repo-specific source.
- Context prep allocates the runtime and mounts repos/secrets before a harness
  starts.
- Harnesses and executors are separate axes:
  - harness: OpenCode, `pi`, ACP, other agents;
  - executor: process runtime, container, isolate, VM, Kubernetes.
- Cleanup of completed-task resources is automatic when deterministic policy
  gates pass.

## Docs

- [North star](docs/north-star.md)
- [Requirements](docs/requirements.md)
- [Key concepts](docs/key-concepts.md)
- [Open questions](docs/open-questions.md)

These docs are intentionally concise. They are meant to guide contributors and
their agents as implementation begins, not to pre-design every subsystem.
