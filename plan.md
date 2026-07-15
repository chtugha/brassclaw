# Tomedo Integration — Reborn v2 Architecture Plan

## Problem Statement

The previous approach (WASM tools, `tools-src/tomedo/`, and `tools-src/tomedo/src/types.rs`)
used the **old WASM extension model**, which is **not the Reborn v2 architecture**.

The Reborn v2 architecture is a **Rust-native port/adapter stack** that crosses these layers:

```
browser (webui_v2_static JS)
  └ apiFetch → brassclaw_webui_v2 handler (descriptor + route)
      └ Arc<dyn RebornServicesApi>  (brassclaw_product_workflow facade)
          └ port trait → composition impl (brassclaw_reborn_composition)
              └ substrate handles (secret store, config files, HTTP client)
```

No WASM, no `tools-src/`, no WIT bindings. Everything is Rust async across `crates/`.

---

## Files to DELETE / Abandon (Wrong Architecture)

These files were written for the WASM approach and must NOT be continued:

- `tools-src/tomedo/` — entire directory (WASM crate, wrong model)
  - `tools-src/tomedo/Cargo.toml`
  - `tools-src/tomedo/tomedo-tool.capabilities.json`
  - `tools-src/tomedo/src/types.rs` ← just created, abandon it

The native monitor daemon (`native-plugins/tomedo-monitor/`) is architecture-agnostic
(it's a system daemon serving a local HTTP endpoint) and **stays**.

---

## Target Architecture: What to Build

### Layer 0 — macOS Native Daemon (keep as-is)
**Location:** `native-plugins/tomedo-monitor/`  
**Status:** ✅ Already complete  
**Purpose:** Polls Tomedo window via Accessibility API, exposes patient ID at `http://127.0.0.1:49152/tomedo`

---

### Layer 1 — Port Traits + DTOs
**Location:** `crates/brassclaw_product_workflow/src/reborn_services/tomedo.rs`  
**Pattern:** Follow `crates/brassclaw_product_workflow/src/reborn_services/llm_config.rs`

Define:
- `TomedoService` trait (async, `Send + Sync`)
- All request/response DTOs
- `TomedoServiceError` + `TomedoServiceErrorCode`

Methods needed on the trait:
```
get_active_patient_id()  → ActivePatientResult
get_patient_info(patient_id, fields?)  → PatientInfo
get_icd_codes(patient_id, quarter, year)  → IcdCodesResult
add_icd_code(patient_id, icd_code, certainty, note?)  → Diagnosis
delete_icd_code(patient_id, diagnosis_id)  → ()
remove_duplicate_icd_codes(patient_id, quarter, year)  → DuplicateRemovalResult
get_invoices(patient_id, quarter, year)  → InvoiceResult
add_invoice_item(patient_id, billing_number, description, quarter?, year?)  → InvoiceItemAdded
check_invoice_completeness(patient_id, quarter, year)  → InvoiceCompletenessResult
check_p4_qualification(patient_id, quarter?, year?, auto_apply)  → P4QualificationResult
scan_patient_records(patient_id, quarter?, year?)  → RecordScanResult
get_tomedo_config()  → TomedoConfig
save_tomedo_config(config)  → ()
```

Also expose in `brassclaw_product_workflow/src/reborn_services.rs`:
- `tomedo_service()` method with default "unavailable" body on `RebornServicesApi`

---

### Layer 2 — Facade Integration
**Location:** `crates/brassclaw_product_workflow/src/reborn_services.rs`

Add to the `RebornServicesApi` struct/trait:
- `Option<Arc<dyn TomedoService>>` field
- `with_tomedo_service(impl TomedoService)` builder
- Default "unavailable" implementations for all N methods

---

### Layer 3 — Composition Adapter
**Location:** `crates/brassclaw_reborn_composition/src/tomedo.rs`

Implement `TomedoService` against:
- A `reqwest::Client` (or the host HTTP client) for Tomedo REST API calls
- A local HTTP client for `http://127.0.0.1:49152/tomedo` (the monitor)
- The host `SecretStore` for `tomedo_api_url`, `tomedo_username`, `tomedo_api_password`
- A config store / file for `tomedo_patient_fields`, `tomedo_invoice_rules`, `tomedo_p4_icd_codes`

Wire through `factory.rs` and attach in `webui.rs` → `build_webui_services`.

Gate behind a cargo feature: `tomedo-integration` (so it can be opt-in).

---

### Layer 4 — HTTP Route Descriptors + Handlers
**Location:** `crates/brassclaw_webui_v2/src/`

Add route constants and patterns to `descriptors.rs`:
```
WEBUI_V2_ROUTE_TOMEDO_GET_ACTIVE_PATIENT   GET  /api/webchat/v2/tomedo/active-patient
WEBUI_V2_ROUTE_TOMEDO_GET_PATIENT_INFO     GET  /api/webchat/v2/tomedo/patients/{patient_id}
WEBUI_V2_ROUTE_TOMEDO_GET_ICD_CODES        GET  /api/webchat/v2/tomedo/patients/{patient_id}/diagnoses
WEBUI_V2_ROUTE_TOMEDO_ADD_ICD_CODE         POST /api/webchat/v2/tomedo/patients/{patient_id}/diagnoses
WEBUI_V2_ROUTE_TOMEDO_DELETE_ICD_CODE      POST /api/webchat/v2/tomedo/patients/{patient_id}/diagnoses/{diagnosis_id}/delete
WEBUI_V2_ROUTE_TOMEDO_DEDUP_ICD_CODES      POST /api/webchat/v2/tomedo/patients/{patient_id}/diagnoses/deduplicate
WEBUI_V2_ROUTE_TOMEDO_GET_INVOICES         GET  /api/webchat/v2/tomedo/patients/{patient_id}/invoices
WEBUI_V2_ROUTE_TOMEDO_ADD_INVOICE_ITEM     POST /api/webchat/v2/tomedo/patients/{patient_id}/invoices
WEBUI_V2_ROUTE_TOMEDO_CHECK_COMPLETENESS   POST /api/webchat/v2/tomedo/patients/{patient_id}/invoices/check
WEBUI_V2_ROUTE_TOMEDO_CHECK_P4             POST /api/webchat/v2/tomedo/patients/{patient_id}/p4-check
WEBUI_V2_ROUTE_TOMEDO_SCAN_RECORDS         POST /api/webchat/v2/tomedo/patients/{patient_id}/scan
WEBUI_V2_ROUTE_TOMEDO_GET_CONFIG           GET  /api/webchat/v2/tomedo/config
WEBUI_V2_ROUTE_TOMEDO_SAVE_CONFIG          POST /api/webchat/v2/tomedo/config
```

Add handler file: `crates/brassclaw_webui_v2/src/handlers/tomedo.rs`
- Thin handlers: parse HTTP params → call `state.services().tomedo_*()` → serialize JSON
- Bearer-auth required on all routes (standard `IngressAuthPolicy::BearerRequired`)

Update `crates/brassclaw_webui_v2/tests/webui_v2_descriptors_contract.rs` with new routes.

---

### Layer 5 — Frontend JS
**Location:** `crates/brassclaw_webui_v2_static/static/js/pages/settings/`

Create:
- `lib/tomedo-api.js` — `apiFetch` wrappers for all tomedo endpoints
- `components/TomedoSettingsTab.js` — Settings tab UI:
  - Connection config (URL, username, password — password write-only, never displayed)
  - Patient fields selector (checkboxes for: name, birthdate, insurance, address, phone, email)
  - Invoice rules editor (text area, JSON array)
  - P4 custom ICD overrides field
  - "Test Connection" button → calls `/api/webchat/v2/tomedo/active-patient`
- `hooks/useTomedo.js` — state/fetch hooks for the settings page
- Wire the tab into `settings-page.js`

---

### Layer 6 — Agent Skill (Markdown)
**Location:** `skills/tomedo/skill.md`

Write a comprehensive SKILL.md following the existing pattern in `skills/github/skill.md`:
- YAML frontmatter: activation keywords (tomedo, patient, ICD, HZV, P4, Abrechnung, etc.)
- Instructions for the agent covering all 6 use cases:
  1. Connect to Tomedo API (prompt user for URL/credentials, saved via `/tomedo/config`)
  2. Fetch active patient via monitor, then retrieve patient info
  3. Check invoice completeness against configured rules
  4. Read and deduplicate ICD codes
  5. P4 qualification check and auto-apply logic (56544 code, 2x/quarter)
  6. Scan patient records for HAVG-billable items
- Embedded P4 ICD code reference list (from HAEV/AOK documents)
- Embedded HAVG billing code reference list (from the HAVG Gegenüberstellung PDF)

---

## Implementation Order (dependency-safe)

1. **DELETE / abandon** `tools-src/tomedo/` WASM files
2. **Layer 1**: Write `crates/brassclaw_product_workflow/src/reborn_services/tomedo.rs`
3. **Layer 2**: Add `tomedo_service` to `RebornServicesApi` in `reborn_services.rs`
4. **Layer 3**: Write `crates/brassclaw_reborn_composition/src/tomedo.rs` adapter
5. **Layer 3**: Wire into `factory.rs`, `webui.rs` under `tomedo-integration` feature
6. **Layer 4**: Add descriptors to `descriptors.rs`, handlers to `handlers/tomedo.rs`, mount in `router.rs`
7. **Layer 4**: Update descriptor contract test
8. **Layer 5**: Write JS files (`tomedo-api.js`, `TomedoSettingsTab.js`, `useTomedo.js`)
9. **Layer 5**: Wire tab into `settings-page.js`
10. **Layer 6**: Write `skills/tomedo/skill.md`
11. **Verify**: `cargo build -p brassclaw_product_workflow`, then composition, then webui_v2, then full

---

## Key Data: P4 ICD Codes (AOK BW HZV)

The P4 programme requires ≥3 diagnosed chronic conditions from:

| Group | ICD-10 Codes |
|---|---|
| Hypertension | I10, I11, I12, I13, I15 |
| Diabetes Typ 2 | E11 |
| Dyslipidaemia | E78 |
| Adipositas | E66 |
| KHK | I20, I25 |
| Herzinsuffizienz | I50 |
| COPD | J44 |
| Asthma | J45 |
| Vorhofflimmern | I48 |
| PAVK | I70, I73 |
| Schlaganfall / TIA | I63, I64, G45 |
| Niereninsuffizienz (CKD) | N18 |
| Demenz | F00, F01, F02, F03, G30 |
| Depression | F32, F33 |
| Rheumatoide Arthritis | M05, M06 |

**Billing code 56544** = P4 flat rate, billed **twice per quarter** when ≥3 codes confirmed.

---

## Key Data: HAVG HZV Billing Codes (from Gegenüberstellung PDF)

Representative codes that may be addable based on patient record content
(full list to be parsed from the PDF during implementation):

| Code | Description |
|---|---|
| 56545 | Chroniker-Zuschlag |
| 56546 | Pflegeheim-Zuschlag |
| 56547 | Telefonkonsultation |
| 56548 | Hypertonie-Coaching |
| (full list from HAVG PDF) | ... |

---

## Boundaries / Rules to Follow

- `brassclaw_reborn_composition` must NOT depend on root `brassclaw` crate or `src/`
- webui_v2 handlers consume ONLY `RebornServicesApi` — no direct DB, no dispatcher
- Credential values (API password) must use `SecretString` — never appear in Debug/logs/responses
- Tenant/agent/project identity comes from `WebUiAuthenticatedCaller`, not the request body
- New routes must appear in the descriptor contract test
- Feature-gate with `tomedo-integration` so the binary doesn't force the dep on everyone
- The native daemon remains standalone — do not couple the monitor binary to any reborn crate

---

## Current Status

| Component | Status |
|---|---|
| `native-plugins/tomedo-monitor/` | ✅ Complete |
| `tools-src/tomedo/` (WASM — wrong arch) | ❌ To be deleted |
| Layer 1: Port traits (`tomedo.rs` in product_workflow) | ⬜ Not started |
| Layer 2: Facade integration | ⬜ Not started |
| Layer 3: Composition adapter | ⬜ Not started |
| Layer 4: HTTP descriptors + handlers | ⬜ Not started |
| Layer 5: Frontend JS | ⬜ Not started |
| Layer 6: Skill markdown | ⬜ Not started |
