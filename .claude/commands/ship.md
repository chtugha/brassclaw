---
description: Assess change readiness using the current development policy, required acceptance checks, and applicable evidence
allowed-tools: Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*), Bash(git diff:*), Read, Grep, Glob
---

Assess the actual change against [the development policy](../../docs/development-policy.md)
and [testing rules](../rules/testing.md). This command does not itself deploy,
publish, merge or approve runtime components.

## Steps

1. **Establish scope:** inspect the connected diff, affected callers/features,
   subsystem acceptance and existing evidence. Use `$ARGUMENTS`, if supplied, as
   a test selector within that scope, never as shell code or a waiver of other
   required acceptance. Verify selectors actually execute the intended tests.

2. **Respect Cargo ownership:** only the designated leading chat runs Cargo,
   including fmt. Other chats submit bounded requests to that owner, preserving
   the source freeze, canonical target, serial execution and disk-space rules.
   Pending requests are unverified, not completed checks.

3. **Select sufficient checks:** ordinary prose needs links/consistency review and
   `git diff --check`; Rust behavior needs regression evidence through the actual
   caller and affected-package linting with relevant features. Database, migration,
   security and lifecycle changes need real backend/production-path acceptance
   including applicable failures. Component/seed/embedded changes retain their
   packaging, behavior, integrity and approval gates. Do not stack equivalent
   builds or impose a full all-feature suite on every edit.

4. **Review outcomes:** reuse only still-applicable evidence. Fix causes before
   repeating failed/invalidated checks. Fix introduced warnings; report unrelated
   baseline blockers separately. Skipped/ignored/zero-case tests, missing databases
   or other unavailable dependencies cannot satisfy required acceptance.

5. **Report readiness:** identify exercised paths/features, reused evidence,
   fresh pass/fail results and unverified/blocking checks. Stop validation once
   required applicable checks pass. Broad CI/release/platform and component
   approval gates remain authoritative; a focused local result does not certify
   full release or deployment readiness.
