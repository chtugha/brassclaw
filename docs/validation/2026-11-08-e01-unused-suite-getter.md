# E-01 unused suite getter removal

Concrete Root Run 18 warning correction, prepared only in the package worktree
while Root's source remains frozen. Transfer only this correction commit after
Root releases its build freeze, with any other required compiler/native fixes.

Searched Composition sources for `.suite(`, `Context::suite` and `fn suite(`.
The only match was the private `Context::suite` definition. Checked direct
`claim.suites` consumers in the qualifier, execution and tests: they iterate the
retained suites or select the existing program directly and never call this
getter. Removed only the seven-line unused getter. No dummy use, suppression,
cfg change, workflow change or new scope.

Actual checks: rustfmt edition 2024 with `skip_children=true`, `git diff --check`,
SHA-256 generation and readback. No Cargo, DB, tests or Modal; no build acceptance
claim. Root Run 18 was still running when this correction was prepared.

The sibling manifest identifies the corrected qualifier source and supersedes
its entry in the historical Run 17 correction manifest. The existing exact-byte
claim hashes the qualifier source, so this reviewed package gets a new identity;
historical evidence is neither rewritten nor claimed for these changed bytes.
