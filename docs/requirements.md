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
- `workd` must stay deployable in two styles with no lock-in, from the same
  flake: a NixOS module (systemd service) for personal servers, and an OCI
  image (ECR/helm-friendly) for Kubernetes deployments. Configuration works
  through files plus environment variables so both styles are ergonomic.
- TLS is terminated in front of `workd` (reverse proxy or ingress); clients
  refuse plaintext for non-loopback servers unless explicitly overridden.
- Multi-user and tenant/organization support are built in from the start.
- Single-node personal use is the simple case: one user, one organization, local
  auth defaults.
- SQLite is acceptable when one `workd` owns the DB. Postgres remains available
  for deployments that need it. Never share a SQLite file across machines.
- The first implementation stores daemon-owned state in SQLite under the local
  state directory. One `workd` owns this database; task workspaces and large
  artifacts remain filesystem-backed.
- Persistence is accessed through a `ControlStore` trait. SQLite is the first
  implementation, but the daemon should not depend on SQLite-specific APIs so a
  Postgres-backed store can replace it for multi-worker/server deployments.

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
- Task outputs must be durable control-plane records with enough body/metadata to
  publish them to external collaboration surfaces without rereading transient
  workspace files.
- External publication is a projection of stored inputs/outputs/artifacts. Linear
  comments, Slack threads, GitHub reviews, manual handoffs, or other targets are
  tracked by append-only task records such as projection requested/succeeded/
  failed, integration observed, and acknowledged.
- Current publication state should be rebuildable from records. Materialized
  delivery tables/views are allowed later for query speed but must not become the
  only source of truth.

## Provider credentials

- Inference/provider credentials (OpenRouter, Anthropic, and other agent
  backends) are per-user control-plane state. Each user configures their own
  credentials self-service; they are not global daemon configuration.
- Credentials are dynamically configurable without redeploying `workd`, and
  scoped by organization/user like other dynamic config.
- Context preparation injects the submitting user's provider credentials into
  the execution context, materialized in the harness's native format (e.g. an
  opencode `auth.json`). Secret values never appear in manifests, task
  records, artifacts, or logs.
- Storage must protect secrets at rest; the mechanism (encryption approach,
  KMS/agenix integration, rotation) is an open question.
- Bootstrap state: statically provisioned host-level credentials for the
  daemon's service user. This is temporary and must be replaced by the
  per-user store before any multi-user deployment.

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
- Cleanup must not remove the only copy of an output whose required projection or
  acknowledgement records have not been observed according to policy.
