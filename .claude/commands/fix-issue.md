---
description: Fetch a GitHub issue, create a branch, research the codebase, plan the fix, implement with tests, and commit
disable-model-invocation: true
allowed-tools: Bash(gh issue view:*), Bash(gh repo view:*), Bash(git fetch:*), Bash(git checkout:*), Bash(git status:*), Bash(git branch:*), Bash(git add:*), Bash(git commit:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*), Read, Edit, Write, Grep, Glob
argument-hint: "<issue-number or github-issue-url>"
---

# Fix GitHub Issue

Read root `AGENTS.md`, [the development policy](../../docs/development-policy.md)
and the affected subsystem guides. Apply [testing rules](../rules/testing.md)
when selecting evidence and respect the designated central Cargo owner.

## Step 1: Resolve the issue

Parse `$ARGUMENTS` to extract the issue number:
- If it's a URL like `https://github.com/owner/repo/issues/42`, extract `42`.
- If it's a bare number, use it directly.
- If empty, stop and ask the user for an issue number.

Fetch the issue:

```
gh issue view {number} --json title,body,labels,assignees,comments,state
```

If the issue is closed, warn the user and ask if they still want to proceed.

## Step 2: Create a branch

Create a fresh branch off the latest main:

1. Fetch latest: `git fetch origin`
2. Detect default branch: `gh repo view --json defaultBranchRef --jq .defaultBranchRef.name`
3. Create and switch to a new branch: `git checkout -b fix/{number}-{short-slug} origin/{default-branch}`
   - `{short-slug}` is 3-5 words from the issue title, lowercase, hyphenated (e.g. `fix/42-idor-workspace-check`)

If the working tree has uncommitted changes, warn the user and stop. Do not stash or discard their work.

## Step 3: Understand the issue

Summarize the issue in 2-3 sentences. Identify:
- **What's broken or missing** (the symptom or feature request)
- **Acceptance criteria** (what "done" looks like, from the issue body or comments)
- **Constraints** (mentioned technologies, backward compatibility, performance requirements)

If the issue is unclear or ambiguous, list the open questions. These will be addressed during planning.

## Step 4: Research the codebase

Before planning, gather context:

1. **Find relevant code** - Search for files, functions, types, and patterns mentioned in the issue. Read them in full.
2. **Trace the flow** - If the issue is about a specific behavior, trace the code path from the entry point (route handler, CLI command, etc.) through to the relevant logic.
3. **Check existing tests** - Find tests related to the affected code. Understand what's already covered.
4. **Check for prior art** - Look for similar patterns in the codebase that solve analogous problems. Prefer consistency with existing patterns.

## Step 5: Plan the bounded fix

Scale the implementation plan to the change. Cover:

1. **Root cause** (for bugs) or **design approach** (for features)
2. **Files to modify** with specific descriptions of what changes in each
3. **New files** (if any) with justification for why they're needed
4. **Required evidence** - affected behavior, relevant failure/boundary cases,
   still-applicable existing coverage and checks needed for remaining uncertainty.
   Add focused caller-level regressions only where existing tests do not establish
   the claim; do not require a separate test for every changed function/path.
5. **BrassClaw-specific concerns**:
   - All persistence uses PostgreSQL only — no libSQL/dual-backend; new store methods go into the `Pg*` type in the relevant crate
   - No `.unwrap()` or `.expect()` in production code
   - Use `crate::` imports, not `super::`
   - Error types via `thiserror` in `error.rs`
6. **Migration or compatibility concerns** (if any)

Follow the project's CLAUDE.md guidance for architecture decisions.

Proceed within the user's existing authorization. Ask only when an unresolved
requirement or action needs their input; do not add a mandatory planning-mode or
approval round to an already authorized fix. Component activation gates remain.

## Step 6: Implement

For the authorized bounded fix:

1. Implement each change from the plan.
2. Complete necessary regression coverage and review the connected diff before
   executable checks.
3. Select the smallest sufficient batch under the development policy. Rust
   behavior needs caller-level regression evidence and relevant affected-package
   linting. Database, migration, security and lifecycle claims need their actual
   backend/production caller and relevant failures. Ordinary prose needs links/
   consistency review and `git diff --check`, not Cargo. Preserve component/seed
   packaging, integrity, behavior and approval requirements when affected.
4. Reuse valid evidence; route Cargo requests to the designated owner without
   starting an independent build or changing its source freeze/target. Diagnose
   and fix causes before repeating failed/invalidated checks. Stop after required
   checks pass; record skipped/zero-case/unavailable checks as unverified. Required
   PostgreSQL acceptance is not waived when the database is unavailable.

## Step 7: Commit and summarize

1. Commit with a descriptive message referencing the issue (e.g. `fix: prevent IDOR in function call outputs (#42)`).
2. Summarize what was done:
   - Files changed with line references
   - Tests added and what they cover
   - Any follow-up work or open questions
