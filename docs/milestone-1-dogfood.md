# Milestone 1: dogfood a real delegated task (done 2026-07-01)

**Definition of done:** submit a real coding task against a real repo through
`workctl` with the `opencode-acp` harness, watch it work live, review the
result, and use the output — without reading daemon logs or spelunking the
workspace directory by hand.

All gaps closed:

1. `workctl task watch <id>` / `submit --watch` — polls the task and streams
   new records live; tails the harness protocol log for agent text and tool
   calls when locally visible; `--json` emits NDJSON.
2. Dogfood run — real opencode-acp task against this repo completed in ~124s
   with live watch output.
3. `workctl task review <id>` — summary output plus captured `repo-diff`
   artifacts (per-repo working-tree diffs with intent-to-add for new files).
4. Findings fixed in the same day (below).

## Findings and outcomes

1. **Blocker, fixed:** generated prompts hardcoded a summarize-only task
   ("Do not modify files"), so workctl could not delegate code changes at
   all. Prompts are now intent-driven. In the same pass the ACP permission
   callback was changed from cancelling every request to selecting an allow
   option — the execution context is the safety boundary, not per-tool-call
   approval.
2. **Fixed by a delegated task:** tool calls streamed as bare titles. The
   round-2 dogfood agent added location paths to tool-call labels; its diff
   was reviewed via `task review` and applied to this repo. Review caught a
   real defect (a sed-mangled raw string terminator) — validating the
   human-review step.
3. The loop held up: records, artifacts, session provenance, live streaming,
   diff capture, and review all behaved as designed on real tasks.

Open items carried to the Milestone 2 backlog: duplicate summary printing at
the end of a watched run; opencode `edit` tool misbehaving under the
redirected HOME/XDG environment.

Round 2 was the milestone's proof: *submit "Show tool-call targets in watch
output" → watch the agent edit workctl's own code live → review the captured
diff → apply it*. workctl's first shipped output was an improvement to
workctl.
