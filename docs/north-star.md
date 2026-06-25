# North star

`workctl` makes delegated coding tasks first-class.

A task should be something you can hand off to an agent, run in an explicit
execution context, observe while it works, claim as a human, resume elsewhere,
review, and clean up automatically when complete.

## Product sentence

`workctl` is a Rust control plane for running and managing subagent-sized coding
tasks across users, organizations, repos, runtimes, and machines.

## First-class objects

- Task intent and lifecycle.
- Execution context: runtime, mounted repos, mounted secrets, prompt/context
  files, tools, resource limits, artifact paths.
- Session state: running, blocked, failed, completed.
- Ownership: leases and heartbeats for users, agents, workers, contexts, and
  actions.
- Task outputs: local changes, PRs, direct commits, patch emails, review
  summaries, release notes, or other project-specific outputs.
- Artifacts: prompts, logs, summaries, test output, context manifests, reports,
  snapshots.
- Cleanup: automatic archival/deletion when policy gates pass.

## Non-goals for the first implementation

- A generic distributed platform before task execution works.
- Fine-grained approval/rejection of each shell command or tool call.
- Treating git, jj, or any VCS as a task-level abstraction.
- Raw directory sync as the default handoff mechanism.
- Bolting on auth, tenants, or organizations later.
