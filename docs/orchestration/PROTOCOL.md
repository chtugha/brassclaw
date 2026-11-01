# Orchestrator Coordination Protocol

## Overview

The **orchestrator** (root agent) is the only agent that runs `cargo` commands.
Sub-tasks communicate with the orchestrator exclusively through two files:

- `docs/orchestration/INBOX.md` — sub-tasks write requests here (build, review, clarification)
- `docs/orchestration/OUTBOX.md` — orchestrator writes results, assignments, and decisions here

## Polling contract

Every active sub-task **MUST** poll `OUTBOX.md` every 10 seconds for messages
addressed to its task-id. A sub-task may only proceed past a `WAIT_FOR_BUILD`
or `WAIT_FOR_REVIEW` gate after the orchestrator posts a matching `RESULT` entry.

## INBOX.md message format

Each message is a fenced block. Append — never overwrite existing entries.

```
## MSG-<nnn> [TASK-<id>] <TYPE> <timestamp>

<body>
```

Types:
- `BUILD_REQUEST` — ask orchestrator to run clippy/fmt/test on a crate
- `REVIEW_REQUEST` — ask orchestrator to spawn a review sub-task
- `CLARIFICATION` — ask orchestrator a question
- `DONE` — sub-task reports completion; include a summary of changes

BUILD_REQUEST body fields (one per line):
```
CRATE: brassclaw_reborn_composition
FEATURES: postgres,skills-db
TEST_FILTER: (optional regex)
REASON: short human description
```

## OUTBOX.md message format

```
## OUT-<nnn> → [TASK-<id>] <TYPE> <timestamp>

<body>
```

Types:
- `ASSIGNMENT` — new work package with instructions
- `BUILD_RESULT` — outcome of a BUILD_REQUEST (PASS / FAIL + log excerpt)
- `REVIEW_RESULT` — outcome of a REVIEW_REQUEST (PASS / FINDINGS)
- `ANSWER` — response to CLARIFICATION
- `STOP` — sub-task is no longer needed

BUILD_RESULT body:
```
REF: MSG-<nnn>
STATUS: PASS | FAIL
ERRORS: (if FAIL — paste first 40 lines of compiler errors)
```

## Orchestrator rules

1. Check INBOX.md before starting any new sub-task.
2. Process all pending inbox messages before dispatching new work.
3. Never start two `cargo` runs concurrently — check `/private/tmp/brassclaw-target-cargo.lock`.
4. After posting a BUILD_RESULT, immediately check INBOX for the next item.
5. Commit after every green build cycle (fmt + clippy + relevant tests).
6. Use `screen -dmS build-<slug>` for all cargo invocations.

## Sub-task rules

1. Poll OUTBOX.md every 10 seconds for your TASK-id.
2. Never modify source files outside the files listed in your ASSIGNMENT.
3. Never run `cargo` — post a BUILD_REQUEST instead.
4. When finished, post DONE to INBOX.md with a summary.
5. One BUILD_REQUEST at a time — wait for BUILD_RESULT before next request.

## Active task registry

See `docs/orchestration/TASKS.md` for the live task list and their assigned files.
