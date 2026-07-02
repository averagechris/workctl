# Milestones

## Roadmap

1. **Milestone 1: dogfood a real delegated task** — done (2026-07-01). See
   `milestone-1-dogfood.md` for the record.
2. **Milestone 2: remote control plane** — current. Defined below.
3. **Milestone 3: isolated execution** — a second `ExecutorBackend` giving
   each task an isolated runtime. Decided in `adr/0001-executor-isolation.md`:
   OCI containers are the task runtime contract (`container` backend first,
   `k8s-job` second, VM-grade isolation via runtime selection where KVM
   exists).
4. **Milestone 4: Linear integration** — project task outputs to Linear
   comments, then react to Linear comments by creating tasks. Reaction to
   external triggers stays parked until isolation exists, because it means
   externally-initiated code execution.

Later milestones (unordered): claim/resume for humans, durable leases and
action queues, cleanup gates, additional integrations.

## Milestone 2: remote control plane

**Definition of done:** `workd` runs on a server. From a laptop with no shared
filesystem, this session works over the network with a token:

```text
workctl --server https://workd.example task submit ... --watch   # streams remotely
workctl task review <task-id>                                    # diffs fetched via API
workctl task list                                                # only tasks the token may see
```

and:

- A request without a valid token is rejected (401).
- A token scoped to one user cannot read another user's tasks (403/404).
- Watch streams records *and* harness output without reading the daemon's
  filesystem; review fetches diff content the same way.
- The same flake produces both a NixOS module (personal server) and an OCI
  image that runs unmodified under Kubernetes/helm (work). No lock-in to
  either deployment style.

## Gap list (in order)

1. ~~**Artifact content API.**~~ — **done.** `GET
   /tasks/{id}/artifacts/{position}/content?offset=N` serves artifact bytes
   from an offset (empty body when the file is not written yet). Harness
   protocol/stderr logs are pre-registered at session start via
   `AgentHarness::planned_artifacts` so observers can stream them while the
   session runs. `task watch` tails the log and `task review` fetches diffs
   exclusively through this endpoint; the CLI no longer reads the daemon's
   filesystem at all. Authorization checks attach when gaps 2–3 land.
2. ~~**Token auth.**~~ — **done.** `workd` accepts `--auth-token
   token:user:org` entries (or `WORKD_AUTH_TOKENS`); bearer tokens resolve to
   a user/org identity on every request. Anonymous requests resolve to the
   local identity only when explicitly allowed or when no tokens are
   configured on a loopback bind; non-loopback binds without tokens refuse to
   start. `workctl` sends `--token` / `WORKCTL_TOKEN`.
3. ~~**Authorization.**~~ — **done.** `PolicyEngine::task_visible` scopes task
   visibility by organization. List filters; get and artifact-content return
   404 for invisible tasks to avoid existence leaks. Submissions take their
   user/org from the authenticated identity.
4. ~~**TLS/transport stance.**~~ — **done.** HTTP+JSON stays; TLS terminates
   at a reverse proxy or ingress. `workctl` refuses plaintext HTTP to
   non-loopback servers unless `--insecure-http`.
5. ~~**Packaging.**~~ — **done.** Flake outputs `packages.workd`,
   `packages.workctl`, `packages.workd-image` (Linux OCI image for ECR/helm),
   and `nixosModules.workd` (hardened systemd service with DynamicUser,
   environment-file token config, and reverse-proxy TLS guidance). See
   `docs/deployment.md`.
6. **Remote dogfood run.** Deploy to the personal NixOS server, run the
   Milestone 1 workflow end to end from the laptop, and record findings here.
   Known risk to exercise: opencode credentials for the service user (the
   host-config symlink convenience does not exist on a fresh server).

## Working rules (carried forward)

- **No new traits** unless a gap-list item cannot ship without one. (Auth
  resolution should attach to the existing `PolicyEngine` seam or plain axum
  middleware before earning a new trait.)
- **No new record kinds** unless a gap-list feature reads them.
- **Refactors ride along, never lead.** The agent-suggested `workd` module
  split may ride along with gap work that touches `workd`, hunk by hunk — it
  must not become its own project.
- Every landed change moves a gap-list item or fixes a dogfood-discovered bug.

## Why this milestone

Everything in the long-term picture — pod/microvm executors, multiple clients,
Linear reactions — assumes the control plane is reachable over a network and
enforces identity. Milestone 1 deliberately leaned on a shared filesystem for
watch and review; those are the exact seams that must become API surface
before any other milestone can start. Auth cannot be bolted on later (north
star non-goal), so it lands with the first remote deployment, not after.

## Backlog from Milestone 1 dogfooding

- Watch UX: final summary reprints text that already streamed live; suppress
  when already streamed.
- Investigate why opencode's `edit` tool misbehaves under the redirected
  HOME/XDG workspace environment (agent fell back to `bash`+`sed` and mangled
  a raw string; caught in review).
