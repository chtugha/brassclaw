# Acknowledged catalogue source-export capacity — 2026-10-25

> **26 October evidence correction:** the historical 530-file worker-reuse
> inventory did not cover all actual build dependencies. Observed outcomes and
> immutable receipts remain, but full unchanged-worker source applicability is
> unproven. See [the exact limitation and scoped current requalification](2026-10-26-prefix-worker-evidence-review.md).

**Scoped production-consumer acceptance passed.** `catalogue_source_export.rs`
now reads the existing acknowledged `execution_limits.max_catalogue_bytes` once
per source export. It uses `InstalledMontyCatalogue::preparation_policy()` and
the ordinary settings owner's effective view. The former independent fixed
64-MiB constant is removed. There is no new setting, table, cache or endpoint.
The existing Monty VM editor and `GET/PUT /api/settings/monty-vm` remain the single
configuration path for that field.

Generation syntax is validated before reading settings. The capture happens
before catalogue activation/SQL locks; one export retains that captured ceiling.
Once the runtime has bound its settings source, an unavailable effective owner
fails rather than substituting a compiled default. Existing pre-binding bootstrap
preparation retains its captured initial policy. Exact generation/revision/approval/source verification
and read-only repeatable snapshots remain in place.

The complete encoded JSON size is counted through a private `Write` sink instead
of allocating a second full serialized byte vector merely to measure it. Escapes
and UTF-8 bytes count as actually serialized. Overflow rejects the whole export;
no shorter inventory, clipped component, altered source or partial result returns.
Successful export data and its versioned transfer formats stay unchanged. The
same underlying owner implements ordinary and qualified-private-prefix scopes;
the new native boundary test exercises the ordinary scope.

## Actual validation

Receipt: `target/accepted-evidence/prefix-export-capacity-20261025/run-1/accepted.json`.
SHA-256: `22826ed2ad2d4612cb950b978bec738f5541e5cd99438a97882001f5096e34f3`.
Only one Rust file changed. Its exact bytes, the actual executable, serial runner,
worker and raw logs are retained in that directory. This package passed its first
validation run, without an intermediate source/test failure.

**Two selected Rust tests freshly pass**, with positive expected counts and no
ignored cases:

- The new test starts the ordinary runtime and actual PostgreSQL with the real
  live settings owner. Its complete serialized source export succeeds at the
  exact configured byte count and fails at one byte less. The failure remains
  with discretionary token budgets disabled and enabled. Restoring the original
  canonical ceiling returns byte-identical complete content. A real PostgreSQL
  trigger then rejects the attempted settings commit: persisted/effective
  revision and the export remain unchanged. The test checks the acknowledged
  policy and stored value, retains the same catalogue generation, and shuts the
  runtime down normally. It does not use a substitute settings store or runtime.
- The existing versioned-source test still preserves both exact selected revisions
  and rejects conflicting bytes/classes and forbidden ordinary-scope combinations.

Current strict Composition test lint with the affected features, default
Composition lint, formatting, whitespace and final source integrity pass.
The common target and serial lock were used. No extra model, provider, browser,
full release or private-prefix-scope native test is claimed for this package.

The exact prior worker is reused with 529 directly unchanged recorded source
inputs. The remaining `Cargo.lock` change is the already accepted addition of
existing Network test dependency edges outside the worker graph, independently
rechecked against the original lock and the current lock. That qualification and
its accepted Root receipt are preserved in `worker-lock-qualification.json` and
`worker-reuse-review.json`. This is explicitly not a claim that all 530 input
files have unchanged bytes, or that a new worker was built.

## Remaining boundaries

This is a byte-capacity/settings consumer repair. It is not the model's
`max_prefix_tokens` or `max_prompt_tokens`, and adds no copied model limits.
Native artifact, IPC, execution-data and other independent technical constraints
still apply at their existing owners. Passing this export check does not certify
that a later complete generation package or provider request fits those bounds.

Authoritative complete-library/registered-source coverage, more-than-1,000-record
capture/concurrent-activation acceptance, qualified Sources jobs/projection,
accepted model/design bindings, Generate, final semantic token guards, serving
and model/cache quality remain governed by the [Prefix plan](../plans/prefix_v3_upgrade.md#remaining-implementation-sequence--full-completion).
No approval/bootstrap exception or deployment was changed.
