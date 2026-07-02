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
3. `workctl task review <id>` — show the summary output plus the diff of each
   mounted repo in the task workspace.
4. Fix what the dogfood run surfaces. Repeat until the definition of done holds.

## Dogfood findings (2026-07-01)

1. **Blocker: generated prompts hardcode a summarize-only task.**
   `render_prompt` in `workd` appends "Summarize the checked-out repository...
   Do not modify files." to every prompt regardless of intent. workctl cannot
   currently delegate a task that *changes code* — the core product promise.
   The prompt should be driven by the submitted intent, with the summarize
   framing reserved for summary-style tasks (or dropped entirely).
2. Watch UX: the final `print_task` summary reprints text that already
   streamed live, duplicating output at the end of a watched run. Minor;
   consider suppressing the summary when it was already streamed.
3. Observation quality: tool calls stream as bare titles (`tool: read`).
   Including the target (file path) would make watching materially better.
4. The loop itself held up: records, artifacts, session provenance, and live
   streaming all behaved as designed on a real repo with the real harness.

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
