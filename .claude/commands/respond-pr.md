---
description: Respond to PR review comments — triage, plan fixes, implement after confirmation, push, and reply to reviewers
disable-model-invocation: true
allowed-tools: Bash(gh pr list:*), Bash(gh pr comment:*), Bash(gh api:*), Bash(gh repo view:*), Bash(git branch:*), Bash(git status:*), Bash(git add:*), Bash(git commit:*), Bash(git push:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*), Read, Edit, Write, Grep, Glob
argument-hint: "[pr-number (optional, auto-detects from branch)]"
---

# Review and Address PR Comments

Follow root `AGENTS.md`, [the development policy](../../docs/development-policy.md)
and [testing rules](../rules/testing.md), including central Cargo ownership.

## Step 1: Find the PR

If `$ARGUMENTS` is provided, use that as the PR number. Otherwise, detect the PR for the current branch:

```
gh pr list --head $(git branch --show-current) --json number,title,url --jq '.[0]'
```

If no PR is found, tell the user and stop.

## Step 2: Fetch all review comments

Resolve the repo owner and name:

```
gh repo view --json owner,name --jq '"\(.owner.login)/\(.name)"'
```

Fetch the full set of review comments (not issue-level comments):

```
gh api --paginate repos/{owner}/{repo}/pulls/{number}/comments
```

Also fetch the review summaries:

```
gh api --paginate repos/{owner}/{repo}/pulls/{number}/reviews
```

Deduplicate comments that appear multiple times (bots sometimes post the same finding under different IDs). Group by the actual issue being raised, not by comment ID.

## Step 3: Triage and plan

For each unique issue raised in the comments:

1. **Check if already addressed** - Read the current code at the referenced location. If a prior commit already fixed it, note it as "already resolved".
2. **Assess validity** - Determine if the comment identifies a real problem or is a false positive. Be honest about false positives but explain why.
3. **Classify severity** - Critical (security/data loss), High (bugs/broken behavior), Medium (correctness/robustness), Low (style/naming/nits).
4. **Plan the fix** - For each valid unresolved issue, describe the specific code change needed.

Present the plan as a table to the user:

| # | Issue | File:Line | Severity | Status | Planned Fix |
|---|-------|-----------|----------|--------|-------------|

Proceed when the user has authorized these fixes; otherwise obtain authorization
for the concrete proposed changes. Do not repeat an already granted approval.

## Step 4: Implement fixes

For the authorized fixes:

1. Implement each fix in the plan.
2. Review the connected diff and choose policy-required evidence for the affected
   callers/features. Reuse adequate existing regressions; add missing caller-level
   coverage and run relevant affected-package linting for Rust behavior. Real
   backend, migration, security and lifecycle claims need actual integration/
   production-path checks and applicable failures. Preserve component/seed gates.
   Route Cargo requests to its designated owner and respect source freezes. Fix
   causes before repeating failed/invalidated checks; stop when required checks
   pass. Ordinary prose needs links/consistency review and `git diff --check`.
   Report skipped/zero-case/unavailable checks and baseline blockers explicitly;
   they do not pass required acceptance.
3. Commit with a descriptive message referencing the PR review.
4. Push to the branch.

## Step 5: Reply to comments

For each comment addressed, reply on the PR with a short message stating what was fixed and the commit SHA. For false positives or already-resolved items, reply explaining why no change was needed.

## Rules

- Never guess at code you haven't read. Always read the referenced file and line before assessing a comment.
- Group duplicate comments (same issue reported by multiple bots) and reply to all of them.
- Do not make changes beyond what the review comments ask for. Stay focused.
- If a comment suggests a change you disagree with, present your reasoning to the user during the planning phase rather than silently ignoring it.
- Follow BrassClaw conventions: no `.unwrap()` in production code, use `crate::` imports, `thiserror` errors.
- All persistence uses PostgreSQL only — no libSQL; new store methods go into the `Pg*` type in the relevant crate.
