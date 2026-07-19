# Remove `BRASSCLAW_REBORN_PROFILE` — Path B (three independent knobs)

> **Path B:** Merge the profile-removal idea into `integrate-postgres.md` as a
> three-knob refinement. This document is the **pre-merge draft** — a sound,
> `integrate-postgres.md`-compatible design that can be reviewed before being
> folded into the integrate-postgres plan's phase sequence.
>
> **Compatibility note:** This plan assumes the `integrate-postgres.md` world:
> embedded Postgres is the default, libSQL is removed (Phase 6 of that plan),
> all config lives in `brassclaw_config`, and the secret ceremony
> (`brassclaw_secrets_master` + raw-key-on-disk vs passphrase-wrapped) is
> already specified in §4.4 of that plan. All env-var names, schema names, and
> section references below are aligned to `integrate-postgres.md` revision 21.
>
> **Status:** ✅ MERGED INTO `integrate-postgres.md` — all fourteen section updates applied.
> See `integrate-postgres.md` for the merged plan (§0, §1c, §4.4, §4.31, §7.1, §7.2,
> §7.3, §8.1, Phase 3 checklist, Phase 11). This file is now superseded.
>
> **Review revision 1:** All findings from the first cross-agent review addressed
> (C1 production shim warning, C2 ceremony check ordering invariant, I3
> SecureDefault non-local surprise, I4 Phase 4 decision gate, I5 Phase 1 step 4
> hard prerequisite, I6 empty-string vs unset bash semantics, M7 AGENTS.md
> unconditional updates, M8 to_event_store_profile fate, M9 profile list →
> runtime-profile list rename, M10 Phase 7 → Phase 8-9 correction).
> **Review revision 2:** All findings from the second cross-agent review addressed
> (C1 §7.3 systemd unit two-variant replacement, I1 event store guard
> replacement specified directly — §4.18 reference removed, I2 production shim
> relaxed — PG_URL not required, I3 local_runtime_profile.rs all 4 functions +
> error type enumerated, M1 --test-exit flag prerequisite note, M2
> to_event_store_profile fully-qualified return type).
> **Review revision 3:** All findings from codebase cross-reference review
> addressed (boot.rs RebornBootConfig.profile field removal, config_file.rs
> [boot].profile field removal, doctor.rs RebornDoctorReport.profile removal,
> run.rs:54 + mod.rs:155 + doctor.rs:16 + config/mod.rs:53 profile print removal,
> skills.rs profile rejection logic removal, config/init.rs template update,
> profile_contract.rs + doctor_contract.rs test removal, build_production_shaped()
> already-unreachable-from-CLI note, local_dev_host_tests.rs test call sites
> for deleted helpers, nonexistent composition/boot.rs reference corrected,
> smoke tests #19-23 added for config file/doctor/run/skills output changes).
> **Review revision 4:** Cross-reference against integrate-postgres.md revision
> 14. Updated revision reference (13 → 14). Added 3 missing entries to the
> "Files changed by integrate-postgres.md" table: (1) §0 line 251
> "production-only" phrase for BRASSCLAW_SECRETS_PASSPHRASE_FILE, (2) §4.4 line
> 731 "production only" phrase in the BRASSCLAW_SECRETS_PASSPHRASE_FILE
> description, (3) §4.4 lines 754-758 schema comment "local-dev: wrapped_key"
> / "production: wrapped_key" → ceremony-based wording. Fixed all stale line
> references in the table (§0 176→248, §1c 269→341, §1c 273→345, §4.4 672→744,
> §7.1 2122/2168→2358/2404, §7.3 2192→2428, §8.1 2281-2308→2516-2537, §8.1
> 2338→2574, Phase 3 checklist 2453/2454→2689/2690). Fixed stale event store
> guard section reference (§4.22 → §4.24 — §4.22 is "Budget gates" and §4.23
> is "Identities" in revision 14). Updated section update count (ten →
> fourteen — the original count was off by one even before the 3 new entries).
> All source code line references verified accurate against current codebase.
> **Review revision 5:** Cross-reference against integrate-postgres.md revision
> 17 (revisions 15-17 added §4.24-§4.30 and grew the file from 2658 to 4799
> lines). Updated revision reference (14 → 17). Updated all line numbers in
> "Files changed by integrate-postgres.md" table to revision-17 actuals (§0
> 248→615/618, §1c 341→771, §1c 345→775, §4.4 731→1345, §4.4 744→1358, §4.4
> 754-758→1368-1372, §7.1 2358→4053, §7.2 2404→4099, §7.3 2428→4123, §8.1
> 2516-2537→4322-4345, §8.1 2574→4430, Phase 3 4550/4551). Updated §8.1
> dry-run "current text" to "Steps 3-10" (was "Steps 3-8" — integrate-postgres.md
> extended migration steps in revisions 15-17). Updated event store guard
> section suggestion (§4.24 → §4.31 — §4.30 is "Path B chunk embedding", now
> the highest section).
> **Review revision 6:** Cross-reference against integrate-postgres.md revisions
> 18-21 (no new sections added; file remains 4799 lines; all line numbers in the
> "Files changed" table verified unchanged). Updated revision reference (17 → 21).
> Added event-store call-site sequencing note (rev 19 C2 / rev 20 C2 clarify that
> `RebornEventStoreConfig::Postgres { url }` is retired in Phase 4 and the
> `factory.rs:2536` call site is eliminated by Path B Phase 3 step 5 before or
> alongside Phase 4 — the call-site update note in the event store section is only
> applicable when Path B lands before integrate-postgres.md Phase 4).

---

## Problem Statement

`BRASSCLAW_REBORN_PROFILE` (4 values: `local-dev`, `local-dev-yolo`,
`production`, `migration-dry-run`) is a single enum that gates four orthogonal
concerns. `integrate-postgres.md` already adds a fourth concern (secret
ceremony) to the profile's responsibilities by making §4.4's ceremony selection
depend on "production profile vs local-dev profile." This plan retires the
profile enum entirely and replaces it with three independent knobs that are
each focused on one concern.

The user wants ONE binary that installs and runs identically for everyone — no
profile branching. Storage, security policy, and secret ceremony are
independent operator choices, not a single hard-coded bundle.

---

## Root Cause: One Enum Doing Four Jobs

`RebornProfile` / `RebornCompositionProfile` conflates four orthogonal concerns:

| Job | What it controls | Current coupling | Right mechanism |
|-----|-----------------|------------------|-----------------|
| **Storage shape** | In-memory vs libSQL vs Postgres | `local-dev` → in-memory/libSQL; `production` → durable | `BRASSCLAW_PG_URL` (already in integrate-postgres.md §1c) |
| **Security policy** | Approval gates, filesystem scope, isolation | `local-dev` → `LocalDev`; `local-dev-yolo` → `LocalYolo`; `production` → (unspecified, derived separately) | `BRASSCLAW_RUNTIME_PROFILE` → `RuntimeProfile` (12 variants, already exists) |
| **Secret ceremony** | Raw-key-on-disk vs passphrase-wrapped master key | `local-dev` → raw-key-on-disk; `production` → passphrase-wrapped (integrate-postgres.md §4.4) | `BRASSCLAW_SECRETS_PASSPHRASE_FILE` presence (already in integrate-postgres.md §1c) |
| **Migration mode** | Read-only schema validation run | `migration-dry-run` profile value | CLI flag `brassclaw migrate --dry-run` |

These four should be independent config knobs, not a single profile string.
`integrate-postgres.md` already defines two of the three replacement knobs
(`BRASSCLAW_PG_URL`, `BRASSCLAW_SECRETS_PASSPHRASE_FILE`) in its §1c bootstrap
tier — but still gates them behind `BRASSCLAW_REBORN_PROFILE`. This plan
completes the decoupling.

---

## Architecture of the Two Systems

### Layer 1 — RebornProfile / RebornCompositionProfile (TO BE REMOVED)

```
BRASSCLAW_REBORN_PROFILE env var
        ↓
RebornProfile (4 variants)               crates/brassclaw_reborn_config/src/profile.rs
        ↓ composition_profile()          crates/brassclaw_reborn_cli/src/runtime/mod.rs:541
        ↓
RebornCompositionProfile (5 variants)    crates/brassclaw_reborn_composition/src/profile.rs
        ↓ match arm in build_reborn_services()   crates/brassclaw_reborn_composition/src/factory.rs:531
        ├── Disabled                       → RebornServices::disabled()
        ├── LocalDev | LocalDevYolo        → build_local_dev()
        └── Production | MigrationDryRun   → build_production_shaped()
```

### Layer 2 — RuntimeProfile (TO BE KEPT ENTIRELY, elevated to primary)

```
BRASSCLAW_RUNTIME_PROFILE env var (NEW) → RuntimeProfile (12 variants)
        ↓ brassclaw_runtime_policy::resolve()
        ↓ inputs: DeploymentMode, RuntimeProfile, yolo_disclosure_acknowledged, org_policy
        ↓
EffectiveRuntimePolicy          crates/brassclaw_host_api/src/runtime_policy.rs
  • ApprovalPolicy: AskAlways | AskWrites | AskDestructive | Minimal | OrgPolicy
  • FilesystemBackendKind: ScopedVirtual → HostWorkspace → HostWorkspaceAndHome
  • ProcessBackendKind: None → LocalHost → TenantSandbox → OrgDedicatedRunner
  • NetworkMode: Brokered | DirectLogged | Direct | Allowlist
  • SecretMode: BrokeredHandles | ScrubbedEnv | TenantBroker | InheritedEnv | OrgBroker
  • AuditMode: LocalMinimal | Standard | OrgPolicy
```

**The runtime policy resolver is pure, fail-closed, and security-critical.
Nothing in it changes.** It becomes the *primary* security knob, no longer
hidden behind a profile string.

---

## Target State: Three Independent Knobs

```
BRASSCLAW_PG_URL (optional)           BRASSCLAW_RUNTIME_PROFILE (optional)
   ↓                                     ↓
   absent  → embedded Postgres           default: local_dev
   set     → external Postgres           resolve() → EffectiveRuntimePolicy
        ↓                                     ↓ (yolo needs --confirm-host-access)
        └─────────────────────────────────────→ build_local_dev()  (single code path)

BRASSCLAW_SECRETS_PASSPHRASE_FILE (optional)
   ↓
   absent  → raw-key-on-disk ceremony   (key at $REBORN_HOME/.secrets-master-key)
   set     → passphrase-wrapped ceremony (Argon2id unwrap at boot)
```

| Knob | Concern | Default | integrate-postgres.md ref |
|------|---------|---------|---------------------------|
| `BRASSCLAW_PG_URL` | Storage backend | absent → embedded PG | §1c, §2 |
| `BRASSCLAW_RUNTIME_PROFILE` | Security policy | `local_dev` | (new — replaces profile's policy arm) |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | Secret ceremony | absent → raw-key-on-disk | §1c, §4.4 |
| `brassclaw migrate --dry-run` | Migration validation | (CLI flag) | §8.1 (replaces `migration-dry-run` profile) |

One binary. Storage is determined by `BRASSCLAW_PG_URL`. Security policy is
determined by `BRASSCLAW_RUNTIME_PROFILE`. Secret ceremony is determined by
`BRASSCLAW_SECRETS_PASSPHRASE_FILE` presence. Migration dry-run is a CLI flag.

`BRASSCLAW_REBORN_PROFILE` is gone.

---

## What Is Preserved (Security Properties Unchanged)

All of the following remain byte-for-byte identical:

- `RuntimeProfile` enum and all 12 variants
- `brassclaw_runtime_policy::resolve()` — fail-closed resolver
- `yolo_disclosure_acknowledged` gate on the `ResolveRequest`
- `admin_approves_dedicated_yolo` gate for `EnterpriseYoloDedicated`
- `DeploymentMode` family gating (`Local*` only for `LocalSingleUser`, etc.)
- `validate_production_libsql_target()` — rejected (libSQL removed by
  integrate-postgres.md Phase 6; the SSL guard `enforce_remote_ssl_mode()`
  remains for `BRASSCLAW_PG_URL`)
- `enforce_remote_ssl_mode()` — rejects `sslmode=disable` Postgres URLs
- `OrgPolicyConstraints::max_profile` ceiling narrowing
- The `--confirm-host-access` CLI flag that maps to `yolo_disclosure_acknowledged`
- The §4.4 secret ceremony: `brassclaw_secrets_master` schema, Argon2id
  wrapping, raw-key-on-disk fallback, rewrap command, key-source invariant
- The §4.4 fail-closed: service refuses to boot if
  `brassclaw_secrets_master` has no row for the tenant AND no raw key file exists

---

## The Ceremony Derivation (replacing profile-branched ceremony)

`integrate-postgres.md` §4.4 currently selects the ceremony based on
`BRASSCLAW_REBORN_PROFILE`: `local-dev` → raw-key-on-disk, `production` →
passphrase-wrapped. Path B replaces this with **`BRASSCLAW_SECRETS_PASSPHRASE_FILE`
presence as the ceremony selector** — but with the DB row's `algorithm` column
as the source of truth at boot time.

### At migration / first-run time (§8.1 step 6 replacement)

The ceremony is **set up** based on whether the operator has provided a
passphrase file. "Set" means `BRASSCLAW_SECRETS_PASSPHRASE_FILE` is present in
the environment AND non-empty (an empty string is treated as "absent" — an
empty path is not a valid file path and is ignored):

- **`BRASSCLAW_SECRETS_PASSPHRASE_FILE` is set** → operator wants passphrase
  ceremony. If migrating from an existing raw key file
  (`.reborn-local-dev-secrets-master-key` exists) and
  `brassclaw_secrets_master` has no row: **do not auto-migrate**. Print:
  `"Run 'brassclaw secrets rewrap --strategy passphrase-file=<path>' before starting."`
  and exit non-zero. The operator runs rewrap interactively once, which writes
  `algorithm = 'aes256gcm-argon2id'`, then restarts. This is the same
  fail-closed as integrate-postgres.md §8.1 step 6 production branch — the
  only change is the **trigger** (passphrase-file presence, not profile string).

- **`BRASSCLAW_SECRETS_PASSPHRASE_FILE` is absent** → operator wants
  raw-key-on-disk. Copy `.reborn-local-dev-secrets-master-key` to
  `$REBORN_HOME/.secrets-master-key` (0600), upsert
  `brassclaw_secrets_master(tenant, 1, '', 'raw-key-on-disk')`. Both
  `wrapped_key` and `algorithm` are explicitly set (same as integrate-postgres.md
  §8.1 step 6 local-dev branch). Zero and delete the old file.

### At boot time (ongoing, §4.4 boot path replacement)

> **Ordering invariant:** This check runs **after** the schema runner
> (integrate-postgres.md Phase 1) has created `brassclaw_secrets_master` AND
> after either the boot migration (§8.1 step 6) or the first-run wizard (§6.1)
> has populated the row. On a completely fresh install where neither has run
> yet (no pre-existing files, `boot.initialized` absent), the ceremony check
> is **skipped** — the first-run wizard will set up the ceremony. The check
> only fires once a `brassclaw_secrets_master` row exists for the boot tenant.

The ceremony is **expected** based on the DB row's `algorithm` column, and the
passphrase file must be consistent. "Set" means present in the environment AND
non-empty (same semantics as migration-time — an empty string is "absent"):

- **`algorithm = 'aes256gcm-argon2id'`** → passphrase ceremony. **Require**
  `BRASSCLAW_SECRETS_PASSPHRASE_FILE` to be set (present and non-empty). If
  not set: fail with
  `"Master key is passphrase-wrapped but BRASSCLAW_SECRETS_PASSPHRASE_FILE is not set. Set the env var or run 'brassclaw secrets rewrap --strategy raw-key' to revert."`
  If set: read passphrase, unwrap key, proceed.

- **`algorithm = 'raw-key-on-disk'`** → raw-key ceremony. Read key from
  `$REBORN_HOME/.secrets-master-key`. If `BRASSCLAW_SECRETS_PASSPHRASE_FILE`
  is also set (present and non-empty): warn
  `"BRASSCLAW_SECRETS_PASSPHRASE_FILE is set but master key is not wrapped. The file will be ignored. Run 'brassclaw secrets rewrap --strategy passphrase-file=<path>' to switch to passphrase ceremony."`
  and proceed with raw-key. (Warn, not fail — the DB row is the source of
  truth; a stale env var should not block boot.)

### At rewrap time (§4.4 rewrap, unchanged)

The operator explicitly chooses the target ceremony via `--strategy`:
`--strategy raw-key` reverts to raw-key-on-disk; `--strategy passphrase-file=<path>`
wraps with passphrase. Rewrap writes the new `algorithm` value to the DB row.
This is already specified in integrate-postgres.md §4.4 and does not change.

### Why this is safe

The DB row's `algorithm` column is the **single source of truth** for which
ceremony is in effect at boot time. The passphrase-file env var is the
**required input** for the passphrase ceremony. This is stricter than the
profile-based model: a profile string could be changed at boot time without
touching the DB, but the algorithm column can only be changed by `rewrap`
(which requires the old key). An operator who flips `BRASSCLAW_REBORN_PROFILE`
from `production` to `local-dev` today gets a silent ceremony mismatch; under
Path B, the boot path checks algorithm-vs-env-var consistency and fails closed
on mismatch.

---

## The Fail-Closed Guard (replacing profile-branched storage guard)

`integrate-postgres.md` currently fails closed for `production` profile if
durable storage is absent. Path B replaces this with a **RuntimeProfile-based
guard**:

```
if RuntimeProfile.is_local() == false AND BRASSCLAW_PG_URL is unset:
    fail: "Non-local runtime profile '{profile}' requires BRASSCLAW_PG_URL.
           Embedded Postgres is for single-host local deployments only.
           Set BRASSCLAW_PG_URL to an external Postgres URL or use a local
           runtime profile (local_dev, local_safe, local_yolo)."
```

The `is_local()` predicate already exists at
`crates/brassclaw_host_api/src/runtime_policy.rs:212` and uses an exhaustive
match (so new variants force a compile-time decision). The local variants are:

- **`LocalSafe`** — cautious local coding mode (yes, usable without PG_URL —
  it's a local profile with ask-on-write/ask-on-shell)
- **`LocalDev`** — default local coding-agent mode (the default)
- **`LocalYolo`** — trusted-laptop mode (requires `--confirm-host-access`)

All other variants are **non-local** and require `BRASSCLAW_PG_URL`:
- **`SecureDefault`** — yes, `SecureDefault` is non-local (`is_local()` returns
  `false`). Despite its name, it describes "scoped virtual filesystem, sandbox
  or disabled process, brokered network/secrets" — a hosted/enterprise helper
  shape, not a local one. An operator who sets
  `BRASSCLAW_RUNTIME_PROFILE=secure_default` without `BRASSCLAW_PG_URL` gets
  the fail-closed error.
- **`HostedSafe`, `HostedDev`, `HostedYoloTenantScoped`** — hosted multi-tenant
- **`EnterpriseSafe`, `EnterpriseDev`, `EnterpriseYoloDedicated`** — enterprise
- **`Sandboxed`, `Experiment`** — helper-process / disposable modes

Embedded PG on a single host is not appropriate for any non-local deployment.

This is **stricter** than the original profile-based guard:
- The profile guard checked `profile == Production`, which is a string the
  operator sets. An operator who forgets `BRASSCLAW_REBORN_PROFILE=production`
  gets silent local mode.
- The Path B guard checks `!RuntimeProfile.is_local()`, which is a property of
  the resolved security policy. An operator who sets
  `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` but forgets `BRASSCLAW_PG_URL` gets
  a clear fail-closed error. An operator who wants local mode with embedded PG
  just uses the default (`local_dev`) — no fail-closed.

**Interaction with the ceremony:** The ceremony guard and the storage guard
are independent. An operator can use:
- `local_dev` + embedded PG + raw-key-on-disk (local development)
- `local_dev` + external PG + raw-key-on-disk (single-host production, no ceremony)
- `local_dev` + external PG + passphrase-wrapped (single-host production with ceremony)
- `hosted_safe` + external PG + passphrase-wrapped (multi-tenant production)
- `hosted_safe` + external PG + raw-key-on-disk (multi-tenant, key on app host —
  acceptable since DB breach alone can't expose secrets; ceremony is
  defense-in-depth, not a hard requirement)

The only **blocked** combination is non-local RuntimeProfile + no PG_URL
(fail-closed). All other combinations are valid operator choices.

**Warning (not fail) for non-local + no passphrase file:** If
`RuntimeProfile.is_local() == false` AND `BRASSCLAW_SECRETS_PASSPHRASE_FILE`
is unset AND `algorithm = 'raw-key-on-disk'`: warn
`"Non-local runtime profile with raw-key-on-disk secrets. Consider running 'brassclaw secrets rewrap --strategy passphrase-file=<path>' for defense-in-depth."`
This is a recommendation, not a hard requirement — raw-key-on-disk on the app
host is not a security hole (the key and the ciphertext are on different hosts
when PG is external).

---

## The Deprecation Shim (old → new knob translation)

During the deprecation phase (Phase 2 below), `BRASSCLAW_REBORN_PROFILE` is
still accepted but emits a warning and translates to the new knobs:

| Old profile value | `BRASSCLAW_RUNTIME_PROFILE` | `BRASSCLAW_PG_URL` | `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | Behavior |
|---|---|---|---|---|
| `local-dev` | `local_dev` (default) | unchanged | unchanged | identical to current |
| `local-dev-yolo` | `local_yolo` | unchanged | unchanged | identical to current (yolo gate still enforced by resolver) |
| `production` | unchanged (leave at default/config-derived) **but warn** | **not required** — embedded PG is durable in the integrate-postgres.md world | unchanged | boots with embedded PG (or external PG if `BRASSCLAW_PG_URL` is set) + default `local_dev` policy + deprecation warning + runtime-profile warning. If `BRASSCLAW_RUNTIME_PROFILE` is explicitly set to a non-local value AND `BRASSCLAW_PG_URL` is unset, the fail-closed guard fires (not the shim). |
| `migration-dry-run` | n/a | n/a | n/a | **error:** `"BRASSCLAW_REBORN_PROFILE=migration-dry-run is removed. Use 'brassclaw migrate --dry-run' instead."` |

**Note on `production` translation:** The `production` profile in the current
code is primarily about storage shape (durable) + fail-closed, not about a
specific `RuntimeProfile`. The security policy in production is derived
separately (from config or the local_runtime_profile mapping). The deprecation
shim therefore does **not** synthesize a `BRASSCLAW_RUNTIME_PROFILE` value for
`production` — it leaves the runtime profile at its default (`local_dev`) or
config-derived value.

**Why `BRASSCLAW_PG_URL` is NOT required by the shim:** In the
integrate-postgres.md world, the default storage when `BRASSCLAW_PG_URL` is
unset is embedded Postgres, which IS durable (a real PG instance). The old
`production` profile's fail-closed was about rejecting in-memory/filesystem
storage — but in the integrate-postgres.md world, in-memory is `#[cfg(test)]`
only and filesystem storage is removed. Embedded PG is always durable. An
operator running `production` with embedded PG (single-host production) should
NOT be broken by the shim. The fail-closed for non-local profiles (e.g.
`hosted_safe` without `BRASSCLAW_PG_URL`) is handled by the **new guard**
(`!is_local() && pg_url.is_none()`), not by the shim. The shim's only
hard requirement is the deprecation warning + the runtime-profile warning.

**The shim MUST emit a warning when `BRASSCLAW_REBORN_PROFILE=production`
is detected AND `BRASSCLAW_RUNTIME_PROFILE` is not set:**
```
"WARNING: BRASSCLAW_REBORN_PROFILE=production is deprecated and no longer
implies a security policy. Defaulting to BRASSCLAW_RUNTIME_PROFILE=local_dev.
Set BRASSCLAW_RUNTIME_PROFILE explicitly for your deployment tier
(e.g., local_safe for single-host production, hosted_safe for multi-tenant,
enterprise_safe for org-dedicated)."
```
This avoids a silent security regression: without the warning, an operator
who was relying on `production` to signal "I want production-grade security"
would silently get `local_dev` (the developer default) with no indication
that they need to choose a runtime profile. The warning is not a fail —
the service still boots (with PG_URL set) — but the operator is explicitly
told they need to set the new knob.

---

## Files to Change

### Files to Delete

| File | Reason |
|------|--------|
| `crates/brassclaw_reborn_config/src/profile.rs` | `RebornProfile` enum and env-var parsing |

### Files to Modify

| File | Change |
|------|--------|
| `crates/brassclaw_reborn_config/src/lib.rs` | Remove `pub mod profile;` and re-exports of `RebornProfile`, `REBORN_PROFILE_ENV` |
| `crates/brassclaw_reborn_config/src/boot.rs` | Remove the `profile: RebornProfile` field from `RebornBootConfig`; remove `profile()` accessor; remove `from_env()` reading of `REBORN_PROFILE_ENV`; remove `resolve_from_env_parts()` `profile: Option<OsString>` parameter; update `into_parts()` to return only `RebornHome` (or remove if no callers remain). The compiler will enumerate all `config.profile()` call sites. |
| `crates/brassclaw_reborn_config/src/config_file.rs` | Remove the `boot.profile` field from the `RebornConfigFile` boot struct (line ~671 validation + line ~1102 read); remove the `check(Cow::Borrowed("boot.profile"), profile)?` validation; the `deny_unknown_fields` attribute on the boot struct will reject any operator TOML that still has `profile = "..."` — add a migration note in `config init` |
| `crates/brassclaw_reborn_config/src/doctor.rs` | Remove `profile: RebornProfile` field from `RebornDoctorReport`; remove `profile()` accessor; update `from_config()` to not call `config.into_parts()` for the profile (only extract `home`). The doctor command and config path command both print `report.profile()` — see CLI entries below |
| `crates/brassclaw_reborn_config/tests/profile_contract.rs` | Remove or rewrite: the entire file tests `RebornProfile` parsing (`from_env_value`, `from_str`, `InvalidProfile` error). These tests are obsolete once `RebornProfile` is deleted. |
| `crates/brassclaw_reborn_config/tests/doctor_contract.rs` | Remove `report.profile() == RebornProfile::MigrationDryRun` assertion (line 16) — update to not assert on profile (the `RebornDoctorReport` no longer has a `profile()` accessor). If the test's only purpose was to verify the profile field, remove the test entirely. |
| `crates/brassclaw_reborn_composition/src/profile.rs` | Collapse to `{ Disabled, Active }` (remove `LocalDevYolo`, `Production`, `MigrationDryRun` variants); remove `requires_production_shape()`; simplify `to_event_store_profile()` to always return `brassclaw_reborn_event_store::RebornProfile::LocalDev` as a constant-return stub (it is called at `factory.rs:2536`; do NOT delete it yet — keep the stub until the event store guard is reworked per the "Event store guard replacement" section below, then delete the function and inline the constant at the call site) |
| `crates/brassclaw_reborn_composition/src/factory.rs` | Remove `Production \| MigrationDryRun → build_production_shaped()` branch from `build_reborn_services()` (line 536-538); remove `build_production_shaped()` function (line 2087); `build_local_dev()` becomes the only active code path (the storage shape is determined by `RebornStorageInput`, which is already URL-derived). **Note:** `build_production_shaped()` is already unreachable from the CLI — both `run` and `serve` go through `local_runtime_build_input_with_options()` (mod.rs:413) which rejects non-local profiles before the `RebornBuildInput` reaches the factory. The function is only reachable from `build_reborn_services()` when a caller constructs a `RebornBuildInput` with `profile: Production` directly (tests only). This means the removal risk is lower than it appears — the function is already dead code from the CLI's perspective. |
| `crates/brassclaw_reborn_composition/src/input.rs` | Remove `profile: RebornCompositionProfile` param from `libsql()`, `postgres()`, `local_dev()` constructors; the profile field on `RebornBuildInput` becomes `RebornCompositionProfile::Active` (or is removed entirely if no code reads it after the factory match is collapsed) |
| `crates/brassclaw_reborn_cli/src/runtime/mod.rs` | Remove `composition_profile()` (line ~541); remove `effective_profile()` (line ~655); remove `print_runtime_banner()`'s `profile:` line (line 155 — `config.profile()` is removed); add `runtime_profile_from_env()` reading `BRASSCLAW_RUNTIME_PROFILE`; add `pg_url_from_env()` reading `BRASSCLAW_PG_URL` (already specified in integrate-postgres.md §1c); construct `ResolveRequest` directly; add the fail-closed guard (`!is_local() && pg_url.is_none() → fail`); add the ceremony-consistency check at boot (algorithm vs passphrase-file presence); remove `use brassclaw_reborn_config::{REBORN_PROFILE_ENV, RebornProfile}` import (line 19) |
| `crates/brassclaw_reborn_cli/src/commands/` | Add `migrate --dry-run` flag to migrate subcommand; Phase 2 repurposes `profile list` to show `RuntimeProfile` values (12 variants) + the three new env vars; Phase 3 **renames** the command from `profile list` to `runtime-profile list` (the `crates/brassclaw_reborn_cli/src/commands/profile.rs` file is renamed to `runtime_profile.rs`; the `ProfileCommand`/`ProfileSubcommand`/`ProfileListCommand` types are renamed accordingly; the old `profile` subcommand is removed) |
| `crates/brassclaw_reborn_cli/src/commands/skills.rs` | Remove `build_skill_list_config()`'s call to `effective_profile()` (line 94) and the `match profile { LocalDev \| LocalDevYolo => {}, Production \| MigrationDryRun => bail!() }` rejection (lines 95-102); the skills command no longer gates on profile — it works with any `RuntimeProfile` (the runtime policy is resolved upstream in `build_services_input_with_options`). Remove the `profile: RebornProfile` field from `SkillListConfig` (line 89) and the `"profile"` JSON field in skill output (line 53). Remove `use brassclaw_reborn_config::RebornProfile` import. |
| `crates/brassclaw_reborn_cli/src/commands/run.rs` | Remove `println!("profile: {}", config.profile())` (line 54) — the `config.profile()` accessor is removed with `RebornBootConfig.profile`. Replace with `println!("runtime_profile: {}", resolved_profile.as_str())` using the `RuntimeProfile` resolved from `BRASSCLAW_RUNTIME_PROFILE`, or remove the line entirely if the banner in `mod.rs:155` covers it. |
| `crates/brassclaw_reborn_cli/src/commands/doctor.rs` | Remove `println!("profile: {}", report.profile())` (line 16) — `RebornDoctorReport.profile()` is removed. Replace with `println!("runtime_profile: {}", ...)` using the resolved `RuntimeProfile`, or remove the line. |
| `crates/brassclaw_reborn_cli/src/commands/config/mod.rs` | Remove `println!("profile: {}", report.profile())` (line 53) in `ConfigPathCommand::execute()` — same reason as doctor.rs. |
| `crates/brassclaw_reborn_cli/src/commands/config/init.rs` | Remove `profile = "local-dev"` (line 149) and the comment `# Composition profile. One of: local-dev, local-dev-yolo, production, migration-dry-run.` (line 146) from the `config init` template. The `[boot]` section no longer has a `profile` field — `deny_unknown_fields` on the boot struct will reject any operator TOML that still has it. Add a comment noting the migration: `# [boot].profile removed — use BRASSCLAW_RUNTIME_PROFILE env var instead.` |
| `crates/brassclaw_reborn_event_store/src/lib.rs` | Remove the `profile: RebornProfile` parameter from `build_reborn_event_stores()` (line 149); remove all three `RebornProfile::Production` branches (lines 154, 166, 195); the event store's own `RebornProfile` enum (line 91: `LocalDev`, `Test`, `Production`) has `Production` removed — see "Event store guard replacement" section below for the complete specification |
| `crates/brassclaw_reborn_composition/src/local_runtime_profile.rs` | Migrate all four public functions + private helper + error type from `RebornCompositionProfile` to `RuntimeProfile`: `local_runtime_build_input()` (line 24), `local_runtime_build_input_with_options()` (line 39), `local_dev_runtime_policy()` (line 53), `local_dev_yolo_runtime_policy()` (line 68), `local_runtime_policy()` (line 85), and `RebornLocalRuntimeProfileError::UnsupportedProfile` (line 12). The `LocalDevYolo → RuntimeProfile::LocalYolo` mapping is removed (the caller passes `RuntimeProfile` directly via `BRASSCLAW_RUNTIME_PROFILE`). See Phase 3 step 9 for the full per-function specification. |
| `crates/brassclaw_reborn_cli/src/runtime/mod.rs` (boot path — ceremony check) | The ceremony-consistency check is added to the boot path in `build_services_input_with_options()` or the serve command's boot sequence: read `brassclaw_secrets_master.algorithm`, compare to `BRASSCLAW_SECRETS_PASSPHRASE_FILE` presence, fail/warn per the ceremony derivation section above. This is already covered by the `mod.rs` row above (the ceremony check is part of the boot path changes in that file). Listed separately here for discoverability — do not create a separate file. |
| `AGENTS.md` | Update Key Environment Variables table — remove `BRASSCLAW_REBORN_PROFILE`; add `BRASSCLAW_RUNTIME_PROFILE`, `BRASSCLAW_PG_URL`, and `BRASSCLAW_SECRETS_PASSPHRASE_FILE` unconditionally (integrate-postgres.md has not landed yet, so none of these are in AGENTS.md today) |
| `crates/brassclaw_reborn_cli/tests/smoke.rs` | Update `profile_list_shows_supported_profiles_without_reborn_home` (lines 68–136); replace `skills_list_rejects_unsupported_profiles` (line ~332) with a test using `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` + no `BRASSCLAW_PG_URL` → fail-closed error |

### Files changed by `integrate-postgres.md` that need post-merge updates

These are sections in `integrate-postgres.md` itself that reference
`BRASSCLAW_REBORN_PROFILE` and would be updated when this plan is merged:

| integrate-postgres.md section | Current text | Path B replacement |
|---|---|---|
| §0 guiding principles (line 615) | bootstrap tier list includes `BRASSCLAW_REBORN_PROFILE` | Remove `BRASSCLAW_REBORN_PROFILE`; add `BRASSCLAW_RUNTIME_PROFILE` to the list |
| §0 guiding principles (line 618) | `BRASSCLAW_SECRETS_PASSPHRASE_FILE` described as "production-only; needed to unwrap the master key" | Change "production-only" to "ceremony-dependent: set when master key is passphrase-wrapped (see §4.4); absent for raw-key-on-disk ceremony" |
| §1c bootstrap tier table (line 771) | `BRASSCLAW_REBORN_PROFILE` — Boot profile — `local-dev` | `BRASSCLAW_RUNTIME_PROFILE` — Security policy — `local_dev` |
| §1c bootstrap tier table (line 775) | `BRASSCLAW_SECRETS_PASSPHRASE_FILE` — "production profile only — see §4.4" | `BRASSCLAW_SECRETS_PASSPHRASE_FILE` — "ceremony selector: present → passphrase-wrapped, absent → raw-key-on-disk (see §4.4)" |
| §4.4 ceremony selection | "local-dev profile → raw-key-on-disk; production profile → passphrase-wrapped" | "passphrase-file absent → raw-key-on-disk; passphrase-file present → passphrase-wrapped; boot path checks algorithm-vs-env consistency" |
| §4.4 `BRASSCLAW_SECRETS_PASSPHRASE_FILE` description (line 1345) | "`BRASSCLAW_SECRETS_PASSPHRASE_FILE` (bootstrap tier, production only) is the required unattended-boot path" | "`BRASSCLAW_SECRETS_PASSPHRASE_FILE` (bootstrap tier, ceremony-dependent) is the required input for passphrase-wrapped unattended boot" |
| §4.4 boot fail-closed (line 1358) | "refuses to boot in production profile if no row and no raw key" | "refuses to boot if no row and no raw key" (drop "in production profile" — the guard is now ceremony-based, not profile-based) |
| §4.4 schema comment (lines 1368-1372) | `-- local-dev: wrapped_key = '' AND algorithm = 'raw-key-on-disk'` / `-- production: wrapped_key = base64(...), algorithm = 'aes256gcm-argon2id'` | `-- raw-key-on-disk ceremony (passphrase-file absent): wrapped_key = '' AND algorithm = 'raw-key-on-disk'` / `-- passphrase-wrapped ceremony (passphrase-file present): wrapped_key = base64(...), algorithm = 'aes256gcm-argon2id'` |
| §7.1 fresh-install step 3 (line 4053) + §7.2 step 2 (line 4099) | `secrets.env` unconditionally writes `BRASSCLAW_SECRETS_PASSPHRASE_FILE` (in both §7.1 step 3 and §7.2 step 2) | Make that line conditional in both sequences: only write it if the operator ran `rewrap --strategy passphrase-file=...`. Operators using raw-key-on-disk ceremony (no rewrap) must omit this line to avoid spurious boot warnings. Both sequences also need a ceremony-choice branch point: step 2 (rewrap) becomes optional — operators who want raw-key-on-disk skip rewrap entirely. |
| §7.3 systemd unit (line 4123) | `Environment=BRASSCLAW_REBORN_PROFILE=production` | See "§7.3 systemd unit replacement" section below — two complete unit variants (single-host + multi-tenant) |
| §8.1 step 6 (lines 4322-4345) | "local-dev profile: copy key, upsert raw-key-on-disk" / "production profile: require rewrap" | "passphrase-file absent: copy key, upsert raw-key-on-disk" / "passphrase-file present: require rewrap" |
| §8.1 migration-dry-run (line 4430) | "migration-dry-run profile: Steps 3–10 run in read-only simulation mode" | "`brassclaw migrate --dry-run`: Steps 3–10 run in read-only simulation mode" (CLI flag, not profile) |
| Phase 3 checklist (line 4550) | "Per-boot unwrap: read `BRASSCLAW_SECRETS_PASSPHRASE_FILE` at serve startup (production only)" | "Per-boot unwrap: ceremony-selector — absent → raw-key-on-disk, present → passphrase-wrapped; boot path checks algorithm-vs-env consistency (see ceremony derivation section)" |
| Phase 3 checklist (line 4551) | "Fail-closed in production profile if master key absent AND no raw key file AND no passphrase file" | "Fail-closed if master key absent AND no raw key file AND no passphrase file" (drop "in production profile") |

### §7.3 systemd unit replacement (complete)

The one-line table entry above is insufficient — replacing
`BRASSCLAW_REBORN_PROFILE=production` with `BRASSCLAW_RUNTIME_PROFILE=hosted_safe`
would **break the unit** because `hosted_safe` is non-local and the fail-closed
guard requires `BRASSCLAW_PG_URL` (which is commented out in the current unit
for embedded-PG deployments). The §7.3 unit must be rewritten with two
documented variants:

> **`BRASSCLAW_PG_URL` is REQUIRED when using a non-local RuntimeProfile**
> (`hosted_*`, `enterprise_*`, `secure_default`, `sandboxed`, `experiment`).
> It is optional only for `local_*` profiles. Embedded Postgres is durable
> storage but is designed for single-host local deployments only.

#### Variant 1 — Single-host with embedded Postgres (local profile)

```ini
# /etc/systemd/system/brassclaw.service — single-host with embedded PG
[Unit]
Description=BrassClaw Reborn Agent
After=network.target

[Service]
Type=simple
User=brassclaw
WorkingDirectory=/opt/brassclaw

# Bootstrap tier — non-secret; safe as inline Environment=
Environment=BRASSCLAW_REBORN_HOME=/var/lib/brassclaw
Environment=BRASSCLAW_RUNTIME_PROFILE=local_safe
Environment=BRASSCLAW_REBORN_LOG=brassclaw=info
# BRASSCLAW_PG_URL is OPTIONAL for local_* profiles — omit to use embedded Postgres:
# Environment=BRASSCLAW_PG_URL=postgresql://brassclaw@127.0.0.1:5434/brassclaw
# Optional — override embedded PG port if 5434 is taken:
# Environment=BRASSCLAW_EMBEDDED_PG_PORT=5435

# Operator-trusted tier (secrets + identity values, never inline) — read by
# systemd as root. File must be root:root 0600.
# Contents: BRASSCLAW_SECRETS_PASSPHRASE_FILE (path to brassclaw-readable file,
#   set ONLY if you ran 'brassclaw secrets rewrap --strategy passphrase-file=...'),
#   BRASSCLAW_REBORN_WEBUI_TOKEN, BRASSCLAW_REBORN_WEBUI_USER_ID, API keys.
EnvironmentFile=/etc/brassclaw/secrets.env

ExecStart=/usr/local/bin/brassclaw serve
Restart=on-failure
RestartSec=5

# Hardening — the full block below is unchanged from integrate-postgres.md §7.3
# and MUST be inlined verbatim in the merged document (do not leave it as a comment stub).
# NOTE: MemoryDenyWriteExecute=yes requires jit=off in postgresql.conf (§2.2).
# If you remove MDWE, also remove jit=off — see §2.2 for the tandem-change note.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
# AF_INET covers TCP to 127.0.0.1:5434 (embedded PG); AF_UNIX covers PG unix sockets if used.
RestrictNamespaces=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
# SystemCallFilter=@system-service covers the baseline. PostgreSQL may need
# additional syscalls (clone, mmap, semget, setrlimit, ioprio_*). If the
# hardened-unit integration test (Phase 10) shows PG requires a syscall outside
# @system-service, extend with that specific syscall rather than weakening the
# filter globally (e.g. SystemCallFilter=@system-service semget).
SystemCallFilter=@system-service
# /etc/brassclaw is read-only to the service (ProtectSystem=strict covers it).
# secrets.env is read by systemd-manager (root) via EnvironmentFile= and
# injected as environment — the service process never opens /etc/brassclaw.
ReadWritePaths=/var/lib/brassclaw
CapabilityBoundingSet=
AmbientCapabilities=
LimitNOFILE=4096
TasksMax=512

[Install]
WantedBy=multi-user.target
```

**Why `local_safe` and not `local_dev`:** `local_safe` is the cautious
single-host production choice (ask-on-write, ask-on-shell). `local_dev` is the
developer default (ask only on dangerous actions). Operators who want the
developer-grade approval policy can use `local_dev` instead. Both are local
profiles and work with embedded PG.

#### Variant 2 — Multi-tenant with external Postgres (non-local profile)

```ini
# /etc/systemd/system/brassclaw.service — multi-tenant with external PG
[Unit]
Description=BrassClaw Reborn Agent
After=network.target

[Service]
Type=simple
User=brassclaw
WorkingDirectory=/opt/brassclaw

# Bootstrap tier — non-secret; safe as inline Environment=
Environment=BRASSCLAW_REBORN_HOME=/var/lib/brassclaw
Environment=BRASSCLAW_RUNTIME_PROFILE=hosted_safe
Environment=BRASSCLAW_REBORN_LOG=brassclaw=info
# BRASSCLAW_PG_URL is REQUIRED for non-local profiles (hosted_*, enterprise_*,
# secure_default, sandboxed, experiment). Omitting it triggers fail-closed.
Environment=BRASSCLAW_PG_URL=postgresql://brassclaw@db.internal:5432/brassclaw
# Optional — override embedded PG port (not used when PG_URL is set):
# Environment=BRASSCLAW_EMBEDDED_PG_PORT=5435

# Operator-trusted tier (secrets + identity values, never inline) — read by
# systemd as root. File must be root:root 0600.
# Contents: BRASSCLAW_SECRETS_PASSPHRASE_FILE (REQUIRED if master key is
#   passphrase-wrapped — run 'brassclaw secrets rewrap --strategy passphrase-file=...'
#   once before first boot),
#   BRASSCLAW_REBORN_WEBUI_TOKEN, BRASSCLAW_REBORN_WEBUI_USER_ID, API keys.
EnvironmentFile=/etc/brassclaw/secrets.env

ExecStart=/usr/local/bin/brassclaw serve
Restart=on-failure
RestartSec=5

# Hardening — the full block below is unchanged from integrate-postgres.md §7.3
# and MUST be inlined verbatim in the merged document (do not leave it as a comment stub).
# NOTE: MemoryDenyWriteExecute=yes requires jit=off in postgresql.conf (§2.2).
# If you remove MDWE, also remove jit=off — see §2.2 for the tandem-change note.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
# AF_INET covers TCP to 127.0.0.1:5434 (embedded PG); AF_UNIX covers PG unix sockets if used.
RestrictNamespaces=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
# SystemCallFilter=@system-service covers the baseline. PostgreSQL may need
# additional syscalls (clone, mmap, semget, setrlimit, ioprio_*). If the
# hardened-unit integration test (Phase 10) shows PG requires a syscall outside
# @system-service, extend with that specific syscall rather than weakening the
# filter globally (e.g. SystemCallFilter=@system-service semget).
SystemCallFilter=@system-service
# /etc/brassclaw is read-only to the service (ProtectSystem=strict covers it).
# secrets.env is read by systemd-manager (root) via EnvironmentFile= and
# injected as environment — the service process never opens /etc/brassclaw.
ReadWritePaths=/var/lib/brassclaw
CapabilityBoundingSet=
AmbientCapabilities=
LimitNOFILE=4096
TasksMax=512

[Install]
WantedBy=multi-user.target
```

**Why `hosted_safe` requires `BRASSCLAW_PG_URL`:** `hosted_safe` is a
multi-tenant profile (`is_local()` returns `false`). Embedded PG on a single
host is not appropriate for multi-tenant deployments — the fail-closed guard
enforces this. The operator must provide an external Postgres URL.

**Ceremony note for both variants:** `BRASSCLAW_SECRETS_PASSPHRASE_FILE` in
`secrets.env` is required only if the operator ran `rewrap --strategy
passphrase-file=...`. If the operator did not run rewrap (raw-key-on-disk
ceremony), omit this var. The boot path checks `brassclaw_secrets_master.algorithm`
for consistency (see ceremony derivation section).

### Event store guard replacement

> **Note:** integrate-postgres.md does NOT specify the event store guard
> replacement anywhere (§4.18 is "Hook predicate state," not event store).
> This section specifies it directly. If this plan is merged into
> integrate-postgres.md, this section should become a new subsection **§4.31**
> — the current highest section is §4.30 ("Path B chunk embedding system
> reuse") in revision 17. Do not use §4.24–§4.30, which are all taken.
> Alternatively, append to the event store inventory row in §1a.

The event store's `build_reborn_event_stores()` at
`crates/brassclaw_reborn_event_store/src/lib.rs:148` currently takes a
`profile: RebornProfile` parameter and has three `RebornProfile::Production`
branches:

1. **Line 154** — `InMemory` + `Production` → error `ProductionInMemoryDisabled`
2. **Line 166** — `Jsonl` + `Production` + no `accept_single_node_durable` → error `ProductionJsonlRequiresAcceptance`
3. **Line 195** — `Libsql` + `Production` → `validate_production_libsql_target()` (SSL check)

**Replacement design:** Remove the `profile: RebornProfile` parameter entirely.
The `RebornEventStoreConfig` variant itself is the guard — no profile string
needed:

- **`InMemory`**: Only constructable behind `#[cfg(test)]`. In serve mode,
  the caller never constructs this variant (the serve path always constructs
  `Postgres` or `Jsonl`). If somehow constructed in non-test code, the
  existing `InMemory` path returns the in-memory store without a profile check
  — the guard is that the caller must not construct it, enforced by the
  serve-path code only building `Postgres`/`Jsonl` configs. Optionally, add a
  `debug_assert!` that `cfg!(test)` is true if `InMemory` is reached.

- **`Jsonl`**: The `accept_single_node_durable: bool` flag already gates this.
  Remove the `profile == Production` check; require
  `accept_single_node_durable == true` unconditionally in serve mode (the
  caller sets it). The flag's name is self-documenting — no profile string
  needed.

- **`Libsql`**: Removed entirely by integrate-postgres.md Phase 6. The
  variant is deleted from `RebornEventStoreConfig` and the match arm is
  removed. The `validate_production_libsql_target()` function is deleted.

- **`Postgres`**: No profile check needed — this is always durable.

**Event store's own `RebornProfile` enum** (line 91: `LocalDev`, `Test`,
`Production`): Remove the `Production` variant. The enum collapses to
`{ LocalDev, Test }` or is removed entirely if no code references it after
the `build_reborn_event_stores()` signature change. The
`to_event_store_profile()` stub in `profile.rs` returns
`brassclaw_reborn_event_store::RebornProfile::LocalDev` (the test/default
variant) until the enum is removed.

**Call site update:** `factory.rs:2536` currently calls:
```rust
.with_reborn_event_store_config(profile.to_event_store_profile(), stores.event_store)
```
After the change, the `profile.to_event_store_profile()` argument is removed
(the parameter is gone from `build_reborn_event_stores()`), and the call
becomes:
```rust
.with_reborn_event_store_config(stores.event_store)
```
or equivalent (depending on how the wiring method signature changes).

> **Sequencing note for this call site:** `factory.rs:2536` is inside
> `build_production_shaped()` (confirmed). Path B Phase 3 step 4/5 removes the
> `Production | MigrationDryRun → build_production_shaped()` branch and deletes
> the function — which eliminates `factory.rs:2536` entirely. The call-site
> update above is therefore only relevant if Path B lands **before**
> integrate-postgres.md Phase 4. If it lands after Phase 4 (the preferred
> sequencing — after Phases 1–6 have landed), the call site is already gone:
> integrate-postgres.md Phase 4 rewrites event store wiring so that
> `RebornEventStoreConfig::Postgres { url }` is retired and stores are wired
> with `Arc<PgPool>` directly (rev 19 C2 / rev 20 C2 of integrate-postgres.md).
> In that case the only work left in this section is removing the `profile:
> RebornProfile` parameter from `build_reborn_event_stores()` in `lib.rs` and
> cleaning up the `Production` guard branches — the `factory.rs` call site no
> longer exists to update.

---

## Phased Execution

> **Sequencing note:** This plan is designed to be merged into
> `integrate-postgres.md`'s phase sequence. The phases below assume
> `integrate-postgres.md` Phases 1-6 (schema, embedded PG, config table,
> migration, libSQL removal) have landed. Path B's phases would slot in as
> a sub-sequence of integrate-postgres.md Phase 8 (file-based config removal,
> which also touches the CLI boot path) and Phase 9 (systemd unit and
> documentation), or as a follow-up release after the Postgres migration is
> complete. Note: integrate-postgres.md Phase 7 is "libSQL → Postgres data
> migration at boot" (data migration, not CLI boot path) — the CLI boot path
> changes belong in Phase 8.

### Phase 1 — Add new knobs, keep old (no breakage)

Objective: New config knobs work; `BRASSCLAW_REBORN_PROFILE` still accepted.
Zero test failures.

1. Add `BRASSCLAW_RUNTIME_PROFILE` parsing in
   `crates/brassclaw_reborn_cli/src/runtime/mod.rs`. When set, call
   `brassclaw_runtime_policy::resolve()` directly; pass result to
   `services_input.with_runtime_policy()`. When absent, derive policy from
   existing `effective_profile()` shim (unchanged behavior).

2. Add `BRASSCLAW_PG_URL` parsing (if not already present from
   integrate-postgres.md Phase 2). When set, use external PG; when absent,
   use embedded PG. This is already specified in integrate-postgres.md §1c
   and §2 — this step only ensures the CLI boot path reads it.

3. Add the fail-closed guard: if `BRASSCLAW_RUNTIME_PROFILE` resolves to a
   non-local profile AND `BRASSCLAW_PG_URL` is unset → fail with the message
   in the fail-closed section above. This guard only fires when
   `BRASSCLAW_RUNTIME_PROFILE` is explicitly set to a non-local value; it
   does not fire when the var is absent (default `local_dev` is local).

4. Add the ceremony-consistency check at boot: read
   `brassclaw_secrets_master.algorithm`, compare to
   `BRASSCLAW_SECRETS_PASSPHRASE_FILE` presence, fail/warn per the ceremony
   derivation section above. This replaces the profile-branched ceremony
   check in integrate-postgres.md §4.4 boot path.
   **Hard prerequisite:** This step requires integrate-postgres.md Phase 3
   (secrets migration — creates `brassclaw_secrets_master` table) to have
   landed. If Phase 3 has not landed, skip this step and defer it to when
   Phase 3 lands. The check also respects the ordering invariant in the
   ceremony derivation section (skip on fresh install before wizard runs).

5. Add `brassclaw migrate --dry-run` CLI flag. Calls
   `build_migration_input(dry_run: bool)`. No changes to main serve/run paths.

**Tests to add:**
- `BRASSCLAW_RUNTIME_PROFILE=local_yolo` without `--confirm-host-access` →
  clear error (resolver already enforces this — test confirms the shim doesn't
  bypass it)
- `BRASSCLAW_RUNTIME_PROFILE=local_yolo` with `--confirm-host-access` →
  resolves `LocalYolo` policy
- `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` + no `BRASSCLAW_PG_URL` →
  fail-closed error
- `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` + `BRASSCLAW_PG_URL=postgres://...`
  → boots with external PG and HostedSafe policy
- `BRASSCLAW_SECRETS_PASSPHRASE_FILE` set + `algorithm='raw-key-on-disk'` →
  warning, proceeds with raw-key
- `BRASSCLAW_SECRETS_PASSPHRASE_FILE` unset + `algorithm='aes256gcm-argon2id'`
  → fail-closed error
- `brassclaw migrate --dry-run` → outputs what would be migrated, no DB writes

### Phase 2 — Deprecate old env var

Objective: `BRASSCLAW_REBORN_PROFILE` still accepted but emits a deprecation
warning and translates to the new knobs per the deprecation shim table above.

1. In `effective_profile()`: when `BRASSCLAW_REBORN_PROFILE` is present, print
   `eprintln!("WARNING: BRASSCLAW_REBORN_PROFILE is deprecated. Use BRASSCLAW_RUNTIME_PROFILE, BRASSCLAW_PG_URL, and BRASSCLAW_SECRETS_PASSPHRASE_FILE.")`.
2. Translate old profile values per the deprecation shim table. The
   `migration-dry-run` translation is an error. When `production` is detected
   AND `BRASSCLAW_RUNTIME_PROFILE` is not set, print the runtime-profile
   warning (see deprecation shim section above) urging the operator to set it
   explicitly. The `production` translation does NOT require `BRASSCLAW_PG_URL`
   — embedded PG is durable in the integrate-postgres.md world. The fail-closed
   for non-local profiles is handled by the new guard, not the shim.
3. When both `BRASSCLAW_REBORN_PROFILE` and `BRASSCLAW_RUNTIME_PROFILE` are set,
   `BRASSCLAW_RUNTIME_PROFILE` wins (new knob takes precedence). Print a warning
   that the old var is being ignored.
4. Repurpose `profile list` command to show available `RuntimeProfile` values
   and the new env vars.
5. Update `AGENTS.md` Key Environment Variables table.

**Tests to add:**
- `BRASSCLAW_REBORN_PROFILE=local-dev` → deprecation warning + `local_dev` policy
- `BRASSCLAW_REBORN_PROFILE=local-dev-yolo` → deprecation warning + `local_yolo` policy
- `BRASSCLAW_REBORN_PROFILE=production` + no `BRASSCLAW_PG_URL` → deprecation
  warning + runtime-profile warning + boots with embedded PG (NOT fail-closed —
  embedded PG is durable)
- `BRASSCLAW_REBORN_PROFILE=production` + `BRASSCLAW_PG_URL` set → deprecation
  warning + runtime-profile warning + boots with external PG
- `BRASSCLAW_REBORN_PROFILE=production` + `BRASSCLAW_RUNTIME_PROFILE=hosted_safe`
  + no `BRASSCLAW_PG_URL` → fail-closed (from the new guard, not the shim:
  non-local profile requires PG_URL)
- `BRASSCLAW_REBORN_PROFILE=production` + `BRASSCLAW_RUNTIME_PROFILE=hosted_safe`
  + `BRASSCLAW_PG_URL` set → deprecation warning + `hosted_safe` policy +
  external PG (new knob wins)
- `BRASSCLAW_REBORN_PROFILE=migration-dry-run` → error

### Phase 3 — Remove old boot profile code (the actual removal)

Objective: `BRASSCLAW_REBORN_PROFILE` fully gone. Codebase is at target state.

Ordered steps (compiler guides you at each step):

1. **Delete** `crates/brassclaw_reborn_config/src/profile.rs`
2. **Remove** `pub mod profile;` from `crates/brassclaw_reborn_config/src/lib.rs`
3. **Remove** `RebornCompositionProfile::LocalDevYolo`, `Production`,
   `MigrationDryRun` from `crates/brassclaw_reborn_composition/src/profile.rs`
   — compiler will enumerate every match site
3a. **Remove `to_event_store_profile()`** from `profile.rs` and inline the
    constant `brassclaw_reborn_event_store::RebornProfile::LocalDev` directly at
    the call site in `factory.rs:2536` (the stub is no longer needed now that
    the event store guard is replaced per the "Event store guard replacement"
    section). After this, the event store's own `RebornProfile` enum (`LocalDev`,
    `Test`) can be collapsed or removed if no other code references it — the
    compiler confirms. Do this in the same commit as step 3 to avoid a
    transient compile error from a function that returns a type whose variant is
    about to disappear.
4. **Remove** the `Production | MigrationDryRun → build_production_shaped()`
   branch from `build_reborn_services()` in `factory.rs`
5. **Delete** `build_production_shaped()` function
6. **Remove** `profile: RebornCompositionProfile` parameters from
   `RebornBuildInput` constructors (or collapse to `Active`/`Disabled`)
7. **Remove** `composition_profile()` and `effective_profile()` from `mod.rs`;
   remove `print_runtime_banner()`'s `profile:` line (line 155); remove
   `use brassclaw_reborn_config::{REBORN_PROFILE_ENV, RebornProfile}` import
7a. **Remove** `profile: RebornProfile` field from `RebornBootConfig` in
    `crates/brassclaw_reborn_config/src/boot.rs` — remove `profile()` accessor,
    `from_env()` reading of `REBORN_PROFILE_ENV`, `resolve_from_env_parts()`
    `profile` parameter, and `into_parts()` profile return. The compiler will
    enumerate all `config.profile()` call sites across the CLI crate.
7b. **Remove** `boot.profile` field from `RebornConfigFile` boot struct in
    `crates/brassclaw_reborn_config/src/config_file.rs` (line ~671 validation +
    line ~1102 read). The `deny_unknown_fields` attribute will reject operator
    TOML that still has `profile = "..."` — this is intentional (forces
    operators to migrate to `BRASSCLAW_RUNTIME_PROFILE`).
7c. **Remove** `profile: RebornProfile` field from `RebornDoctorReport` in
    `crates/brassclaw_reborn_config/src/doctor.rs` — remove `profile()` accessor,
    update `from_config()` to not extract profile. Update CLI consumers:
    `commands/doctor.rs:16` and `commands/config/mod.rs:53` — remove the
    `println!("profile: {}", report.profile())` lines or replace with
    `println!("runtime_profile: {}", ...)` using the resolved `RuntimeProfile`.
7d. **Remove** `println!("profile: {}", config.profile())` from
    `crates/brassclaw_reborn_cli/src/commands/run.rs:54` — replace with
    `println!("runtime_profile: {}", resolved_profile.as_str())` or remove.
7e. **Update** `crates/brassclaw_reborn_cli/src/commands/skills.rs` — remove
    `build_skill_list_config()`'s call to `effective_profile()` (line 94) and
    the `match profile { ... Production | MigrationDryRun => bail!() }`
    rejection (lines 95-102); remove `profile: RebornProfile` field from
    `SkillListConfig` (line 89); remove `"profile"` JSON field in skill output
    (line 53); remove `use brassclaw_reborn_config::RebornProfile` import.
7f. **Update** `crates/brassclaw_reborn_cli/src/commands/config/init.rs` —
    remove `profile = "local-dev"` (line 149) and the comment (line 146) from
    the `config init` template; add migration comment:
    `# [boot].profile removed — use BRASSCLAW_RUNTIME_PROFILE env var instead.`
7g. **Remove or rewrite** `crates/brassclaw_reborn_config/tests/profile_contract.rs`
    — the entire file tests `RebornProfile` parsing and is obsolete. Update
    `crates/brassclaw_reborn_config/tests/doctor_contract.rs:16` to not assert
    on `report.profile()`. Update all `mod.rs` test callers of
    `resolve_from_env_parts()` (lines 802, 835, 857, 881, 930, 966, 1005, 1129,
    1158) to not pass the `profile` parameter.
8. **Remove** `requires_production_shape()` from `RebornCompositionProfile`
9. **Update** `crates/brassclaw_reborn_composition/src/local_runtime_profile.rs`
   — all four public functions + the private helper + the error type currently
   take/hold a `RebornCompositionProfile` and must be migrated:
   - `local_runtime_build_input(profile: RebornCompositionProfile, …)` (line 24)
     → change parameter to `runtime_profile: RuntimeProfile`
   - `local_runtime_build_input_with_options(profile: RebornCompositionProfile, …)`
     (line 39) → change parameter to `runtime_profile: RuntimeProfile`
   - `local_dev_runtime_policy()` (line 53) — currently hardcodes
     `RebornCompositionProfile::LocalDev`; **delete the helper and inline its
     call sites** with direct
     `brassclaw_runtime_policy::resolve(ResolveRequest::new(DeploymentMode::LocalSingleUser,
     RuntimeProfile::LocalDev))` calls. The helper is a trivial one-liner wrapper
     once the `RebornCompositionProfile` mapping is gone; keeping a named function
     adds an indirection layer that serves no purpose when the caller already
     holds a `RuntimeProfile`. **Test call sites:** `local_dev_host_tests.rs:222`
     calls `crate::local_dev_runtime_policy()` — update it to call
     `brassclaw_runtime_policy::resolve()` directly. The test-local
     `fn local_dev_runtime_policy()` at `runtime.rs:3120` constructs an
     `EffectiveRuntimePolicy` directly (does NOT call the crate-level function),
     so it is unaffected — but consider renaming it to avoid confusion with the
     deleted crate-level function.
   - `local_dev_yolo_runtime_policy(confirm_host_access: bool)` (line 68) —
     currently hardcodes `RebornCompositionProfile::LocalDevYolo`; **delete and
     inline at call sites** for the same reason — replace with direct struct
     construction (there is no builder method on `ResolveRequest`):
     ```rust
     brassclaw_runtime_policy::resolve(brassclaw_runtime_policy::ResolveRequest {
         yolo_disclosure_acknowledged: confirm_host_access,
         ..brassclaw_runtime_policy::ResolveRequest::new(
             DeploymentMode::LocalSingleUser,
             RuntimeProfile::LocalYolo,
         )
     })?
     ```
     This matches the existing pattern in `local_runtime_policy()` (line 98-104).
     **Test call sites:** `local_dev_host_tests.rs:218` calls
     `crate::local_dev_yolo_runtime_policy(true)` — update it to call
     `brassclaw_runtime_policy::resolve()` directly. The compiler will find
     every call site.
   - `local_runtime_policy(profile: RebornCompositionProfile, …)` (line 85,
     private) → change parameter to `runtime_profile: RuntimeProfile`; the
     `match profile { LocalDev → LocalDev, LocalDevYolo → LocalYolo, … }`
     mapping is removed (the caller passes the `RuntimeProfile` directly)
   - `RebornLocalRuntimeProfileError::UnsupportedProfile { profile:
     RebornCompositionProfile }` (line 12) → **remove the variant**. After the
     migration, `local_runtime_policy()` accepts `RuntimeProfile` directly from
     the resolver, and the resolver already rejects non-local profiles on
     `LocalSingleUser` deployment before this function is ever called — no code
     path can reach `UnsupportedProfile`. Changing the field type to
     `RuntimeProfile` would keep dead code alive. Delete the variant; the
     compiler will confirm no match arm references it.
10. **Rename** `crates/brassclaw_reborn_cli/src/commands/profile.rs` →
    `runtime_profile.rs`; rename `ProfileCommand` → `RuntimeProfileCommand`,
    `ProfileSubcommand` → `RuntimeProfileSubcommand`,
    `ProfileListCommand` → `RuntimeProfileListCommand`; update the CLI
    subcommand registration from `profile` to `runtime-profile`; the command
    lists the 12 `RuntimeProfile` variants (via `RuntimeProfile::all()` if it
    exists, or a compiled-in array) + the three new env vars
11. **Update smoke tests** — remove/replace assertions about
    `BRASSCLAW_REBORN_PROFILE` and `migration-dry-run`; update
    `profile_list_shows_supported_profiles_without_reborn_home` and
    `profile_list_json_is_stable_and_does_not_resolve_reborn_home` to use
    `runtime-profile list` and assert the 12 `RuntimeProfile` variants
12. **Run** `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`
    — must be zero warnings
13. **Run** `cargo test` — all tests must pass

### Phase 4 — Update integrate-postgres.md sections (decision gate)

> **Decision gate:** Phase 4 is mandatory or no-op depending on execution path:
> - **If this plan is executed BEFORE integrate-postgres.md lands:** Phase 4 is
>   **mandatory** and MUST be in the same PR as Phase 3. Without it, the two
>   docs drift — integrate-postgres.md still references
>   `BRASSCLAW_REBORN_PROFILE` in §1c/§4.4/§7.3/§8.1 while the code no longer
>   reads it. The PR is not mergeable until both are consistent.
> - **If this plan is merged INTO integrate-postgres.md:** Phase 4 is a **no-op**
>   — the merge edit updates the integrate-postgres.md sections as part of the
>   merge itself. There is no separate Phase 4 step.

If this plan is being implemented as a follow-up to integrate-postgres.md
(rather than merged into it), update the integrate-postgres.md sections listed
in the "Files changed by integrate-postgres.md" table above. All **fourteen**
section updates must be in the same PR as the Phase 3 code changes.

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| **Fail-closed guard bypassed** — operator forgets `BRASSCLAW_PG_URL` for a hosted deployment | High | Path B guard checks `!RuntimeProfile.is_local() && pg_url.is_none()` → fail. This is stricter than the old `profile == Production` check (which could be forgotten). |
| **Ceremony mismatch at boot** — operator flips passphrase-file presence without running rewrap | High | Boot path checks `algorithm` vs passphrase-file presence: `aes256gcm-argon2id` + no file → fail; `raw-key-on-disk` + file → warn. The DB row (changed only by rewrap) is the source of truth. |
| **Ceremony check fires before `brassclaw_secrets_master` table exists** | High | Ordering invariant: ceremony check runs after schema runner (Phase 1) + migration/wizard. Skipped on fresh install before wizard runs. Phase 1 step 4 has a hard prerequisite on integrate-postgres.md Phase 3. |
| **Yolo disclosure gate silently lost** | High | `resolver::resolve()` already enforces it; the `--confirm-host-access` → `yolo_disclosure_acknowledged` mapping is unchanged. Test confirms the shim doesn't bypass it. |
| **`production` deprecation translation drops the fail-closed** | High | The shim does NOT require `BRASSCLAW_PG_URL` for `production` — embedded PG is durable in the integrate-postgres.md world, and breaking embedded-PG production users would be a regression. The fail-closed is preserved by the **new guard** (`!is_local() && pg_url.is_none()`): if the operator sets `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` (or any non-local variant), the guard fires regardless of whether `BRASSCLAW_REBORN_PROFILE` is set. If the operator leaves the runtime profile at default (`local_dev`), embedded PG is the correct durable storage for a local profile. The shim's only hard requirements are the deprecation warning + the runtime-profile warning (urging the operator to set `BRASSCLAW_RUNTIME_PROFILE` explicitly). |
| **`production` shim silently drops RuntimeProfile** | Medium | The shim emits a warning when `production` is detected without `BRASSCLAW_RUNTIME_PROFILE`, urging the operator to set it explicitly. Not a fail (preserves backward compat) but avoids silent regression. |
| **`SecureDefault` is non-local — operator surprise** | Medium | `is_local()` returns `false` for `SecureDefault`. An operator who sets `BRASSCLAW_RUNTIME_PROFILE=secure_default` without `BRASSCLAW_PG_URL` gets the fail-closed error. This is correct (SecureDefault is a hosted/enterprise helper shape), but may surprise operators who assume "secure default" means "local safe default." The fail-closed error message lists the local variants explicitly. |
| **`RebornCompositionProfile` persisted in stored state** | Medium | Profile is never serialized to DB; only parsed from startup string. Verify with `grep -r "composition_profile"` on data dirs before Phase 3 cut. |
| **24+ call sites of `RebornBuildInput` break at once** | Medium | Monorepo: compiler enumerates all sites in Phase 3 step 6; fix atomically. |
| **`build_production_shaped()` removal loses production wiring** | Low | `integrate-postgres.md` already moves all production wiring into `build_local_dev()` (single code path with URL-derived storage). `build_production_shaped()` is already vestigial after integrate-postgres.md Phase 5 (hooks and auth — wires factory to single Postgres path). Additionally, `build_production_shaped()` is **already unreachable from the CLI** — both `run` and `serve` go through `local_runtime_build_input_with_options()` (mod.rs:413) which rejects `Production`/`MigrationDryRun` before the `RebornBuildInput` reaches the factory. The function is only called from `build_reborn_services()` when a caller constructs a `RebornBuildInput` with `profile: Production` directly (tests only). Verify no production-only wiring remains in it before deletion, but the risk is lower than it appears. |
| **Smoke test `skills_list_rejects_unsupported_profiles` inverts** | Low | Replace with `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` + no `BRASSCLAW_PG_URL` → fail-closed error. |
| **`EnterpriseYoloDedicated` + `admin_approves_dedicated_yolo` path never tested after change** | Low | Write a unit test for `ResolveRequest` with `EnterpriseYoloDedicated + admin_approves=false → error`. |
| **Non-local + raw-key-on-disk silently accepted without warning** | Low | Boot path emits a warning recommending rewrap. Not a fail (raw-key on app host is not a security hole when PG is external), but the operator is informed. |
| **`[boot].profile` config file field removed — operator TOML rejected** | Medium | `deny_unknown_fields` on the boot struct will reject any operator TOML that still has `profile = "..."` after the field is removed. The `config init` template is updated to not write it, but operators with existing config files will get a parse error on upgrade. Mitigation: the error message from `deny_unknown_fields` includes the field name; add a migration note in the release notes and in the `config init` template comment: `# [boot].profile removed — use BRASSCLAW_RUNTIME_PROFILE env var instead.` |
| **`RebornBootConfig` struct change breaks `resolve_from_env_parts()` test callers** | Low | `boot.rs:resolve_from_env_parts()` takes a `profile: Option<OsString>` parameter that is removed. The compiler will enumerate all call sites (tests in `profile_contract.rs`, `mod.rs` tests at lines 802, 835, 857, 881, 930, 966, 1005, 1129, 1158, etc.). All test callers pass `Some("local-dev")` or similar — update them to not pass the profile parameter. |

---

## Verification / Testing Plan

> **`--test-exit` flag prerequisite:** The smoke tests below use
> `brassclaw serve --test-exit`, which does NOT exist in the current CLI
> (`crates/brassclaw_reborn_cli/src/commands/`). It must be added as part of
> Phase 1 — a test-only flag that starts the service, verifies boot
> (config load, DB connect, ceremony check, policy resolution), and exits
> non-zero with a diagnostic on any failure, without accepting requests.
>
> **Feature-gate:** Use `#[cfg(feature = "smoke")]` alone — **not**
> `#[cfg(any(test, feature = "smoke"))]`. The project runs
> `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`
> which activates every feature including a hypothetical `smoke` feature; using
> `any(test, feature = "smoke")` would therefore expose `--test-exit` in
> `--all-features` release builds, defeating the gate. The `smoke` feature must
> NOT be in the crate's `default` feature set. Alternatively, skip the
> `--test-exit` flag entirely and replace the smoke tests with
> `cargo test --features integration` boot-assertion integration tests (preferred
> — no new feature flag required, integrates with the existing CI gate).

After Phase 3 is complete, run these in order:

```bash
# 1. Lint — zero warnings required
cargo clippy --all --benches --tests --examples --all-features -- -D warnings

# 2. Unit tests for the composition crate
cargo test -p brassclaw_reborn_composition

# 3. Unit tests for the CLI / runtime module
cargo test -p brassclaw_reborn_cli

# 4. Runtime policy tests (all security gates)
cargo test -p brassclaw_runtime_policy

# 5. Config crate tests
cargo test -p brassclaw_reborn_config

# 6. Integration tests (requires PostgreSQL)
cargo test --features integration

# 7. Smoke test: default start uses LocalDev policy + embedded PG
BRASSCLAW_REBORN_HOME=/tmp/bc-test brassclaw serve --test-exit
# Expect: starts with AskDestructive policy, embedded PG, raw-key-on-disk, no profile env var needed

# 8. Smoke test: explicit external Postgres URL works
BRASSCLAW_PG_URL=postgres://... brassclaw serve --test-exit
# Expect: starts with external PG backend, LocalDev policy (default)

# 9. Smoke test: yolo without disclosure = clear error
BRASSCLAW_RUNTIME_PROFILE=local_yolo brassclaw serve --test-exit
# Expect: error "requires explicit disclosure acknowledgement"

# 10. Smoke test: yolo with disclosure works
BRASSCLAW_RUNTIME_PROFILE=local_yolo brassclaw serve --confirm-host-access --test-exit
# Expect: starts with Minimal approval policy

# 11. Smoke test: non-local + no PG_URL = fail-closed
BRASSCLAW_RUNTIME_PROFILE=hosted_safe brassclaw serve --test-exit
# Expect: error "Non-local runtime profile 'hosted_safe' requires BRASSCLAW_PG_URL"

# 12. Smoke test: non-local + PG_URL = boots
BRASSCLAW_RUNTIME_PROFILE=hosted_safe BRASSCLAW_PG_URL=postgres://... brassclaw serve --test-exit
# Expect: starts with external PG and HostedSafe policy

# 13. Smoke test: ceremony mismatch = fail-closed
# (requires a DB row with algorithm='aes256gcm-argon2id')
# NOTE: env -u UNSETS the var; "BRASSCLAW_SECRETS_PASSPHRASE_FILE=" (empty string)
# is "set" in bash and would NOT trigger the fail-closed. The runtime treats
# empty string as absent (see ceremony derivation section), but the test should
# use env -u to verify the truly-unset case.
env -u BRASSCLAW_SECRETS_PASSPHRASE_FILE brassclaw serve --test-exit
# Expect: error "Master key is passphrase-wrapped but BRASSCLAW_SECRETS_PASSPHRASE_FILE is not set"

# 14. Smoke test: migration dry-run flag
brassclaw migrate --dry-run
# Expect: outputs what would be migrated, no DB writes

# 15. Smoke test: deprecation warning
BRASSCLAW_REBORN_PROFILE=local-dev brassclaw serve --test-exit
# Expect: deprecation warning + starts with local_dev policy (Phase 2 only; Phase 3 this is an error)

# 16. No mentions of BRASSCLAW_REBORN_PROFILE in compiled binary help output
brassclaw --help | grep -v "BRASSCLAW_REBORN_PROFILE"
# Expect: grep returns empty (old env var is gone from help text)

# 17. Smoke test: SecureDefault is non-local (surprise check)
BRASSCLAW_RUNTIME_PROFILE=secure_default brassclaw serve --test-exit
# Expect: fail-closed error "Non-local runtime profile 'secure_default' requires
# BRASSCLAW_PG_URL" — SecureDefault is NOT local despite its name

# 18. Smoke test: empty-string passphrase file = absent
# (requires a DB row with algorithm='aes256gcm-argon2id')
# Empty string is "set" in bash but the runtime treats it as absent.
BRASSCLAW_SECRETS_PASSPHRASE_FILE= brassclaw serve --test-exit
# Expect: same fail-closed as test #13 (empty string = absent = no passphrase)
# This test verifies the empty-string-as-absent semantics documented in the
# ceremony derivation section. Contrast with test #13 which uses env -u (truly unset).

# 19. Smoke test: [boot].profile in config.toml is rejected
# Write a config.toml with [boot] profile = "local-dev" and verify it's rejected
# by deny_unknown_fields (the field is removed from the struct).
echo '[boot]\nprofile = "local-dev"' > /tmp/bc-test/config.toml
BRASSCLAW_REBORN_HOME=/tmp/bc-test brassclaw config path
# Expect: error mentioning unknown field `profile` in section [boot]
# (deny_unknown_fields rejects it — forces operator to migrate to env var)

# 20. Smoke test: config init template does not write profile
brassclaw config init --force
# Expect: generated config.toml has NO `profile = "..."` line in [boot];
# instead has comment: "# [boot].profile removed — use BRASSCLAW_RUNTIME_PROFILE"

# 21. Smoke test: doctor command does not print profile line
brassclaw doctor
# Expect: output has NO "profile:" line (or has "runtime_profile:" instead)

# 22. Smoke test: run --readiness does not print profile line
brassclaw run --readiness
# Expect: output has NO "profile:" line (or has "runtime_profile:" instead)

# 23. Smoke test: skills list works without profile env var
brassclaw skills list
# Expect: succeeds (no longer gates on profile=local-dev); the skills command
# works with any RuntimeProfile (policy resolved upstream)
```

---

## Should This Be Integrated into integrate-postgres.md?

**Yes — Path B is the cleaner long-term design, and it is designed for
merging.** The three-knob model eliminates the last composite-profile
coupling in the Postgres migration plan. Specifically:

1. **`BRASSCLAW_PG_URL` is already in integrate-postgres.md §1c** — no new var
   needed, just remove the `BRASSCLAW_REBORN_PROFILE` line from the same table.
2. **`BRASSCLAW_SECRETS_PASSPHRASE_FILE` is already in integrate-postgres.md
   §1c** — no new var needed, just change its description from "production
   profile only" to "ceremony selector."
3. **`BRASSCLAW_RUNTIME_PROFILE` is the only new var** — it replaces the
   security-policy arm of `BRASSCLAW_REBORN_PROFILE` and elevates the existing
   `RuntimeProfile` enum to the primary security knob.
4. **The ceremony derivation, fail-closed guard, and deprecation shim are all
   drop-in replacements** for integrate-postgres.md §4.4, §4.4 boot path,
   §8.1 step 6, and the `migration-dry-run` profile — see the "Files changed
   by integrate-postgres.md" table above for the exact section-by-section
   mapping.

**Recommended merge approach:** After this draft is reviewed and approved,
fold the three-knob model into integrate-postgres.md by:
- Editing §0 guiding principles to remove `BRASSCLAW_REBORN_PROFILE` from the
  bootstrap tier list and add `BRASSCLAW_RUNTIME_PROFILE`
- Editing §1c to replace `BRASSCLAW_REBORN_PROFILE` with
  `BRASSCLAW_RUNTIME_PROFILE` and update the
  `BRASSCLAW_SECRETS_PASSPHRASE_FILE` description
- Editing §4.4 to replace profile-branched ceremony with passphrase-file-presence
  ceremony + algorithm-consistency boot check
- Editing §7.1 step 3 to make the `BRASSCLAW_SECRETS_PASSPHRASE_FILE` line in
  `secrets.env` conditional — only write it if the operator ran
  `rewrap --strategy passphrase-file=...` (step 2); operators using raw-key-on-disk
  ceremony must omit the line to avoid spurious boot warnings
- Editing §7.3 to replace the single `BRASSCLAW_REBORN_PROFILE=production` unit
  with the **two-variant unit replacement** (single-host with `local_safe` +
  embedded PG, multi-tenant with `hosted_safe` + required `BRASSCLAW_PG_URL`).
  Do NOT do a one-line `production` → `hosted_safe` replacement — `hosted_safe`
  is non-local and the fail-closed guard requires `BRASSCLAW_PG_URL`, which is
  commented out in the current unit. See "§7.3 systemd unit replacement" above.
- Editing §8.1 step 6 to replace profile-branched migration with
  passphrase-file-presence migration
- Editing §8.1 to replace `migration-dry-run` profile with `--dry-run` CLI flag
- Editing integrate-postgres.md Phase 3 checklist to update the per-boot-unwrap
  and fail-closed items (drop "in production profile" / update ceremony-selector
  wording) per the "Files changed by integrate-postgres.md" table above
- Adding the fail-closed guard (`!is_local() && pg_url.is_none()`) to the boot
  path
- Adding the deprecation shim as a transitional section (to be removed in the
  release after the migration)

The phased execution (Phase 1-4 above) slots into integrate-postgres.md's
Phase 8 (file-based config removal) and Phase 9 (systemd unit and
documentation), or as a follow-up release.

---

## Summary

The plan does not remove any security properties — all approval gates,
isolation boundaries, yolo disclosure requirements, enterprise admin gates,
secret ceremony fail-closed, and rewrap key-source invariants remain intact.
It removes the `RebornProfile` boot-level concept that was acting as a
composite shorthand for four unrelated decisions (storage selection +
security policy + secret ceremony + migration mode) and replaces it with
three focused, independent mechanisms plus a CLI flag:

1. **`BRASSCLAW_PG_URL`** — determines storage backend (absent = embedded PG,
   set = external PG). Already in integrate-postgres.md.
2. **`BRASSCLAW_RUNTIME_PROFILE`** — determines security policy (default =
   `local_dev` = AskDestructive). Elevates the existing `RuntimeProfile` enum
   to the primary security knob.
3. **`BRASSCLAW_SECRETS_PASSPHRASE_FILE`** — determines secret ceremony (absent
   = raw-key-on-disk, present = passphrase-wrapped). Already in
   integrate-postgres.md; repurposed from "production only" to "ceremony
   selector."
4. **`brassclaw migrate --dry-run`** — replaces the `migration-dry-run` profile
   value with a CLI flag.

The result is one binary, one code path, no profile switching, and a
strictly tighter fail-closed guard (non-local RuntimeProfile + no PG_URL →
fail) than the profile-based guard it replaces.
