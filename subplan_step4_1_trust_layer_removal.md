# Subplan: Step 4.1 — Delete the Trust Layer

## Goal
Remove `SkillTrustLevel`, `trust` fields, `trust_rank()`, and all trust-based attenuation
from the codebase. After this step, `Validated == trusted` — all visible validated skills
are treated as fully trusted (their prompt content is included). This is the Phase 3 design.

## Scope (all files to change)

### Core types — `brassclaw_turns/src/run_profile/skill_context.rs`
- Delete `SkillTrustLevel` enum + `impl` block
- Remove `trust: SkillTrustLevel` field from `InstalledSkillSnapshot`
- Remove `trust: SkillTrustLevel` field from `PromptSkillContextMetadata`
- In `SkillContextService` logic block (line ~349): replace the `match entry.trust { ... }`
  with unconditional: always include full content (description + prompt_content if present)
- Remove `trust_rank()` function (line ~549)
- Remove `trust_rank(a.trust)` from sort in `SkillContextService`
- In snapshot digest (SHA-256): remove trust field from hashed bytes (line ~656)
- Remove `SkillContextError::TrustDataMissing` error variant (and any check that produces it)
- Update doc comments

### Instruction bundle — `brassclaw_turns/src/run_profile/instruction_bundle.rs`
- Remove `SkillTrustLevel` import
- Remove the `metadata.trust_level == SkillTrustLevel::Trusted.as_str()` check (line ~518)
  — always include the skill prompt content for visible skills

### Host — `brassclaw_turns/src/run_profile/host.rs`
- Remove reference to `SkillTrustLevel::Installed carrying prompt_content: None` pattern (line ~732)
  — this is a doc comment, update it

### Exports — `brassclaw_turns/src/run_profile/mod.rs`
- Remove `SkillTrustLevel` from re-exports (line ~119)

### Loop support — `brassclaw_loop_support/src/skill_context.rs`
- Remove `SkillTrustLevel` import
- Update the Trusted-assignment at line ~171 (now unconditional)

### Loop support — `brassclaw_loop_support/src/skill_bundle_context_source.rs`
- Remove any remaining Installed/Trusted branching
- Update "Phase 3: SkillTrust::Installed removed" comment (now complete)

### First-party ports — `brassclaw_first_party_extension_ports/src/skills.rs`
- Remove `SkillTrustLevel` import
- Remove `&& entry.trust == SkillTrustLevel::Trusted` filters (lines ~460, ~469)
  (in tests — also fix test assertions for trust-less snapshots)

### Tests — multiple
- `crates/brassclaw_turns/tests/skill_context_service_contract.rs`:
  Remove `trust: SkillTrustLevel::Trusted` and `trust: SkillTrustLevel::Installed` from snapshots
- `crates/brassclaw_reborn/tests/loop_driver_host.rs` (line ~3969):
  Update "Phase 3: SkillTrust removed" comment

## Steps

### Step 1 — Remove `SkillTrustLevel` from `skill_context.rs` (core change)
- Delete enum definition + `impl SkillTrustLevel`
- Remove `trust` field from `InstalledSkillSnapshot`
- Remove `trust` field from `PromptSkillContextMetadata` (if present)
- Replace `match entry.trust { Trusted => ..., Installed => ... }` with unconditional full-content path
- Remove `trust_rank()` function and its call in sort
- Remove trust bytes from SHA-256 digest
- Update `SkillContextError`: remove `TrustDataMissing` if not used for other purposes
  (check all match arms first)

### Step 2 — Update `instruction_bundle.rs`
- Remove `SkillTrustLevel` import
- Remove trust check in prompt assembly — always include

### Step 3 — Update `mod.rs` exports
- Remove `SkillTrustLevel` from re-exports

### Step 4 — Update `brassclaw_loop_support`
- `skill_context.rs`: remove import + Trusted assignment (now just always-trusted)
- `skill_bundle_context_source.rs`: remove any remaining trust branching

### Step 5 — Update `brassclaw_first_party_extension_ports/src/skills.rs`
- Remove import + trust filters

### Step 6 — Update all tests
- `skill_context_service_contract.rs`: remove trust fields from all snapshot entries
- `loop_driver_host.rs`: update comment

### Step 7 — Run clippy + tests
```bash
cargo clippy -p brassclaw_turns -p brassclaw_loop_support -p brassclaw_first_party_extension_ports -p brassclaw_reborn --all-targets --all-features -- -D warnings
cargo test -p brassclaw_turns -p brassclaw_loop_support -p brassclaw_first_party_extension_ports
```

### Step 8 — Mark checkup.md Step 4.1, commit and push

## Key invariant
After removal: ALL visible validated skills get their full prompt content.
The `Installed` path (description-only) no longer exists.
`fetch_for_consumer` (§4.3, already done) remains as the gate — if a skill
row has `05:validator` tag it won't reach the model at all; trust level is
irrelevant because all that reach `SkillContextService` are already validated.
