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

## Development

This project uses a Nix flake for its development environment and package
outputs. With direnv installed:

```sh
direnv allow
```

Without direnv, enter the shell directly:

```sh
nix develop
```

Static pre-push checks are configured for `jj lint`. They intentionally run
formatters and static analysis for Rust and Nix code, not the full test suite.

The fail-safe release contract is deliberately two commands:

```sh
nix run .#release -- --version X.Y.Z --check
nix run .#release -- --version X.Y.Z
```

`--check` is non-mutating. A new release starts from a fresh empty jj
working-copy commit whose parent exactly matches local `main` and
`main@origin`. The local GitHub backend validates refs and the prepared tree,
then atomically publishes the annotated tag and `main` with a lease on the
observed remote ref. The two-binary (`workctl` and `workd`) artifacts are built
and checksum-verified by GitHub Actions only after the tag push. An operator
then verifies the build identities and sidecars, manually
publishes the four GitHub Release assets, and dispatches the Pages refresh by
following [`docs/release.md`](docs/release.md). Future releases are GitHub-only;
historical SourceHut downloads and `builds/release-linux-x86_64.yml` remain
archival. There are no release-stage bypass flags.

`jj lint` remains the broad pre-push suite. The release additionally runs the
evaluated help/documentation contract after the standard fmt, Clippy, and test
apps. Remote sandboxes cannot exercise every local-loop integration path (and
may reject build scripts on `noexec` filesystems), so apply-side local
validation and CI are authoritative; CI intentionally sets
`WORKCTL_SKIP_LOCAL_LOOP` to avoid hanging daemon health checks.

The shared release interface comes from the SHA-pinned
`github:averagechris/fleet` `lib.fleet.presets.rust` preset.

The dev shell also includes dependency-management and supply-chain tooling:

- `cargo audit` for RustSec vulnerability audits.
- `cargo deny` for dependency policy, licenses, bans, and source checks.
- `cargo machete` for finding unused dependencies.
- `cargo outdated` for finding available dependency updates.
- `cargo add` / `cargo rm` / `cargo upgrade` from `cargo-edit`.
- `cargo sort` for keeping dependency tables sorted.

## First local vertical slice

The first implementation pass provides a local-only end-to-end path:

1. Start the daemon and embedded worker loop:

   ```sh
   workd serve --bind 127.0.0.1:7878
   ```

2. Submit a repository-summary task through the CLI:

   ```sh
   workctl submit \
     --title "Summarize linear-cli" \
     --intent "Clone the repository and summarize its purpose, structure, commands, dependencies, and notable implementation details." \
     --repo git@git.sr.ht:~averagechris/linear-cli \
     --repo-name linear-cli \
     --harness opencode-acp \
     --wait
   ```

`workd` owns state under `${XDG_DATA_HOME:-~/.local/share}/workctl` by default.
Canonical state is stored in `workd.sqlite3` with normalized tasks, outputs,
records, and artifacts; per-task workspaces hold cloned repos, generated prompts,
and large artifacts. Each task gets redirected `HOME` / XDG / temp paths for
process-level isolation. The OpenCode harness symlinks host OpenCode config/data
into those redirected XDG paths when present so local credentials can be used
without copying secret files into the workspace. If the primary repo has a
`flake.nix`, the OpenCode ACP harness is started through
`nix develop <repo> --command opencode acp --cwd <repo>`; otherwise it is started
directly.

Inspect task outputs, records, and artifacts with:

```sh
workctl task outputs <task-id>
workctl task records <task-id>
workctl task artifacts <task-id>
```

The default test path uses `--harness fake-summary` so CI does not need network,
SSH, or LLM credentials. The real Sourcehut/OpenCode E2E is ignored by default
and can be run explicitly with `WORKCTL_E2E_OPENCODE=1 cargo test -- --ignored`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
