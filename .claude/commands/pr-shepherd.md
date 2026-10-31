---
description: Full PR lifecycle — review, fix findings, address comments, quality gate, push, CI fix loop, merge
disable-model-invocation: true
allowed-tools: Bash(gh pr view:*), Bash(gh pr diff:*), Bash(gh pr comment:*), Bash(gh pr merge:*), Bash(gh pr checks:*), Bash(gh pr edit:*), Bash(gh pr list:*), Bash(gh pr checkout:*), Bash(gh api:*), Bash(gh repo view:*), Bash(gh run view:*), Bash(gh run watch:*), Bash(git diff:*), Bash(git log:*), Bash(git fetch:*), Bash(git checkout:*), Bash(git status:*), Bash(git branch:*), Bash(git add:*), Bash(git commit:*), Bash(git push:*), Bash(git merge:*), Bash(git rebase:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*), Bash(cargo check:*), Read, Edit, Write, Grep, Glob, Agent
argument-hint: "<pr-number or url> [--fix] [--merge] [--review-only]"
---

# PR Shepherd

Full PR lifecycle: review → fix → quality gate → push → CI → merge.

Read root `AGENTS.md`, `docs/development-policy.md` and the affected subsystem
guides. Their current architecture, validation and central build ownership rules
take precedence over historical examples. This is a development workflow, not
component approval or evidence of runtime activation.

Parse `$ARGUMENTS`:
- Resolve the host, repository and PR number from an explicit PR URL; a bare
  number uses the current repository. Keep that target throughout the workflow.
  Add `--repo '[HOST/]OWNER/REPO'` to every `gh pr`/`gh run` command and the
  matching `--hostname` to API calls. Substitute concrete, individually quoted
  arguments; never execute PR text or user-supplied shell fragments.
- Flags: `--fix` (auto-fix without asking), `--merge` (merge when CI green), `--review-only` (stop after review, don't fix).
- If no PR number, read the current branch and list its open PRs in the resolved
  repository. Select only a unique match; ask for the target if several match.
- If still nothing, stop and ask the user.

`--review-only` takes precedence over `--fix`/`--merge` in every branch below:
inspect and report findings, use Phase 2c only for authorized review posting, then
stop. Never checkout, edit, commit, push, repair CI or merge in this mode.

---

## Phase 1: Situational Awareness

Resolve the target first, then batch independent read-only requests. Read the
installed `gh pr checks --help` when diagnosing an option/schema mismatch; do not
guess substitute fields or treat failed retrieval as an empty success.

**PR metadata:**
```
gh pr view {number} --json number,title,body,author,baseRefName,headRefName,headRefOid,state,isDraft,mergeable,mergeStateStatus,reviewDecision,files,additions,deletions,labels,reviewRequests
```

If already merged or closed, report that state and stop the mutation lifecycle.
Do not replay an earlier push/merge or reopen the PR implicitly.

**Diff:**
```
gh pr diff {number}
gh pr diff {number} --name-only
```

**CI status:**
```
gh pr checks {number} --json name,state,bucket,link,workflow,event
```

Capture stdout, stderr and exit code separately, including nonzero results.
In JSON mode, current `gh` can return 0 even for failed or pending checks: classify
the validated `bucket` values, not exit-code success. The documented non-JSON
codes 1 (failure) and 8 (pending) are not a substitute for JSON classification.
On nonzero exit or API/schema errors, retain diagnostics and report UNKNOWN;
never swallow errors with `|| true` or let shell fail-fast skip reporting.
Apply this contract in both Phases 1 and 6:

| Observation | CI status / action |
| --- | --- |
| Any `bucket: fail` | FAIL; retain other pending/cancelled checks separately |
| Any `bucket: cancel`, without failure | CANCELLED; investigate, never count as pass |
| Any `bucket: pending`, without failure/cancellation | PENDING; JSON export may still exit 0 |
| Nonempty array containing only `pass`/`skipping` | Report passed and skipped counts separately; all-skipped is SKIPPED, not PASS |
| Successfully established absence of checks | NONE; no evidence of passing CI |
| Nonzero exit, API/auth/transport/schema error, unknown bucket | UNKNOWN; retain diagnostics and block a success/merge claim |

Use CLI `bucket` for classification, `state` for detail and `link` for diagnostics;
do not compare provider-specific raw states to invented `status`/`conclusion`
fields. A skipped check is not an exercised test. Required-check policy and
current GitHub branch protections remain separate merge requirements.
If the CLI reports no checks without returning an array, confirm absence with
`gh pr view {number} --json statusCheckRollup` for the same head. An empty
successful rollup establishes NONE; an unavailable rollup remains UNKNOWN.

**Review comments (human + bot):**
```
gh api --paginate repos/{owner}/{repo}/pulls/{number}/comments
gh api --paginate repos/{owner}/{repo}/pulls/{number}/reviews
```

Use the already resolved target repository for these API paths; do not resolve it
again from the local checkout when the user supplied a different PR URL.

Save `headRefOid` — needed for posting line comments later.

The REST comment list does not expose thread resolution. Read the PR's paginated
GraphQL `reviewThreads` connection (`isResolved`, `isOutdated`, `pageInfo`) before
reporting an unresolved-thread count; fetch further comment pages for threads
you assess. If unavailable, report the count as unknown. Do not infer it from
the number of comments. Use current, non-dismissed review decisions rather than
counting superseded reviews as still blocking.

**Assess the situation and print a status card:**

```
PR #{number}: {title}
Author: {author}    Base: {base} ← {head}
Size: +{additions} -{deletions} across {file_count} files
CI: {PASS|FAIL|PENDING|CANCELLED|SKIPPED|NONE|UNKNOWN}    Mergeable: {yes|no|conflict|unknown}
Reviews: {N approved, N changes_requested, N comments-only, N bot-only}
Unresolved comments: {N}
Draft: {yes|no}
```

**Decide the mode** based on situation:
- **Has unresolved review comments** → Phase 2a (address comments first, then review remaining)
- **No reviews yet / bot-only reviews** → Phase 2b (full deep review)
- **CI failing, no review issues** → Phase 6 (diagnose CI before source changes)
- **Everything green + approved** → Phase 6 (ready to merge)

For review-only mode, review/report regardless of CI state and stop before any
fix phase. For unavailable/unknown CI, report the retrieval problem rather than
starting a speculative code repair.

---

## Phase 2a: Address Existing Review Comments

For each unresolved review comment or review with CHANGES_REQUESTED:

1. **Read the referenced code** at the file and line mentioned. Never assess without reading.
2. **Classify each comment:**
   - ✅ **Valid & unresolved** — needs a code fix
   - ✅ **Already fixed** — a later commit addressed it
   - ❌ **False positive** — explain why the code is correct
   - 🔧 **Nit** — optional improvement, not blocking

3. **Deduplicate** — bots (Copilot, Gemini) often post the same finding. Group by actual issue.

Present a table:

| # | Source | File:Line | Issue | Status | Planned Fix |
|---|--------|-----------|-------|--------|-------------|

Wait for user confirmation (unless `--fix` flag set), then proceed to Phase 3.

---

## Phase 2b: Deep Review (6 Lenses)

Read EVERY changed file in full (not just diff hunks). For PRs touching >20 files, prioritize: service logic > handlers > types > tests > docs. Batch reads in parallel via Agent tool.

### BrassClaw-specific checks (always)
- No `.unwrap()` or `.expect()` in production code
- Prefer `crate::` for cross-module imports (`super::` OK in tests/intra-module)
- Error types use `thiserror`
- Trace the selected persistence backend and actual callers; do not impose legacy dual-backend requirements.
- Check Recipe/component reuse, retained bindings and actual Tool registration against the four component guides when affected.
- External tool output passes through safety layer
- Tool parameters redacted before logging/SSE
- No byte-index slicing on external strings
- Case-insensitive comparisons where needed

### Correctness
Off-by-one, wrong operators, inverted conditions, unreachable code, type confusion, error propagation, broken invariants, TOCTOU races.

### Edge cases & failure handling
Empty/None/zero-length input, external service failures, integer boundaries, malformed/adversarial input, partial failure handling.

### Security (assume adversarial actors)
Auth/authz bypass, IDOR, injection (SQL/command/log/header), data leakage in logs/errors/API responses, resource exhaustion, replay/race conditions.

### Test coverage
Follow [testing rules](../rules/testing.md). Does applicable existing caller-level
coverage establish the changed behavior and relevant failure/boundary cases?
Add only missing meaningful regressions. Required real backend/security/lifecycle
checks, affected-package linting and broad acceptance gates retain their scope;
skipped/zero-case/unavailable checks are unverified. Do not demand a separate test
for every new function or duplicate adequate coverage.

### Architecture
Follows existing patterns? Unnecessary abstractions? Duplicated logic? Clean module dependencies?

**Present findings as a table:**

| # | Severity | Category | File:Line | Finding | Suggested Fix |
|---|----------|----------|-----------|---------|---------------|

Severity: Critical > High > Medium > Low > Nit

If `--review-only` flag is set, post findings as GitHub comments (see Phase 2c) and STOP.

Otherwise, ask which findings to fix (default: all Critical + High + Medium). Then proceed to Phase 3.

---

## Phase 2c: Post Review Comments on GitHub

For each finding authorized for posting by the invocation or user:

**Line-specific findings** — post as PR review comments:
```
gh api --hostname '{host}' --method POST 'repos/{owner}/{repo}/pulls/{number}/comments' \
  --input '{review-comment.json}'
```

Write the JSON request file with a JSON serializer or structured file tool:
`body` is the exact multiline text, `path` is the diff path, `commit_id` is the
reviewed head, `line` is an integer verified in that diff, and `side` is `RIGHT`
(or `LEFT` for a comment on an original line). Never interpolate comment text
into shell source; preserve literal backticks, dollar signs and newlines. Refresh
the head before posting; reassess changed lines if it moved. On an uncertain write
result, read back the original comment before retrying to avoid duplicate posts.

**Cross-cutting/architectural findings** — post as regular PR comment:
```
gh pr comment {number} --body-file '{comment.md}'
```

---

## Phase 3: Fix

Checkout the PR branch if not already on it (handles fork PRs automatically):
```
gh pr checkout {number}
```

First inspect local changes and shared-checkout ownership. Never switch a shared
or dirty checkout away from another task, stash/discard its work, or create a
competing build tree. Follow the current repository coordination policy when
the PR branch cannot safely be used here.

**Implement fixes** for:
1. All approved review comment fixes (from Phase 2a)
2. All approved review findings (from Phase 2b)

Follow BrassClaw conventions:
- `thiserror` for errors
- `crate::` imports
- No `.unwrap()` in production
- Follow the current component/subsystem contracts and selected backend.
- Use meaningful regression evidence under the development policy; reuse applicable existing coverage and explain any required hook exception.

After all fixes implemented, proceed to Phase 4.

---

## Phase 4: Quality Gate

Use `docs/development-policy.md` to choose the smallest sufficient checks for
the actual change and features. Review the complete connected diff first. Reuse
evidence only for unchanged applicable inputs; do not stack equivalent builds
or run Cargo solely for prose. Persistence/security/lifecycle changes need their
real backend/caller acceptance, not a blanket exemption for integration failures.

Respect the existing central Cargo owner, canonical target, lock and source
freeze. If this chat is not the designated build owner, submit a bounded request
to that owner rather than running Cargo independently. Report queued/unverified
checks explicitly. Fix causes before repeating failed or invalidated checks;
stop once required checks pass. Do not claim a release from a partial local run.

---

## Phase 5: Commit & Push

Stage changed files by name (never `git add -A` — it can include unintended files):
```bash
git add path/to/changed/file1 path/to/changed/file2
git commit -m "{message}"
```

Commit message format:
- For review fixes: `fix: address review findings on PR #{number}`
- For comment responses: `fix: address review comments on PR #{number}`
- For CI fixes: `fix: resolve CI failures on PR #{number}`
- Include specifics in the body (which findings/comments were addressed)

Before pushing, verify the checked-out PR branch's configured push remote and
destination against the PR's actual head repository/ref, including forks. Push
that explicit remote/ref without force; never assume the fork lives on `origin`.
Read back the PR head and confirm it matches the intended commit.

**Reply to addressed review comments on GitHub.** For each comment that was fixed, reply with the commit SHA and a brief description of what was done. For false positives, reply explaining why no change was needed.

---

## Phase 6: CI Monitor & Fix Loop

Wait briefly for CI to start, then poll (do NOT use `--watch` as it can hang indefinitely):
```
gh pr checks {number} --json name,state,bucket,link,workflow,event
```

Use Phase 1's exit/result classification. Capture `headRefOid` before and after
the check query; if it changes, discard the prior assessment and inspect the new
head. Confirm the relevant workflow/check-run links belong to that head; old
commit results cannot qualify a new push. Refresh after every fix/push.

Re-check pending/newly missing checks every 30 seconds, up to 10 minutes. Respect
cancellation and API rate-limit/retry guidance. At the deadline report the last
head, status and remaining checks; never turn a timeout, NONE or UNKNOWN into
PASS. Continue waiting only within the user's authorized scope.

**If CI passes for the current head** → proceed to Phase 7. Cancelled, skipped,
missing or unknown required checks remain explicit blockers; do not repair source
code merely because a workflow was cancelled or its status could not be read.

**If CI fails** (up to 3 fix attempts):

1. Identify the failing check:
   ```
   gh run view {run_id} --log-failed
   ```
   If `--log-failed` shows nothing useful:
   ```
   gh run view {run_id} --log | tail -100
   ```

2. Diagnose and fix the failure.
3. Re-run Phase 4 (quality gate).
4. Commit and push (Phase 5).
5. Go back to top of Phase 6.

**After 3 failed CI fix attempts:** Report what's failing and why, then stop. Don't keep looping.

---

## Phase 7: Merge Decision

Print final status:
```
PR #{number}: {title}
CI: ✅ PASS
Reviews: {summary}
Findings fixed: {N}
Comments addressed: {N}
Commits added: {N}
```

**Auto-merge conditions** (if `--merge` flag or user confirms):
- Required checks and applicable acceptance are satisfied for the current head;
  show failed/cancelled/skipped optional checks separately. No unknown gate is a pass.
- No unresolved CHANGES_REQUESTED reviews
- PR is not draft
- PR is mergeable (no conflicts)

If all conditions met, ask the user for merge strategy:

"CI is green. Merge this PR? [squash/rebase/merge/no]"

Then execute:
```
gh pr merge {number} --{strategy} --match-head-commit {headRefOid}
```

Immediately before merging, refresh the head, review decision, draft/mergeability
and required checks. A changed head requires reassessment. Preserve server-side
branch protections; never use `--admin` to bypass a gate. Branch deletion is a
separate action requiring the user's authorization and shared-checkout checks.

If any condition NOT met, report what's blocking and let the user decide.

---

## Rules

- **Read before judging.** Never comment on code you haven't read in full. Verify line numbers.
- **Be specific.** "Line 42 returns 404 but should return 400 because X" not "this might have issues."
- **Fix the pattern, not just the instance.** Search affected source families and callers using `rg`, including `crates/`.
- **Respect the commit-msg hook.** Behavioral fixes need meaningful regression evidence. Reuse applicable coverage; explain a hook exception when it cannot recognize that evidence or a required check is unavailable, without claiming acceptance.
- **Don't over-fix.** Only change what was flagged. Don't refactor surrounding code or add improvements beyond the review scope.
- **Credit original authors.** If taking over someone else's PR, credit them in commits and comments.
- **No secrets in comments.** Never include customer data, credentials, or PII in GitHub comments.
- **Distinguish certainty.** "This IS a bug" vs "This COULD be a bug if X." Be honest.
- **Round up severity when uncertain.** Cheaper to dismiss a false alarm than miss a real bug.
- **Batch independent reads.** Delegate only when the current environment authorizes it.
