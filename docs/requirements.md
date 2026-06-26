# Requirements

This document states baseline requirements and decisions.

## Control plane

- `workd` owns canonical state, migrations, runtime config, policy, leases,
  actions, audit events, observations, and artifact pointers.
- `workctl` talks to an authenticated/configured `workd`; normal CLI commands do
  not write the DB directly.
- Workers pull actions and report observations/artifacts. Servers should not need
  to SSH into workers.
- The control plane can run locally or remotely.
- Multi-user and tenant/organization support are built in from the start.
- Single-node personal use is the simple case: one user, one organization, local
  auth defaults.
- SQLite is acceptable when one `workd` owns the DB. Postgres remains available
  for deployments that need it. Never share a SQLite file across machines.
- The first implementation stores daemon-owned state as JSON task records under a
  local state directory. This is an implementation scaffold for the local
  vertical slice; replacing it with SQLite should not change CLI semantics.

## Dynamic config

- Runtime config is control-plane state and can be changed with `workctl`.
- Running workers should observe relevant config changes without redeploy when
  practical.
- Static files and Nix modules are bootstrap mechanisms only: defaults, server
  startup, secrets wiring, initial node registration.
- Config is scoped by organization/tenant, with policy-defined user/node/task
  overrides.

## Tasks

- Tasks are durable records of delegated development work.
- A task may involve one repo, many repos, or no repo-specific output.
- Repo flags and issue metadata are context hints, not a single-repo constraint.
- External issue/review state is observed or snapshotted; it must not silently
  replace task intent.
- A task may produce many outputs across mounted repos.

## Context preparation

- Context preparation turns a task into a ready execution environment.
- It allocates/prepares the runtime through an executor backend.
- It mounts/materializes required repos.
- It mounts/injects required secrets/API keys.
- It writes generated prompt/context files.
- It records artifact output locations, a runtime handle, and a context manifest.
- Tasks should not care whether context came from git, jj, an existing checkout,
  snapshot, volume, container image, or cluster resource.

## Harnesses and executors

- Harnesses drive agent protocols/tools: OpenCode, `pi`, ACP, other agents.
- Executors provide runtimes: process runtime, container, isolate, VM,
  Kubernetes, other backends.
- Harness and executor are independent. A harness starts inside a prepared
  execution context.
- First-pass executor support is `local-devshell`: per-task local directories
  with redirected `HOME`, XDG cache/config, temp paths, cloned repos, generated
  prompts, and artifact output. It uses `nix develop` for repos with `flake.nix`
  and a direct process otherwise.
- First-pass harness support includes a deterministic `fake-summary` test harness
  and an `opencode-acp` harness that drives `opencode acp` over newline-delimited
  ACP JSON-RPC.
- The first OpenCode harness mounts local OpenCode config/data by symlink when
  present. This is a local-dev convenience for credentials, not a final secret
  distribution model.

## Safety and cleanup

- The safety boundary is the execution context: mounted repos, writable paths,
  mounted secrets, available tools, network policy, resource limits.
- Do not start with per-command approval as the safety model.
- Process/dev-shell runtimes are useful for local claim/editor workflows but have
  weaker isolation than containers/isolates/VMs/pods.
- Completed-task resources are archived/cleaned automatically when deterministic
  policy gates pass.
- Active leases, protected resources, stale observations, or ambiguous state block
  automatic cleanup and may create attention.
