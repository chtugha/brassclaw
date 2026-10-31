# BUILD020 — checkpoint CI and integration repairs

The [machine receipt](2026-10-31-checkpoint-ci-repairs.json) records exact sources,
actual images, runtime, original logs and attempts. SHA-256: `fa2c25d18741999120cd6ee3aa7479065a2c9105b73d0ce5322611f8ccca85f6`.

The connected source compiled without diagnostics. **125 native cases,32 architecture
cases,24 tagged Monty cases and1 legacy case passed; zero ignored.** This includes
all60 model gateway callers, memory/FS/PG Auth custody, original Prefix SDK/policy
consumers, five root Subagent cases with five threads and the actual global
embedded/base/Sempai consumer. Connected and isolated strict Clippy passed with
`-D warnings`. Actual actionlint and33 exact rendered-workflow shell cases passed.

The public native Auth runner now uses the existing GateRef; its private UUID and
`native-auth-wait/1` bytes remain readable. Existing transport/fixture owners are
reused. Domain-specific atomic loops preserve overflow, saturation and ordering.
Native storage preparation precedes the original bounded Subagent initialization.
There is no production Rust-loop fallback or artificial timeout inflation.

The first continuation stopped at an incorrect private selector, corrected from
the actual compiled listing. The next found an old upstream lifecycle driver that
omitted the existing pool coordinator; its successor keeps that coordinator pending
across host failures and explicitly shuts it down after both workers. The
production orchestrator was unchanged. A real91% disk guard stopped the third
attempt; an empty Composition cleanup was insufficient. Required LLM package-only
cleanup followed image/worker verification. All original failures remain recorded;
no source rollback, blanket cleanup or warning suppression occurred.

The full map now records8,233 named inputs, including isolated-workspace tests
missing from the earlier8,190-entry Composition map. Unchanged compiled inputs,
images and passing cases were verified and reused. The actual companion worker
remains `227a4b2c…`. R3 had already passed the corrected tagged gate before stopping at the legacy
disk guard. R4 also repeated that fast tagged gate after recovery; its24 cases
are counted once. The original per-attempt evidence is preserved.

This is local scoped maintenance acceptance. The original GitHub checkpoint's
browser failure is addressed separately in BUILD021; a new GitHub execution and
platform/release gates remain required. General A/Pure/S/function approval,
PrefixFree/design/serving, K3–K7, migrations and full-plan completion remain open.
