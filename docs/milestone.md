# Current milestone

## Milestone 1: dogfood a real delegated task

**Definition of done:** submit a real coding task against a real repo through
`workctl` with the `opencode-acp` harness, watch it work live, review the
result, and use the output — without reading daemon logs or spelunking the
workspace directory by hand.

Concretely, this session must be possible:

```text
workctl submit --title "..." --intent "..." --repo <real repo> --harness opencode-acp
workctl task watch <task-id>     # live progress: state changes, session events, harness output
workctl task review <task-id>    # summary + workspace diff, usable as a review surface
```

## Gap list (in order)

1. ~~`workctl task watch <id>`~~ — **done.** `task watch` (and `submit
   --watch`) polls the task and streams new records live; when the harness
   protocol log is visible on the local filesystem it also streams agent text
   and tool calls. `--json` emits records as NDJSON. Tailing the log file
   directly is a local-milestone convenience, not the final observation
   transport.
2. ~~Dogfood run~~ — **done (first pass, 2026-07-01).** Submitted a real
   opencode-acp task against this repo; full loop completed in ~124s with live
   watch output. Findings below.
3. ~~`workctl task review <id>`~~ — **done.** Shows the summary output plus
   captured `repo-diff` artifacts. Sessions now capture per-repo working-tree
   diffs (including new files) as artifacts after the harness runs. Diff
   contents are read from the local filesystem; remote artifact retrieval is a
   later transport question.
4. Fix what the dogfood run surfaces. Repeat until the definition of done holds.

## Dogfood findings (2026-07-01)

Round 2 (same day): submitted "Show tool-call targets in watch output" against
this repo with opencode-acp. The agent edited `crates/workctl/src/main.rs`,
the session captured a `repo-diff` artifact, `task review` rendered the diff,
and the patch was applied to the real repo. **The definition of done has been
exercised end to end.** Remaining polish items below.

1. ~~**Blocker: generated prompts hardcode a summarize-only task.**~~ —
   **fixed.** Prompts are now intent-driven: they describe the workspace and
   mounted repos, permit direct working-tree changes when the task asks for
   them, and reserve read-only behavior for analysis-style intents. In the
   same pass, the ACP permission callback now selects an allow option instead
   of cancelling every request — the execution context is the safety boundary,
   not per-tool-call approval — so agents can actually edit files.
2. Watch UX: the final `print_task` summary reprints text that already
   streamed live, duplicating output at the end of a watched run. Minor;
   consider suppressing the summary when it was already streamed.
3. ~~Observation quality: tool calls stream as bare titles.~~ — **fixed by a
   delegated task.** The round-2 agent added location paths to tool-call
   labels; its diff was reviewed via `task review` and applied. Review caught
   a real defect (a sed-mangled raw string terminator), validating the
   human-review step.
4. The loop itself held up: records, artifacts, session provenance, live
   streaming, diff capture, and review all behaved as designed on real tasks.
5. New (round 2): the agent's `edit` tool appeared to fail inside the
   prepared workspace and it fell back to `bash`+`sed`, which introduced the
   syntax error. Investigate why opencode's edit tool misbehaves under the
   redirected HOME/XDG environment.

## Working rules until this milestone is done

- **No new traits.** Ten seams exist with one implementation each. A new trait
  or a second implementation of an existing one requires a concrete feature in
  the gap list that cannot ship without it.
- **No new record kinds** unless a gap-list feature reads them.
- **Refactors ride along, never lead.** A refactor is only in scope while it is
  blocking a gap-list item, and it lands in the same change as the feature it
  unblocks.
- Every landed change should move a gap-list item or fix a dogfood-discovered
  bug. If a change does neither, it does not land.

## Why this milestone

The north star names seven verbs: hand off, run, observe, claim, resume,
review, clean up. Only the first two exist. Observation and review are the
minimum needed to actually *use* the tool on real work; dogfooding is what
replaces the refactor backlog with a product backlog. Claim/resume, durable
leases, cleanup gates, and remote deployment all stay parked until this
milestone is done.
