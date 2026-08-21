# tomedo v3 — Extension Plan

> **Purpose:** This document defines every v3 artifact required to integrate the
> tomedo EMR REST API into BrassClaw Reborn as a first-class extension.
> It follows the same orchestrator-first, LLM-minimal design as `builtin_stuff_v3.md`.
>
> **Source of truth for the tomedo API:** `tomedo-crawl.cpp` and `docs/tomedo-crawl.md`
> from `https://github.com/chtugha/coding.agent` (probed live 2026-04-11).
>
> **Extension name:** `tomedo`
> **Extension slug:** `ext-tomedo` (class 23 ExtensionCatalogue)
>
> ---
>
> ## Core Design Principle: Orchestrator-First, LLM-Minimal
>
> Every tomedo operation that is deterministic (known URL, known field names,
> known response shape) MUST be Tier 0. The orchestrator calls Rust tools via
> `__execute_action__()` in a PythonCode step. The LLM is only involved when
> the task requires content composition, disambiguation, or irreversible action
> that needs confirmation.
>
> **Mandatory two-channel pattern (same as builtin_stuff_v3.md §tier0):**
> ```
> channel: "rust"           → pre-loads the ToolSkill binding (does NOT execute)
> channel: "orchestrator"   → PythonCode calls __execute_action__() to run the tool
> ```
>
> **Tomedo-specific Tier 0 eligibility:**
> All read-only tomedo API calls (GET endpoints) with a known patient ID or no
> parameters are Tier 0. Write operations do not exist in the tomedo REST API
> (it is read-only from the integration perspective). Every recipe below
> targets Tier 0 unless noted otherwise.
>
> **Auth guard (§tomedo-auth):**
> All tomedo tool calls require `tomedo_base_url` and `tomedo_cert_pem` config
> values to be set. PythonCode bodies include an auth-check guard that surfaces a
> clear error rather than failing silently when credentials are absent.
>
> **One leaf skill per approach rule (enforced):**
> Each of the 6 tomedo API function groups (server status, patient list, patient
> detail, patient relations, medications, appointments) gets its own leaf skill.
> The phone-index lookup (local SQLite via tomedo-crawl) gets its own leaf skill
> family. No monolithic "do everything" skill is authored.
>
> **Orchestrator call hierarchy:**
> ```
> The orchestrator NEVER calls Rust/tomedo directly.
> Rust makes the builtin.http tool available → ToolSkill binds it →
> PythonCode in the orchestrator channel calls __execute_action__("http", {...}).
> ```
>
> **tomedo-crawl sidecar vs direct tomedo API:**
> Two integration surfaces exist:
> 1. **Direct tomedo REST** (mTLS HTTPS to port 8443): used for all patient data
>    reads. Uses `builtin.http` with mTLS cert configured.
> 2. **tomedo-crawl sidecar HTTP** (loopback port 13181): used for phone lookup
>    (which the tomedo API cannot do server-side), RAG query, crawler control,
>    and service health. Uses `builtin.http` without cert (loopback plain HTTP
>    or shared TLS).
>
> Both surfaces are covered below with dedicated tools, skills, and recipes.
>
> ---
>
> ## API Reference Summary (from live probe + source code)
>
> ### tomedo REST API (port 8443, mTLS)
>
> | Method | Path | Tier | Description |
> |--------|------|------|-------------|
> | GET | `/{db}/serverstatus` | 0 | Server health check |
> | GET | `/{db}/patient?flach=true` | 0 | Flat patient list (~15k records, no phone) |
> | GET | `/{db}/patient/{id}` | 0 | Full patient record incl. phone numbers |
> | GET | `/{db}/patient/{id}/patientenDetailsRelationen?...` | 0 | Diagnoses, Kartei, Behandlungsfälle |
> | GET | `/{db}/patient/{id}/patientenDetailsRelationen/medikamentenPlan` | 0 | Medication plan |
> | GET | `/{db}/patient/{id}/termine?flach=true` | 0 | Appointments (flat) |
> | GET | `/{db}/besuch/{id}/besucheForPatient` | 0 | Visit records |
> | GET | `/{db}/patient/searchByAttributes?query={name}` | 1 | Name search (LLM composes query) |
>
> **Confirmed NOT available server-side:** phone number search
> (`searchByAttributes?telefonNummern=true → {}`)
>
> ### tomedo-crawl sidecar API (port 13181, loopback)
>
> | Method | Path | Tier | Description |
> |--------|------|------|-------------|
> | GET | `/health` | 0 | Crawl service status + indexed doc count |
> | POST | `/caller` | 0 | Register incoming call + phone for async lookup |
> | GET | `/caller/{call_id}` | 0 | Poll phone-lookup result |
> | DELETE | `/caller/{call_id}` | 0 | Deregister call on hangup |
> | GET | `/query?text=...&top_k=N&patient_id=N` | 0 | RAG semantic search |
> | POST | `/crawl/trigger` | 0 | Trigger immediate crawl |
> | POST | `/vectors/wipe` | 1 | Wipe vector store (destructive, needs confirm) |
> | GET | `/config` | 0 | Read all config keys |
> | POST | `/config` | 1 | Write config keys (needs confirm) |
> | GET | `/ollama/status` | 0 | Ollama embedding service status |
>
> ---
>
> ## Key JSON Field Reference (tomedo patient object)
>
> ### Flat list fields (`GET /patient?flach=true`)
> ```
> ident              — patient ID (integer)
> nachname           — family name
> vorname            — given name
> titel              — title (may be null)
> geburtsDatum       — birthdate as epoch ms (may be negative for pre-1970)
> ort                — city/town
> zuletztAufgerufen  — last-accessed epoch ms (used for incremental crawl)
> nachname_phonetic  — phonetic family name
> vorname_phonetic   — phonetic given name
> ```
>
> ### Full patient record fields (`GET /patient/{id}`)
> ```
> + patientenDetails.kontaktdaten.telefon          — main phone
> + patientenDetails.kontaktdaten.telefon2          — secondary phone
> + patientenDetails.kontaktdaten.handyNummer       — mobile
> + patientenDetails.kontaktdaten.telefon3          — tertiary phone
> + patientenDetails.kontaktdaten.weitereTelefonummern[] — additional numbers
> ```
>
> ### Relations fields (`GET /patient/{id}/patientenDetailsRelationen`)
> ```
> diagnosen[].freitext                — human-readable diagnosis text
> diagnosen[].typ                     — "G"|"V"|null (null → use freitext)
> diagnosen[].icdKatalogEintrag.ident — ICD catalog entry ID
> karteiEintraege[]                   — Kartei (medical record) entries
> behandlungsfaelle[]                 — treatment cases
> ```
>
> ### Medication plan fields (`GET /patient/{id}/patientenDetailsRelationen/medikamentenPlan`)
> ```
> nameBeiVerordnung          — medication name at prescription
> wirkstaerkeBeiVerordnung   — active substance strength
> darreichungBeiVerordnung   — dosage form
> dosierungFrueh             — morning dose
> dosierungMittag            — midday dose
> dosierungAbend             — evening dose
> dosierungNacht             — night dose
> ```
>
> ### Appointment fields (`GET /patient/{id}/termine?flach=true`)
> ```
> ident     — appointment ID
> beginn    — start epoch ms
> ende      — end epoch ms
> info      — description text
> ```
>
> ---


## Step 1 — Tool Rows (class 0)

Two tools cover all integration surfaces: one for the direct tomedo mTLS REST
API and one for the tomedo-crawl sidecar. Both wrap `builtin.http` at the
artifact level — they are logical capability declarations that tell the
orchestrator what is available and under what conditions.

> **Implementation note:** In the current v3 stack there is no dedicated
> `builtin.tomedo` Rust capability — all tomedo calls route through the
> existing `builtin.http` tool (which already handles HTTPS + cert). The
> tool rows below are *extension-level tool declarations* (class 0) that
> appear in the component catalog to make the orchestrator aware of the
> tomedo integration surface and its auth requirements.

---

### Step 1.1 — Tool: `tomedo-api` (class 0)

```
name:            "tomedo-api"
description:     "Make an authenticated HTTPS GET request to the tomedo EMR REST API.
                  Requires mTLS client certificate (PEM file path in tomedo_cert_pem
                  config). Base URL: https://{tomedo_host}:{tomedo_port}/{tomedo_db}/
                  Returns JSON. All tomedo REST endpoints are read-only GET calls.
                  Auth: Mutual TLS — no Authorization header needed.
                  Timeout: 15 000 ms per call (60 000 ms for the flat patient list)."
capability_id:   "builtin.http"
effect_type:     "read"
param_schema: {
  "type": "object",
  "properties": {
    "url":           {"type": "string", "description": "Full tomedo HTTPS URL"},
    "method":        {"type": "string", "enum": ["GET"], "description": "Always GET"},
    "headers":       {"type": "object", "description": "Optional extra headers"},
    "timeout_ms":    {"type": "number", "description": "Timeout in ms (default 15000, use 60000 for patient list)"},
    "cert_pem_path": {"type": "string", "description": "Path to mTLS client PEM file"}
  },
  "required": ["url"]
}
param_template:  {"url": "", "method": "GET"}
preconditions:   "tomedo_cert_pem config key must be set.
                  tomedo_host and tomedo_port must be reachable.
                  Network: LAN-only — tomedo server is on the practice LAN (e.g. 192.168.10.9:8443)."
error_handling:  "HTTP non-200: surface status code + body to orchestrator.
                  TLS error: surface as connection failure.
                  Timeout: 15 000 ms (60 000 ms for patient list endpoint)."
consumer_tags:   ["00:rusty", "02:orchestrator", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 1.2 — Tool: `tomedo-crawl-api` (class 0)

```
name:            "tomedo-crawl-api"
description:     "Make an HTTP request to the tomedo-crawl sidecar service running on
                  loopback port 13181. The sidecar provides: phone-number lookup
                  (GET /caller/{id}), RAG semantic search (GET /query), crawl control
                  (POST /crawl/trigger), service health (GET /health), and config
                  read/write (GET|POST /config).
                  No authentication required — loopback binding is the security boundary.
                  Returns JSON. Timeout: 5 000 ms."
capability_id:   "builtin.http"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "url":     {"type": "string", "description": "Full URL: http://127.0.0.1:13181/..."},
    "method":  {"type": "string", "enum": ["GET","POST","DELETE"], "description": "HTTP method"},
    "body":    {"type": "string", "description": "JSON body for POST requests"},
    "headers": {"type": "object", "description": "Optional headers"}
  },
  "required": ["url", "method"]
}
param_template:  {"url": "http://127.0.0.1:13181/health", "method": "GET"}
preconditions:   "tomedo-crawl sidecar must be running on 127.0.0.1:13181.
                  Check GET /health before issuing queries."
error_handling:  "Connection refused: sidecar not running — surface error to orchestrator.
                  HTTP 503 from /query: Ollama/embeddings unavailable.
                  HTTP 404 from /caller/{id}: call_id not registered."
consumer_tags:   ["00:rusty", "02:orchestrator", "05:validator"]
source:          "system"
validation_status: "validated"
```


## Step 2 — ToolSkills (class 13)

One ToolSkill per distinct call pattern. Each binds exactly one tool and
documents the exact URL shape, params, and response fields the executor needs.

---

### Step 2.1 — ToolSkill: `ts-tomedo-serverstatus` (class 13)

```
name:          "ts-tomedo-serverstatus"
tool_name:     "tomedo-api"
description:   "GET /{db}/serverstatus. Checks if the tomedo server is reachable and
                returns its software version and revision.
                Response: {status:'OK', softwareVersion:'...', revision:N}
                Use timeout_ms: 10000."
param_schema:  [
  {name: "url",          param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/serverstatus"},
  {name: "timeout_ms",   param_type: "number", required: false,
   description: "Timeout in ms, default 10000"}
]
param_template: {"url": "{{tomedo_base_url}}/serverstatus", "method": "GET", "timeout_ms": 10000}
preconditions:  "tomedo_cert_pem must be set. Server LAN-reachable."
error_handling: "Non-200 or connection refused → tomedo server offline."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.2 — ToolSkill: `ts-tomedo-patient-list` (class 13)

```
name:          "ts-tomedo-patient-list"
tool_name:     "tomedo-api"
description:   "GET /{db}/patient?flach=true. Returns all patients as a flat JSON array
                (~15 000 records). Fields per record: ident, nachname, vorname, titel,
                geburtsDatum (epoch ms, may be negative), ort, zuletztAufgerufen.
                Phone numbers are NOT included in the flat list. Use timeout_ms: 60000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/patient?flach=true"},
  {name: "timeout_ms", param_type: "number", required: false,
   description: "Use 60000 — full list response can be large"}
]
param_template: {"url": "{{tomedo_base_url}}/patient?flach=true", "method": "GET", "timeout_ms": 60000}
preconditions:  "tomedo_cert_pem must be set. Large response — do not use in-context without filtering."
error_handling: "Non-200 → auth or server error. Empty array → no patients or wrong db name."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.3 — ToolSkill: `ts-tomedo-patient-detail` (class 13)

```
name:          "ts-tomedo-patient-detail"
tool_name:     "tomedo-api"
description:   "GET /{db}/patient/{id}. Returns full patient record including phone
                numbers. Phone fields are nested at:
                  patientenDetails.kontaktdaten.telefon      (main)
                  patientenDetails.kontaktdaten.telefon2     (secondary)
                  patientenDetails.kontaktdaten.handyNummer  (mobile)
                  patientenDetails.kontaktdaten.telefon3     (tertiary)
                  patientenDetails.kontaktdaten.weitereTelefonummern[] (additional)
                Use timeout_ms: 15000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/patient/{id}"},
  {name: "timeout_ms", param_type: "number", required: false,
   description: "Timeout in ms, default 15000"}
]
param_template: {"url": "{{tomedo_base_url}}/patient/{{vars.patient_id}}", "method": "GET", "timeout_ms": 15000}
preconditions:  "patient_id must be a valid integer ident from the patient list."
error_handling: "HTTP 404 → patient_id not found. HTTP 401 → cert invalid."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.4 — ToolSkill: `ts-tomedo-patient-relations` (class 13)

```
name:          "ts-tomedo-patient-relations"
tool_name:     "tomedo-api"
description:   "GET /{db}/patient/{id}/patientenDetailsRelationen with limit params.
                Returns diagnoses, Kartei entries, Behandlungsfälle, Verordnungen.
                Key array: diagnosen[].{freitext, typ, icdKatalogEintrag.ident}
                  — freitext is the human-readable diagnosis text (primary field).
                  — typ is 'G' (gesichert), 'V' (Verdacht), or null.
                Recommended params: limitScheine=true&limitKartei=50
                  &limitVerordnungen=50&limitZeiterfassungen=true
                  &limitBehandlungsfaelle=true
                Use timeout_ms: 15000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "Full URL with limit params"},
  {name: "timeout_ms", param_type: "number", required: false,
   description: "Timeout in ms, default 15000"}
]
param_template: {
  "url": "{{tomedo_base_url}}/patient/{{vars.patient_id}}/patientenDetailsRelationen?limitScheine=true&limitKartei=50&limitVerordnungen=50&limitZeiterfassungen=true&limitBehandlungsfaelle=true",
  "method": "GET",
  "timeout_ms": 15000
}
preconditions:  "patient_id must be valid. Diagnosen array may be empty for new patients."
error_handling: "HTTP 404 → patient not found. Null typ → use freitext as primary label."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.5 — ToolSkill: `ts-tomedo-patient-medications` (class 13)

```
name:          "ts-tomedo-patient-medications"
tool_name:     "tomedo-api"
description:   "GET /{db}/patient/{id}/patientenDetailsRelationen/medikamentenPlan.
                Returns JSON array of medication plan entries. Per entry:
                  nameBeiVerordnung        — medication name at prescription time
                  wirkstaerkeBeiVerordnung — active substance strength
                  darreichungBeiVerordnung — dosage form code
                  dosierungFrueh           — morning dose (null if not prescribed)
                  dosierungMittag          — midday dose
                  dosierungAbend           — evening dose
                  dosierungNacht           — night dose
                Dosing notation: {frueh}-{mittag}-{abend} (e.g. '1-0-0.5')
                Use timeout_ms: 15000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/patient/{id}/patientenDetailsRelationen/medikamentenPlan"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url": "{{tomedo_base_url}}/patient/{{vars.patient_id}}/patientenDetailsRelationen/medikamentenPlan",
  "method": "GET",
  "timeout_ms": 15000
}
preconditions:  "patient_id must be valid. Empty array is valid (no active medications)."
error_handling: "HTTP 404 → patient not found. Null dose fields → medication not dosed in that interval."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.6 — ToolSkill: `ts-tomedo-patient-appointments` (class 13)

```
name:          "ts-tomedo-patient-appointments"
tool_name:     "tomedo-api"
description:   "GET /{db}/patient/{id}/termine?flach=true.
                Returns flat JSON array of appointments. Per entry:
                  ident  — appointment ID
                  beginn — start time as epoch ms
                  ende   — end time as epoch ms
                  info   — description text (may be null)
                To find the next future appointment: filter beginn > now_ms,
                sort ascending, take first. Use timeout_ms: 15000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/patient/{id}/termine?flach=true"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url": "{{tomedo_base_url}}/patient/{{vars.patient_id}}/termine?flach=true",
  "method": "GET",
  "timeout_ms": 15000
}
preconditions:  "patient_id must be valid. Empty array is valid (no appointments)."
error_handling: "HTTP 404 → patient not found. beginn=0 → skip entry."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.7 — ToolSkill: `ts-tomedo-patient-visits` (class 13)

```
name:          "ts-tomedo-patient-visits"
tool_name:     "tomedo-api"
description:   "GET /{db}/besuch/{id}/besucheForPatient.
                Returns visit/consultation records for a patient.
                Use timeout_ms: 15000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/besuch/{id}/besucheForPatient"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url": "{{tomedo_base_url}}/besuch/{{vars.patient_id}}/besucheForPatient",
  "method": "GET",
  "timeout_ms": 15000
}
preconditions:  "patient_id must be valid."
error_handling: "HTTP 404 → patient not found. Empty array → no visit records."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.8 — ToolSkill: `ts-tomedo-patient-search` (class 13)

```
name:          "ts-tomedo-patient-search"
tool_name:     "tomedo-api"
description:   "GET /{db}/patient/searchByAttributes?query={name}.
                Searches patients by name (partial match). Returns matching patient
                objects. NOTE: phone-number search does NOT work server-side
                (confirmed: telefonNummern=true returns empty dict). Name search only.
                Use timeout_ms: 15000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/patient/searchByAttributes?query={encoded_name}"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url": "{{tomedo_base_url}}/patient/searchByAttributes?query={{vars.query}}",
  "method": "GET",
  "timeout_ms": 15000
}
preconditions:  "query must be URL-encoded. Name search only — do NOT pass phone digits."
error_handling: "Empty dict {} → no match (not an array on empty). HTTP 401 → cert invalid."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.9 — ToolSkill: `ts-tomedo-crawl-health` (class 13)

```
name:          "ts-tomedo-crawl-health"
tool_name:     "tomedo-crawl-api"
description:   "GET http://127.0.0.1:13181/health.
                Returns tomedo-crawl sidecar status including indexed doc count,
                Ollama state, and last crawl timestamp.
                Response: {status:'ok', indexed_docs:N, index_usage_pct:N,
                           ollama_installed:bool, ollama_running:bool,
                           last_crawl: unix_timestamp_or_null}"
param_schema:  [
  {name: "url",    param_type: "string", required: true,
   description: "http://127.0.0.1:13181/health"},
  {name: "method", param_type: "string", required: true, description: "GET"}
]
param_template: {"url": "http://127.0.0.1:13181/health", "method": "GET"}
preconditions:  "tomedo-crawl sidecar must be running."
error_handling: "Connection refused → sidecar not running."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.10 — ToolSkill: `ts-tomedo-crawl-register-caller` (class 13)

```
name:          "ts-tomedo-crawl-register-caller"
tool_name:     "tomedo-crawl-api"
description:   "POST http://127.0.0.1:13181/caller.
                Registers an incoming call + phone number for async patient lookup.
                Request body: {call_id: integer, phone_number: string}
                Response: 202 {status:'pending'}
                The lookup completes asynchronously — poll GET /caller/{call_id}."
param_schema:  [
  {name: "url",    param_type: "string", required: true, description: "http://127.0.0.1:13181/caller"},
  {name: "method", param_type: "string", required: true, description: "POST"},
  {name: "body",   param_type: "string", required: true,
   description: "JSON: {call_id: N, phone_number: '07383-942735'}"}
]
param_template: {
  "url": "http://127.0.0.1:13181/caller",
  "method": "POST",
  "body": "{\"call_id\": {{vars.call_id}}, \"phone_number\": \"{{vars.phone_number}}\"}"
}
preconditions:  "call_id must be a unique integer for this call session. phone_number is the raw SIP From: number."
error_handling: "HTTP 400 → missing call_id. Connection refused → sidecar not running."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.11 — ToolSkill: `ts-tomedo-crawl-get-caller` (class 13)

```
name:          "ts-tomedo-crawl-get-caller"
tool_name:     "tomedo-crawl-api"
description:   "GET http://127.0.0.1:13181/caller/{call_id}.
                Polls the async patient-lookup result for a registered call.
                Response: {call_id:N, status:'pending'|'found'|'not_found'|'error',
                           name:string|null, vorname:string|null, patient_id:N,
                           all_patients:[{patient_id,name,vorname}]}
                Poll until status != 'pending' (typically resolves within 100 ms)."
param_schema:  [
  {name: "url",    param_type: "string", required: true,
   description: "http://127.0.0.1:13181/caller/{call_id}"},
  {name: "method", param_type: "string", required: true, description: "GET"}
]
param_template: {
  "url": "http://127.0.0.1:13181/caller/{{vars.call_id}}",
  "method": "GET"
}
preconditions:  "call_id must have been registered via POST /caller first."
error_handling: "HTTP 404 → call_id not registered or already deleted. status='error' → queue overflow."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.12 — ToolSkill: `ts-tomedo-crawl-rag-query` (class 13)

```
name:          "ts-tomedo-crawl-rag-query"
tool_name:     "tomedo-crawl-api"
description:   "GET http://127.0.0.1:13181/query?text={encoded}&top_k=N&patient_id=N.
                Semantic RAG search against the patient vector store embedded via Ollama.
                Returns top-K text chunks most semantically similar to the query text.
                Response: {results:[{text:string, source:'patient/N', patient_id:N, score:float}]}
                score is L2 distance (lower = more similar).
                Optional patient_id filter returns only that patient's chunks.
                Returns 503 if Ollama is unreachable."
param_schema:  [
  {name: "url",    param_type: "string", required: true,
   description: "http://127.0.0.1:13181/query?text={query}&top_k=N"},
  {name: "method", param_type: "string", required: true, description: "GET"}
]
param_template: {
  "url": "http://127.0.0.1:13181/query?text={{vars.query}}&top_k={{vars.top_k}}&patient_id={{vars.patient_id}}",
  "method": "GET"
}
preconditions:  "tomedo-crawl sidecar must be running. Ollama must be running and indexed_docs > 0."
error_handling: "HTTP 503 → Ollama not reachable. Empty results → no matching chunks (crawl may be needed)."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.13 — ToolSkill: `ts-tomedo-crawl-trigger` (class 13)

```
name:          "ts-tomedo-crawl-trigger"
tool_name:     "tomedo-crawl-api"
description:   "POST http://127.0.0.1:13181/crawl/trigger.
                Requests an immediate re-crawl of all tomedo patients.
                The crawl thread picks up the flag within 1 second.
                Response: 202 {status:'crawl_triggered'}
                Crawl duration: varies by patient count (~15k patients × 4 API calls each)."
param_schema:  [
  {name: "url",    param_type: "string", required: true,
   description: "http://127.0.0.1:13181/crawl/trigger"},
  {name: "method", param_type: "string", required: true, description: "POST"}
]
param_template: {"url": "http://127.0.0.1:13181/crawl/trigger", "method": "POST"}
preconditions:  "tomedo-crawl sidecar must be running."
error_handling: "Connection refused → sidecar not running."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.14 — ToolSkill: `ts-tomedo-crawl-config-read` (class 13)

```
name:          "ts-tomedo-crawl-config-read"
tool_name:     "tomedo-crawl-api"
description:   "GET http://127.0.0.1:13181/config.
                Returns all tomedo-crawl configuration keys from the encrypted SQLite
                config table. Keys: tomedo_host, tomedo_port, tomedo_db,
                tomedo_cert_pem, crawl_interval_sec, ollama_url, ollama_model,
                hnsw_max_elements."
param_schema:  [
  {name: "url",    param_type: "string", required: true,
   description: "http://127.0.0.1:13181/config"},
  {name: "method", param_type: "string", required: true, description: "GET"}
]
param_template: {"url": "http://127.0.0.1:13181/config", "method": "GET"}
preconditions:  "tomedo-crawl sidecar must be running."
error_handling: "Connection refused → sidecar not running."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```


## Step 3 — PythonCode Executors (class 22)

One PythonCode per distinct Tier-0 dispatch. Each calls `__execute_action__()`
exactly once with a hardcoded URL pattern. Slot values are baked in by IBS
before execution. No imports, no I/O, no network calls outside
`__execute_action__`.

---

### Step 3.1 — PythonCode: `pc-tomedo-serverstatus` (class 22)

```python
# Channel: orchestrator | Class: 22
# Dispatches ts-tomedo-serverstatus.
# IBS bakes in {{vars.tomedo_base_url}} before execution.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/serverstatus",
    "method": "GET",
    "timeout_ms": 10000
})
```

---

### Step 3.2 — PythonCode: `pc-tomedo-patient-list` (class 22)

```python
# Channel: orchestrator | Class: 22
# Dispatches ts-tomedo-patient-list.
# Returns all ~15k patients (flat, no phones). Large response — save to file.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/patient?flach=true",
    "method": "GET",
    "timeout_ms": 60000
})
```

---

### Step 3.3 — PythonCode: `pc-tomedo-patient-detail` (class 22)

```python
# Channel: orchestrator | Class: 22
# Dispatches ts-tomedo-patient-detail for a known patient_id.
# IBS bakes in {{vars.tomedo_base_url}} and {{vars.patient_id}}.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}",
    "method": "GET",
    "timeout_ms": 15000
})
```

---

### Step 3.4 — PythonCode: `pc-tomedo-patient-relations` (class 22)

```python
# Channel: orchestrator | Class: 22
# Dispatches ts-tomedo-patient-relations with standard limit params.
# Returns diagnoses (up to 50 Kartei, 50 Verordnungen).
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}/patientenDetailsRelationen?limitScheine=true&limitKartei=50&limitVerordnungen=50&limitZeiterfassungen=true&limitBehandlungsfaelle=true",
    "method": "GET",
    "timeout_ms": 15000
})
```

---

### Step 3.5 — PythonCode: `pc-tomedo-patient-medications` (class 22)

```python
# Channel: orchestrator | Class: 22
# Dispatches ts-tomedo-patient-medications.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}/patientenDetailsRelationen/medikamentenPlan",
    "method": "GET",
    "timeout_ms": 15000
})
```

---

### Step 3.6 — PythonCode: `pc-tomedo-patient-appointments` (class 22)

```python
# Channel: orchestrator | Class: 22
# Dispatches ts-tomedo-patient-appointments.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}/termine?flach=true",
    "method": "GET",
    "timeout_ms": 15000
})
```

---

### Step 3.7 — PythonCode: `pc-tomedo-patient-visits` (class 22)

```python
# Channel: orchestrator | Class: 22
# Dispatches ts-tomedo-patient-visits.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/besuch/{{vars.patient_id}}/besucheForPatient",
    "method": "GET",
    "timeout_ms": 15000
})
```

---

### Step 3.8 — PythonCode: `pc-tomedo-crawl-health` (class 22)

```python
# Channel: orchestrator | Class: 22
# Checks tomedo-crawl sidecar health. Fixed URL — no slots needed.
result = __execute_action__("tomedo-crawl-api", {
    "url": "http://127.0.0.1:13181/health",
    "method": "GET"
})
```

---

### Step 3.9 — PythonCode: `pc-tomedo-crawl-register-caller` (class 22)

```python
# Channel: orchestrator | Class: 22
# Registers an inbound call for phone-index lookup.
# IBS bakes in {{vars.call_id}} and {{vars.phone_number}}.
import json as _j
body = _j.dumps({"call_id": {{vars.call_id}}, "phone_number": "{{vars.phone_number}}"})
result = __execute_action__("tomedo-crawl-api", {
    "url": "http://127.0.0.1:13181/caller",
    "method": "POST",
    "body": body
})
```

---

### Step 3.10 — PythonCode: `pc-tomedo-crawl-get-caller` (class 22)

```python
# Channel: orchestrator | Class: 22
# Polls the caller-lookup result for a registered call_id.
# IBS bakes in {{vars.call_id}}.
result = __execute_action__("tomedo-crawl-api", {
    "url": "http://127.0.0.1:13181/caller/{{vars.call_id}}",
    "method": "GET"
})
```

---

### Step 3.11 — PythonCode: `pc-tomedo-crawl-rag-query` (class 22)

```python
# Channel: orchestrator | Class: 22
# Semantic RAG search against the patient vector store.
# IBS bakes in {{vars.query}}, {{vars.top_k}}, {{vars.patient_id}}.
# Use patient_id=-1 to search all patients.
import urllib.parse as _up
encoded = _up.quote("{{vars.query}}")
result = __execute_action__("tomedo-crawl-api", {
    "url": "http://127.0.0.1:13181/query?text=" + encoded + "&top_k={{vars.top_k}}&patient_id={{vars.patient_id}}",
    "method": "GET"
})
```

---

### Step 3.12 — PythonCode: `pc-tomedo-crawl-trigger` (class 22)

```python
# Channel: orchestrator | Class: 22
# Triggers an immediate re-crawl of the tomedo database.
# Fixed URL — no slots needed.
result = __execute_action__("tomedo-crawl-api", {
    "url": "http://127.0.0.1:13181/crawl/trigger",
    "method": "POST"
})
```

---

### Step 3.13 — PythonCode: `pc-tomedo-crawl-config-read` (class 22)

```python
# Channel: orchestrator | Class: 22
# Reads all tomedo-crawl config keys from the sidecar.
result = __execute_action__("tomedo-crawl-api", {
    "url": "http://127.0.0.1:13181/config",
    "method": "GET"
})
```

---

### Step 3.14 — Pure-logic PythonCode: `pc-tomedo-parse-diagnosen` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts up to max_count freitext diagnoses from a patientenDetailsRelationen
# response body (passed as the string {{vars.body}}).
# Returns a comma-separated list of up to 20 diagnoses.
import json as _j
try:
    data = _j.loads("{{vars.body}}")
    diagnosen = data.get("diagnosen", [])
    texts = []
    for d in diagnosen[:20]:
        ft = d.get("freitext", "")
        if ft:
            typ = d.get("typ", "")
            texts.append(ft + (" (" + typ + ")" if typ else ""))
    result = ", ".join(texts) if texts else "keine Diagnosen"
except Exception as e:
    result = "Fehler beim Parsen: " + str(e)
```

---

### Step 3.15 — Pure-logic PythonCode: `pc-tomedo-parse-medications` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Formats a medikamentenPlan array ({{vars.body}}) as human-readable text.
# Returns "Name morgens-mittags-abends; ..." for up to 20 medications.
import json as _j
try:
    meds = _j.loads("{{vars.body}}")
    lines = []
    for m in meds[:20]:
        name = m.get("nameBeiVerordnung", "")
        if not name:
            continue
        frueh = m.get("dosierungFrueh") or "0"
        mittag = m.get("dosierungMittag") or "0"
        abend = m.get("dosierungAbend") or "0"
        lines.append(name + " " + str(frueh) + "-" + str(mittag) + "-" + str(abend))
    result = "; ".join(lines) if lines else "keine Medikamente"
except Exception as e:
    result = "Fehler beim Parsen: " + str(e)
```

---

### Step 3.16 — Pure-logic PythonCode: `pc-tomedo-parse-next-appointment` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Finds the next future appointment from a termine array ({{vars.body}}).
# Returns formatted "DD.MM.YYYY, HH:MM Uhr (info)" or "kein Termin".
import json as _j, time as _t, datetime as _dt
try:
    termine = _j.loads("{{vars.body}}")
    now_ms = int(_t.time() * 1000)
    future = [a for a in termine if isinstance(a.get("beginn"), (int, float)) and a["beginn"] > now_ms]
    if not future:
        result = "kein Termin"
    else:
        best = min(future, key=lambda a: a["beginn"])
        dt = _dt.datetime.fromtimestamp(best["beginn"] / 1000)
        label = dt.strftime("%d.%m.%Y, %H:%M Uhr")
        info = best.get("info", "")
        result = label + (" (" + info + ")" if info else "")
except Exception as e:
    result = "Fehler beim Parsen: " + str(e)
```

---

### Step 3.17 — Pure-logic PythonCode: `pc-tomedo-epoch-to-date` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Converts a tomedo epoch-ms timestamp ({{vars.epoch_ms}}) to DD.MM.YYYY.
# Handles negative epoch values (patients born before 1970).
import datetime as _dt
try:
    ms = int("{{vars.epoch_ms}}")
    if ms == 0:
        result = "unbekannt"
    else:
        dt = _dt.datetime.utcfromtimestamp(ms / 1000)
        result = dt.strftime("%d.%m.%Y")
except Exception as e:
    result = "Fehler: " + str(e)
```

---

### Step 3.18 — Pure-logic PythonCode: `pc-tomedo-format-patient-context` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Assembles the RAG context document from pre-fetched fields.
# Slots: {{vars.vorname}}, {{vars.nachname}}, {{vars.patient_id}},
#        {{vars.geburtsdatum}}, {{vars.diagnosen}}, {{vars.medikamente}},
#        {{vars.termin}}, {{vars.telefon}}, {{vars.handy}}
lines = []
lines.append("Patient: {{vars.vorname}} {{vars.nachname}} (ID {{vars.patient_id}}), geb. {{vars.geburtsdatum}}")
if "{{vars.diagnosen}}":
    lines.append("Diagnosen: {{vars.diagnosen}}")
if "{{vars.medikamente}}":
    lines.append("Medikamente: {{vars.medikamente}}")
if "{{vars.termin}}":
    lines.append("Naechster Termin: {{vars.termin}}")
if "{{vars.telefon}}":
    lines.append("Telefon: {{vars.telefon}}")
if "{{vars.handy}}":
    lines.append("Handy: {{vars.handy}}")
result = "\n".join(lines)
```

---

### Step 3.19 — Pure-logic PythonCode: `pc-tomedo-extract-phone-fields` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts all phone number fields from a full patient record body.
# {{vars.body}} is the raw JSON string from GET /patient/{id}.
import json as _j
try:
    data = _j.loads("{{vars.body}}")
    kd = data.get("patientenDetails", {}).get("kontaktdaten", {})
    phones = {
        "telefon":  kd.get("telefon", "") or "",
        "telefon2": kd.get("telefon2", "") or "",
        "handy":    kd.get("handyNummer", "") or "",
        "telefon3": kd.get("telefon3", "") or "",
        "weitere":  [p for p in kd.get("weitereTelefonummern", []) if p]
    }
    result = phones
except Exception as e:
    result = {"error": str(e)}
```

---

### Step 3.20 — Pure-logic PythonCode: `pc-tomedo-filter-recent-patients` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Filters and sorts a flat patient list ({{vars.body}}) by zuletztAufgerufen
# descending, returning the top {{vars.limit}} most recently accessed patients.
import json as _j
try:
    patients = _j.loads("{{vars.body}}")
    limit = int("{{vars.limit}}")
    sorted_p = sorted(
        [p for p in patients if p.get("zuletztAufgerufen", 0) > 0],
        key=lambda p: p.get("zuletztAufgerufen", 0),
        reverse=True
    )
    result = sorted_p[:limit]
except Exception as e:
    result = []
```


## Step 4 — Leaf Skills (class 1) and Domain Skills (class 2)

One leaf skill per distinct approach. Domain skills reference leaves by name
only — no content duplication.

---

### Step 4.1 — Leaf Skill: `skill-tomedo-serverstatus` (class 1)

```
name:        "skill-tomedo-serverstatus"
class_code:  1
description: "Leaf skill: check whether the tomedo EMR server is reachable."
body: |
  Use ts-tomedo-serverstatus to check if the tomedo server is reachable.
  The URL is {{vars.tomedo_base_url}}/serverstatus.
  A successful response returns {status:'OK', softwareVersion, revision}.
  A non-200 or connection error means the server is offline or the cert is invalid.
  This is a Tier-0 health check — no LLM required.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.2 — Leaf Skill: `skill-tomedo-patient-list` (class 1)

```
name:        "skill-tomedo-patient-list"
class_code:  1
description: "Leaf skill: fetch the complete flat patient list from tomedo."
body: |
  Use ts-tomedo-patient-list to fetch all patients.
  URL: {{vars.tomedo_base_url}}/patient?flach=true
  Timeout: 60 000 ms — the response is large (~15k records).
  The flat list contains: ident, nachname, vorname, geburtsDatum (epoch ms),
  ort, zuletztAufgerufen. Phone numbers are NOT in the flat list.
  Always save large responses to a file via the http.save path and read back
  only the fields you need. Do NOT load 15k records into context.
  For searching by phone: use skill-tomedo-crawl-phone-lookup instead.
  For searching by name: use skill-tomedo-patient-search-by-name instead.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.3 — Leaf Skill: `skill-tomedo-patient-detail` (class 1)

```
name:        "skill-tomedo-patient-detail"
class_code:  1
description: "Leaf skill: fetch a full patient record by ID including phone numbers."
body: |
  Use ts-tomedo-patient-detail to fetch a single patient record by ID.
  URL: {{vars.tomedo_base_url}}/patient/{patient_id}
  Timeout: 15 000 ms.
  The response includes phone numbers nested at:
    patientenDetails.kontaktdaten.telefon       (main)
    patientenDetails.kontaktdaten.telefon2      (secondary)
    patientenDetails.kontaktdaten.handyNummer   (mobile)
    patientenDetails.kontaktdaten.weitereTelefonummern[] (additional)
  Extract phone fields with pc-tomedo-extract-phone-fields.
  patient_id must be a valid integer ident from the patient list or a
  caller-lookup result.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.4 — Leaf Skill: `skill-tomedo-patient-diagnoses` (class 1)

```
name:        "skill-tomedo-patient-diagnoses"
class_code:  1
description: "Leaf skill: fetch diagnoses for a patient via patientenDetailsRelationen."
body: |
  Use ts-tomedo-patient-relations to get the patient's diagnoses.
  URL: {{vars.tomedo_base_url}}/patient/{patient_id}/patientenDetailsRelationen
    ?limitScheine=true&limitKartei=50&limitVerordnungen=50
    &limitZeiterfassungen=true&limitBehandlungsfaelle=true
  Parse with pc-tomedo-parse-diagnosen.
  Key array: diagnosen[].freitext — human-readable text is the primary field.
  typ='G' means confirmed (gesichert), typ='V' means suspected (Verdacht),
  typ=null means use freitext only. Up to 20 diagnoses are returned.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.5 — Leaf Skill: `skill-tomedo-patient-medications` (class 1)

```
name:        "skill-tomedo-patient-medications"
class_code:  1
description: "Leaf skill: fetch the active medication plan for a patient."
body: |
  Use ts-tomedo-patient-medications to get the medication plan.
  URL: {{vars.tomedo_base_url}}/patient/{patient_id}/patientenDetailsRelationen/medikamentenPlan
  Parse with pc-tomedo-parse-medications.
  Dosing notation per medication: {frueh}-{mittag}-{abend} (e.g. '1-0-0.5').
  Null dose fields mean the medication is not dosed in that interval.
  Empty array means no active medications.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.6 — Leaf Skill: `skill-tomedo-patient-appointments` (class 1)

```
name:        "skill-tomedo-patient-appointments"
class_code:  1
description: "Leaf skill: fetch appointments for a patient and find the next one."
body: |
  Use ts-tomedo-patient-appointments to get the appointment list.
  URL: {{vars.tomedo_base_url}}/patient/{patient_id}/termine?flach=true
  Each appointment: ident, beginn (epoch ms), ende (epoch ms), info (text).
  To find the next future appointment: filter beginn > now_ms, sort ascending,
  take the first. Use pc-tomedo-parse-next-appointment for this.
  Empty array means no appointments scheduled.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.7 — Leaf Skill: `skill-tomedo-patient-visits` (class 1)

```
name:        "skill-tomedo-patient-visits"
class_code:  1
description: "Leaf skill: fetch visit/consultation records for a patient."
body: |
  Use ts-tomedo-patient-visits to fetch Besuch (visit) records.
  URL: {{vars.tomedo_base_url}}/besuch/{patient_id}/besucheForPatient
  Returns visit records for the patient's consultation history.
  Empty array means no recorded visits.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.8 — Leaf Skill: `skill-tomedo-patient-search-by-name` (class 1)

```
name:        "skill-tomedo-patient-search-by-name"
class_code:  1
description: "Leaf skill: search patients by name via the tomedo search endpoint."
body: |
  Use ts-tomedo-patient-search when you have a partial or full name to look up.
  URL: {{vars.tomedo_base_url}}/patient/searchByAttributes?query={encoded_name}
  The query must be URL-encoded.
  IMPORTANT: Phone-number search does NOT work server-side — confirmed that
  searchByAttributes?telefonNummern=true returns an empty dict, not an array.
  Name-only search via this endpoint. For phone lookup use the crawl sidecar.
  The LLM must compose the query from the user's intent (Tier 1).
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.9 — Leaf Skill: `skill-tomedo-crawl-health` (class 1)

```
name:        "skill-tomedo-crawl-health"
class_code:  1
description: "Leaf skill: check the tomedo-crawl sidecar health status."
body: |
  Use ts-tomedo-crawl-health to check if the tomedo-crawl sidecar is running.
  URL: http://127.0.0.1:13181/health (fixed loopback address)
  Response includes: indexed_docs (must be > 0 before queries work),
  ollama_running (must be true for RAG), last_crawl (Unix timestamp or null).
  If connection is refused, the sidecar is not running.
  Always check this before attempting RAG queries or phone lookups.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.10 — Leaf Skill: `skill-tomedo-crawl-phone-lookup` (class 1)

```
name:        "skill-tomedo-crawl-phone-lookup"
class_code:  1
description: "Leaf skill: look up a patient by phone number via the crawl sidecar."
body: |
  Phone lookup is a TWO-step process via the tomedo-crawl sidecar:
  Step 1: POST /caller with {call_id, phone_number} → registers the lookup.
          Use ts-tomedo-crawl-register-caller.
          Response is 202 immediately; lookup runs in background (~100ms).
  Step 2: GET /caller/{call_id} → polls for the result.
          Use ts-tomedo-crawl-get-caller.
          Possible statuses: 'pending' (still running), 'found' (patient identified),
          'not_found' (no match), 'error' (queue overflow).
  On 'found': name, vorname, patient_id, and all_patients[] are populated.
  On 'not_found': no patient in the phone_index matches the number.
  NEVER try direct tomedo REST phone search — confirmed it returns empty dict.
  When done: DELETE /caller/{call_id} to clean up the record.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.11 — Leaf Skill: `skill-tomedo-crawl-rag-query` (class 1)

```
name:        "skill-tomedo-crawl-rag-query"
class_code:  1
description: "Leaf skill: run a semantic RAG query against the patient vector store."
body: |
  Use ts-tomedo-crawl-rag-query to search the embedded patient context.
  URL: http://127.0.0.1:13181/query?text={encoded}&top_k=N&patient_id=N
  The query text is embedded via Ollama and matched against patient context chunks.
  Results are ranked by L2 distance (lower score = more similar).
  Use patient_id=-1 (or omit) to search across all patients.
  Pass a specific patient_id to restrict to that patient's chunks only.
  Returns 503 if Ollama is unreachable — check /health first.
  Empty results mean no indexed data yet — trigger a crawl.
  Use pc-tomedo-crawl-rag-query which URL-encodes the query string.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.12 — Leaf Skill: `skill-tomedo-crawl-trigger` (class 1)

```
name:        "skill-tomedo-crawl-trigger"
class_code:  1
description: "Leaf skill: trigger an immediate re-crawl of all tomedo patients."
body: |
  Use ts-tomedo-crawl-trigger to request an immediate crawl.
  POST http://127.0.0.1:13181/crawl/trigger → 202 {status:'crawl_triggered'}
  The crawl thread picks up the request within 1 second.
  Note: a full crawl of ~15k patients takes significant time (4 API calls each).
  After triggering, check /health to monitor indexed_docs progress.
  Only trigger when: first setup, cert was changed, or data is clearly stale.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.13 — Leaf Skill: `skill-tomedo-crawl-config-read` (class 1)

```
name:        "skill-tomedo-crawl-config-read"
class_code:  1
description: "Leaf skill: read tomedo-crawl configuration from the sidecar."
body: |
  Use ts-tomedo-crawl-config-read to inspect current sidecar configuration.
  GET http://127.0.0.1:13181/config
  Returns all config keys: tomedo_host, tomedo_port, tomedo_db,
  tomedo_cert_pem, crawl_interval_sec, ollama_url, ollama_model.
  This is a read-only diagnostic — config writes require LLM confirmation
  (Tier 1) because they change persistent service behavior.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.14 — Leaf Skill: `skill-tomedo-format-context` (class 1)

```
name:        "skill-tomedo-format-context"
class_code:  1
description: "Leaf skill: format collected patient data into a RAG context document."
body: |
  Use pc-tomedo-format-patient-context to assemble a context document after
  collecting the relevant data fields. The context document format is:
    Patient: {vorname} {nachname} (ID {ident}), geb. {geburtsDatum}
    Diagnosen: {diagnosen} (comma-separated freitext, max 20)
    Medikamente: {name} {frueh}-{mittag}-{abend}; ... (max 20)
    Naechster Termin: {datum, uhrzeit} ({info})
    Telefon: {telefon}
    Handy: {handy}
  This format is used by the LLM service for patient-context injection.
  Use pc-tomedo-epoch-to-date to convert geburtsDatum (epoch ms) to date string.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.15 — Domain Skill: `skill-tomedo` (class 2)

```
name:        "skill-tomedo"
class_code:  2
description: "Domain skill: full tomedo EMR integration — when and how to use each function."
body: |
  tomedo is a medical practice management system (EMR). This domain skill
  routes you to the correct leaf skill for each operation.

  ARCHITECTURE:
  Two integration surfaces:
    1. Direct tomedo REST API (mTLS HTTPS, port 8443) — patient data reads.
    2. tomedo-crawl sidecar (loopback HTTP, port 13181) — phone lookup, RAG,
       crawl control. Always check /health before querying the sidecar.

  OPERATION ROUTING:

  Server health:
    → skill-tomedo-serverstatus (Tier 0)

  Patient lookup by name:
    → skill-tomedo-patient-search-by-name (Tier 1, LLM composes query)

  Patient lookup by phone number:
    → skill-tomedo-crawl-phone-lookup (Tier 0, two-step: register + poll)
    NEVER use tomedo REST phone search — confirmed non-functional.

  Full patient data (known patient_id):
    • Contact + phones:     skill-tomedo-patient-detail (Tier 0)
    • Diagnoses:            skill-tomedo-patient-diagnoses (Tier 0)
    • Medications:          skill-tomedo-patient-medications (Tier 0)
    • Next appointment:     skill-tomedo-patient-appointments (Tier 0)
    • Visit history:        skill-tomedo-patient-visits (Tier 0)
    • Full context doc:     skill-tomedo-format-context (Tier 0, pure logic)

  Patient list operations:
    • All patients (flat):  skill-tomedo-patient-list (Tier 0, large — save to file)
    • Recent patients only: use pc-tomedo-filter-recent-patients after fetching

  Semantic RAG search:
    → skill-tomedo-crawl-rag-query (Tier 0)
    Requires: sidecar running, Ollama running, indexed_docs > 0

  Sidecar management:
    • Health check:      skill-tomedo-crawl-health (Tier 0)
    • Trigger crawl:     skill-tomedo-crawl-trigger (Tier 0)
    • Read config:       skill-tomedo-crawl-config-read (Tier 0)

  AUTH REQUIREMENT:
  All direct tomedo API calls require the mTLS client certificate PEM file.
  Check tomedo_cert_pem config before any direct API call.

  TIER-0 ELIGIBILITY:
  All read operations with a known patient_id are Tier 0. No tomedo write
  endpoints exist — the API is read-only from this integration. Name search
  is Tier 1 (LLM composes the query from user intent).
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.16 — Domain Skill: `skill-tomedo-crawl` (class 2)

```
name:        "skill-tomedo-crawl"
class_code:  2
description: "Domain skill: tomedo-crawl sidecar — phone lookup, RAG, and crawl management."
body: |
  The tomedo-crawl sidecar runs on loopback port 13181 and provides:

  PHONE LOOKUP (local SQLite, fast):
    1. POST /caller → registers call_id + phone_number
    2. GET /caller/{call_id} → polls until status != 'pending'
    3. DELETE /caller/{call_id} → cleanup on call hangup
    Skill: skill-tomedo-crawl-phone-lookup

  RAG SEMANTIC SEARCH:
    GET /query?text={encoded}&top_k=N&patient_id=N
    Requires Ollama running + indexed_docs > 0.
    Returns text chunks with L2 distance score (lower = more similar).
    Skill: skill-tomedo-crawl-rag-query

  CRAWL MANAGEMENT:
    POST /crawl/trigger → immediate re-crawl
    Skill: skill-tomedo-crawl-trigger

  STATUS + CONFIG:
    GET /health → service status, indexed_docs, Ollama state
    GET /config → all config keys
    Skills: skill-tomedo-crawl-health, skill-tomedo-crawl-config-read

  PREREQUISITES CHECKLIST before any sidecar operation:
    1. Connection to 127.0.0.1:13181 is accepted (sidecar running)
    2. GET /health returns status:'ok'
    3. For RAG queries: ollama_running:true AND indexed_docs > 0
    4. For phone lookup: indexed_docs > 0 (crawl populates phone_index)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```


## Step 5 — Recipes (class 21)

One recipe per distinct invocation pattern. All tomedo read operations with a
known patient_id are Tier 0. Intent examples cover the full German and English
natural-language range.

---

### Recipe: `tomedo-serverstatus` (class 21) — Tier 0

```
name:              "tomedo-serverstatus"
description:       "Check if the tomedo EMR server is reachable and return its version."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-serverstatus>", "<uuid:skill-tomedo>"],
    "label":   "Load tomedo server-status leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-serverstatus>"],
    "label":   "Pre-load ts-tomedo-serverstatus binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-serverstatus>"],
    "label":   "Execute: GET /{db}/serverstatus via __execute_action__"
  }
]
intent_examples: [
  {"input": "tomedo status",                              "class": 1},
  {"input": "ist tomedo erreichbar",                      "class": 2},
  {"input": "check tomedo server",                        "class": 2},
  {"input": "tomedo server health",                       "class": 2},
  {"input": "tomedo server prüfen",                       "class": 2},
  {"input": "is the tomedo server running",               "class": 3},
  {"input": "tomedo version",                             "class": 2},
  {"input": "praxissystem status",                        "class": 2},
  {"input": "tomedo online",                              "class": 1},
  {"input": "kann ich auf tomedo zugreifen",              "class": 3},
  {"input": "tomedo erreichbarkeit prüfen",               "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-detail` (class 21) — Tier 0

```
name:              "tomedo-patient-detail"
description:       "Fetch a full patient record by patient ID including all phone numbers."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-patient-detail>", "<uuid:skill-tomedo>"],
    "label":   "Load patient-detail leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-detail>"],
    "label":   "Pre-load ts-tomedo-patient-detail binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-detail>"],
    "label":   "Execute: GET /patient/{id} via __execute_action__"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-phone-fields>"],
    "label":   "Extract phone fields from response"
  }
]
intent_examples: [
  {"input": "patient details",                            "class": 2},
  {"input": "show patient 776",                           "class": 3},
  {"input": "patient daten abrufen",                      "class": 2},
  {"input": "phone number for patient 3892",              "class": 3},
  {"input": "telefonnummer von patient",                  "class": 2},
  {"input": "kontaktdaten für patient",                   "class": 2},
  {"input": "fetch patient record",                       "class": 2},
  {"input": "patientendetails abrufen",                   "class": 2},
  {"input": "get full record for patient id 1403",        "class": 3},
  {"input": "patient kontaktinformationen",               "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-diagnoses` (class 21) — Tier 0

```
name:              "tomedo-patient-diagnoses"
description:       "Fetch the confirmed and suspected diagnoses for a patient."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-patient-diagnoses>", "<uuid:skill-tomedo>"],
    "label":   "Load diagnoses leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-relations>"],
    "label":   "Pre-load ts-tomedo-patient-relations binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-relations>"],
    "label":   "Execute: GET /patient/{id}/patientenDetailsRelationen"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-parse-diagnosen>"],
    "label":   "Parse diagnosen array → comma-separated text"
  }
]
intent_examples: [
  {"input": "diagnosen patient",                          "class": 2},
  {"input": "diagnoses for patient",                      "class": 2},
  {"input": "was hat patient 776 für diagnosen",          "class": 3},
  {"input": "ICD Einträge patient",                       "class": 2},
  {"input": "medical diagnoses",                          "class": 2},
  {"input": "krankheiten des patienten",                  "class": 2},
  {"input": "diagnoseübersicht",                          "class": 1},
  {"input": "welche diagnosen hat der patient",           "class": 3},
  {"input": "patient diagnose abrufen",                   "class": 2},
  {"input": "gesicherte diagnosen",                       "class": 2},
  {"input": "verdachtsdiagnosen",                         "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-medications` (class 21) — Tier 0

```
name:              "tomedo-patient-medications"
description:       "Fetch the active medication plan for a patient with dosing schedule."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-patient-medications>", "<uuid:skill-tomedo>"],
    "label":   "Load medications leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-medications>"],
    "label":   "Pre-load ts-tomedo-patient-medications binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-medications>"],
    "label":   "Execute: GET /patient/{id}/.../medikamentenPlan"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-parse-medications>"],
    "label":   "Format medication list with dosing notation"
  }
]
intent_examples: [
  {"input": "medikamente patient",                        "class": 2},
  {"input": "medications for patient",                    "class": 2},
  {"input": "medikamentenplan",                           "class": 1},
  {"input": "welche medikamente nimmt der patient",       "class": 3},
  {"input": "drug list for patient 1403",                 "class": 3},
  {"input": "aktuelle medikation",                        "class": 2},
  {"input": "patient medication plan",                    "class": 2},
  {"input": "medikamente abrufen",                        "class": 2},
  {"input": "patient verschriebene medikamente",          "class": 2},
  {"input": "dosierung medikamente patient",              "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-next-appointment` (class 21) — Tier 0

```
name:              "tomedo-patient-next-appointment"
description:       "Fetch all appointments for a patient and return the next upcoming one."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-patient-appointments>", "<uuid:skill-tomedo>"],
    "label":   "Load appointments leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-appointments>"],
    "label":   "Pre-load ts-tomedo-patient-appointments binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-appointments>"],
    "label":   "Execute: GET /patient/{id}/termine?flach=true"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-parse-next-appointment>"],
    "label":   "Find next future appointment and format date/time"
  }
]
intent_examples: [
  {"input": "nächster termin patient",                    "class": 2},
  {"input": "next appointment for patient",               "class": 2},
  {"input": "wann hat patient 776 seinen nächsten termin","class": 3},
  {"input": "termine patient abrufen",                    "class": 2},
  {"input": "patient appointments",                       "class": 2},
  {"input": "nächsten termin anzeigen",                   "class": 2},
  {"input": "when is the next appointment",               "class": 3},
  {"input": "terminübersicht patient",                    "class": 2},
  {"input": "kommender termin patient",                   "class": 2},
  {"input": "appointment schedule",                       "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-visits` (class 21) — Tier 0

```
name:              "tomedo-patient-visits"
description:       "Fetch visit/consultation records for a patient."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-patient-visits>", "<uuid:skill-tomedo>"],
    "label":   "Load visits leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-visits>"],
    "label":   "Pre-load ts-tomedo-patient-visits binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-visits>"],
    "label":   "Execute: GET /besuch/{id}/besucheForPatient"
  }
]
intent_examples: [
  {"input": "besuche patient",                            "class": 2},
  {"input": "visit records patient",                      "class": 2},
  {"input": "behandlungshistorie patient",                "class": 2},
  {"input": "patient consultation history",               "class": 2},
  {"input": "arztbesuche patient abrufen",                "class": 2},
  {"input": "visit history for patient",                  "class": 2},
  {"input": "besuchsprotokoll",                           "class": 1},
  {"input": "krankenakte besuche",                        "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-phone-lookup` (class 21) — Tier 0

```
name:              "tomedo-phone-lookup"
description:       "Look up a patient by phone number using the tomedo-crawl phone index."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-crawl-phone-lookup>", "<uuid:skill-tomedo-crawl>"],
    "label":   "Load phone-lookup leaf + crawl domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-crawl-register-caller>"],
    "label":   "Pre-load ts-tomedo-crawl-register-caller binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-crawl-register-caller>"],
    "label":   "POST /caller: register call_id + phone_number"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-crawl-get-caller>"],
    "label":   "Pre-load ts-tomedo-crawl-get-caller binding"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-crawl-get-caller>"],
    "label":   "GET /caller/{call_id}: poll until status != pending"
  }
]
intent_examples: [
  {"input": "wer ruft an",                                "class": 1},
  {"input": "anrufer identifizieren",                     "class": 2},
  {"input": "phone number lookup",                        "class": 2},
  {"input": "patient by phone",                           "class": 2},
  {"input": "telefonnummer 07383942735 nachschlagen",     "class": 3},
  {"input": "caller identification",                      "class": 2},
  {"input": "wer ist diese telefonnummer",                "class": 3},
  {"input": "find patient by phone number",               "class": 2},
  {"input": "anrufer patient suchen",                     "class": 2},
  {"input": "telefonindex abfragen",                      "class": 2},
  {"input": "phone index lookup",                         "class": 2},
  {"input": "patient für anruf identifizieren",           "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-rag-query` (class 21) — Tier 0

```
name:              "tomedo-rag-query"
description:       "Semantic search across all patient context chunks via RAG."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-crawl-rag-query>", "<uuid:skill-tomedo-crawl>"],
    "label":   "Load RAG-query leaf + crawl domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-crawl-rag-query>"],
    "label":   "Pre-load ts-tomedo-crawl-rag-query binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-crawl-rag-query>"],
    "label":   "Execute: GET /query?text={encoded}&top_k=N"
  }
]
intent_examples: [
  {"input": "praxissystem durchsuchen",                   "class": 2},
  {"input": "rag query patient context",                  "class": 2},
  {"input": "semantic search patient data",               "class": 2},
  {"input": "suche in patientendaten",                    "class": 2},
  {"input": "find patients with hypertension",            "class": 3},
  {"input": "patienten mit diabetes suchen",              "class": 3},
  {"input": "context search tomedo",                      "class": 2},
  {"input": "volltext suche praxissystem",                "class": 2},
  {"input": "RAG Suche Patienten",                        "class": 2},
  {"input": "ähnliche patienten finden",                  "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-rag-query-for-patient` (class 21) — Tier 0

```
name:              "tomedo-rag-query-for-patient"
description:       "Semantic search scoped to a single patient's context chunks."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-crawl-rag-query>", "<uuid:skill-tomedo-crawl>"],
    "label":   "Load RAG-query leaf + crawl domain skill (patient-scoped)"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-crawl-rag-query>"],
    "label":   "Pre-load ts-tomedo-crawl-rag-query binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-crawl-rag-query>"],
    "label":   "Execute: GET /query?text={encoded}&top_k=3&patient_id={id}"
  }
]
intent_examples: [
  {"input": "search patient 776 context",                 "class": 3},
  {"input": "patientenkontext für patient 1403 durchsuchen","class": 3},
  {"input": "RAG query for specific patient",             "class": 2},
  {"input": "patient specific rag search",                "class": 2},
  {"input": "kontext von patient abrufen",                "class": 2},
  {"input": "patientenkontext suchen",                    "class": 2},
  {"input": "what does the record say about patient",     "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-crawl-health` (class 21) — Tier 0

```
name:              "tomedo-crawl-health"
description:       "Check tomedo-crawl sidecar health: indexed docs, Ollama state, last crawl."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-crawl-health>", "<uuid:skill-tomedo-crawl>"],
    "label":   "Load crawl-health leaf + crawl domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-crawl-health>"],
    "label":   "Pre-load ts-tomedo-crawl-health binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-crawl-health>"],
    "label":   "Execute: GET /health via __execute_action__"
  }
]
intent_examples: [
  {"input": "tomedo crawl status",                        "class": 2},
  {"input": "rag service health",                         "class": 2},
  {"input": "wie viele patienten sind indiziert",         "class": 3},
  {"input": "how many docs indexed in tomedo",            "class": 3},
  {"input": "ist ollama für tomedo aktiv",                "class": 3},
  {"input": "tomedo sidecar status",                      "class": 2},
  {"input": "crawl service check",                        "class": 2},
  {"input": "letzter crawl zeitpunkt",                    "class": 2},
  {"input": "last crawl time tomedo",                     "class": 2},
  {"input": "indexed documents count",                    "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-crawl-trigger` (class 21) — Tier 0

```
name:              "tomedo-crawl-trigger"
description:       "Trigger an immediate re-crawl of the tomedo patient database."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-crawl-trigger>", "<uuid:skill-tomedo-crawl>"],
    "label":   "Load crawl-trigger leaf + crawl domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-crawl-trigger>"],
    "label":   "Pre-load ts-tomedo-crawl-trigger binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-crawl-trigger>"],
    "label":   "Execute: POST /crawl/trigger via __execute_action__"
  }
]
intent_examples: [
  {"input": "tomedo neu crawlen",                         "class": 2},
  {"input": "trigger tomedo crawl",                       "class": 2},
  {"input": "refresh tomedo data",                        "class": 2},
  {"input": "patienten neu indexieren",                   "class": 2},
  {"input": "start tomedo crawl",                         "class": 2},
  {"input": "crawl jetzt starten",                        "class": 2},
  {"input": "tomedo daten aktualisieren",                 "class": 2},
  {"input": "update patient index",                       "class": 2},
  {"input": "neu-indexierung auslösen",                   "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-crawl-config-read` (class 21) — Tier 0

```
name:              "tomedo-crawl-config-read"
description:       "Read all tomedo-crawl configuration keys from the sidecar."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-crawl-config-read>", "<uuid:skill-tomedo-crawl>"],
    "label":   "Load config-read leaf + crawl domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-crawl-config-read>"],
    "label":   "Pre-load ts-tomedo-crawl-config-read binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-crawl-config-read>"],
    "label":   "Execute: GET /config via __execute_action__"
  }
]
intent_examples: [
  {"input": "tomedo konfiguration anzeigen",              "class": 2},
  {"input": "show tomedo config",                         "class": 2},
  {"input": "tomedo einstellungen",                       "class": 2},
  {"input": "read tomedo crawl config",                   "class": 2},
  {"input": "welches embedding modell wird verwendet",    "class": 3},
  {"input": "tomedo host konfiguration",                  "class": 2},
  {"input": "config tomedo-crawl",                        "class": 2},
  {"input": "wie ist tomedo konfiguriert",                "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-full-context` (class 21) — Tier 0

```
name:              "tomedo-patient-full-context"
description:       "Fetch all available context for a patient: detail + diagnoses + medications + next appointment, then format as a RAG context document."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo>", "<uuid:skill-tomedo-format-context>"],
    "label":   "Load tomedo domain + format-context leaf skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-detail>"],
    "label":   "Pre-load patient-detail binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-detail>", "<uuid:pc-tomedo-extract-phone-fields>", "<uuid:pc-tomedo-epoch-to-date>"],
    "label":   "Fetch detail + extract phones + convert birthdate"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-relations>"],
    "label":   "Pre-load patient-relations binding"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-relations>", "<uuid:pc-tomedo-parse-diagnosen>"],
    "label":   "Fetch relations + parse diagnoses"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-medications>"],
    "label":   "Pre-load medications binding"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-medications>", "<uuid:pc-tomedo-parse-medications>"],
    "label":   "Fetch medications + format dosing"
  },
  {
    "step_id": "step-7",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-appointments>"],
    "label":   "Pre-load appointments binding"
  },
  {
    "step_id": "step-8",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-appointments>", "<uuid:pc-tomedo-parse-next-appointment>", "<uuid:pc-tomedo-format-patient-context>"],
    "label":   "Fetch appointments, find next, assemble final context doc"
  }
]
intent_examples: [
  {"input": "vollständiger patientenkontext",             "class": 2},
  {"input": "full patient context",                       "class": 2},
  {"input": "alle patientendaten für patient 776",        "class": 3},
  {"input": "patient context for llm",                    "class": 2},
  {"input": "patientenprofil erstellen",                  "class": 2},
  {"input": "complete patient profile",                   "class": 2},
  {"input": "patientenzusammenfassung",                   "class": 2},
  {"input": "anruf vorbereitung patient",                 "class": 2},
  {"input": "caller context preparation",                 "class": 2},
  {"input": "alle infos zu patient abrufen",              "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-search-by-name` (class 21) — Tier 1

```
name:              "tomedo-patient-search-by-name"
description:       "Search patients by name — LLM composes the URL-encoded query from user intent."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-patient-search-by-name>", "<uuid:skill-tomedo>"],
    "label":   "Load search-by-name leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM composes URL-encoded query from user's name input"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-search>"],
    "label":   "Pre-load ts-tomedo-patient-search binding"
  }
]
intent_examples: [
  {"input": "patient Herbert Arnold suchen",              "class": 3},
  {"input": "find patient Kunsch",                        "class": 3},
  {"input": "patient by name",                            "class": 2},
  {"input": "patient namenssuche",                        "class": 2},
  {"input": "suche patient Müller",                       "class": 3},
  {"input": "patient search by name",                     "class": 2},
  {"input": "nach patient suchen",                        "class": 2},
  {"input": "find Herbert in tomedo",                     "class": 3}
]
source: "system"
validation_status: "validated"
```


## Step 6 — ExtensionCatalogues (class 23)

Two catalogues: one per integration surface.

---

### ExtensionCatalogue: `ext-tomedo` (class 23)

```
name:        "ext-tomedo"
description: "tomedo EMR REST API integration — patient data, diagnoses, medications, appointments."
version:     "1.0"
overview_doc: |
  This catalogue covers all components needed to integrate with the tomedo
  practice management system (EMR) REST API via mutual TLS.

  BASE URL: https://{tomedo_host}:{tomedo_port}/{tomedo_db}/
  AUTH:     Mutual TLS (mTLS) client certificate — PEM file with cert + key.
            No Authorization header needed.
  PROTOCOL: All tomedo REST calls are read-only GET requests.

  CONFIRMED API SURFACE (probed live 2026-04-11):
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ GET /serverstatus                      → server version + revision       │
  │ GET /patient?flach=true                → flat list (~15k, no phones)     │
  │ GET /patient/{id}                      → full record + phone numbers     │
  │ GET /patient/{id}/patientenDetails...  → diagnoses, Kartei, Behandlung   │
  │ GET /patient/{id}/.../medikamentenPlan → medication plan + dosing        │
  │ GET /patient/{id}/termine?flach=true   → appointments                   │
  │ GET /besuch/{id}/besucheForPatient     → visit records                  │
  │ GET /patient/searchByAttributes?query= → name search ONLY               │
  └─────────────────────────────────────────────────────────────────────────┘
  NOT AVAILABLE: phone-number search (confirmed returns empty dict).

  TASK GROUPS:
  1. Health checks:  tomedo-serverstatus
  2. Patient reads:  tomedo-patient-detail, tomedo-patient-diagnoses,
                     tomedo-patient-medications, tomedo-patient-next-appointment,
                     tomedo-patient-visits
  3. Patient search: tomedo-patient-search-by-name (Tier 1)
  4. Full context:   tomedo-patient-full-context (composed)

  KEY DATA SHAPES:
  • geburtsDatum: epoch ms, may be negative (before 1970)
  • Phone fields: patientenDetails.kontaktdaten.{telefon,telefon2,handyNummer,telefon3}
  • Diagnoses: diagnosen[].freitext (primary) + typ ('G'=confirmed, 'V'=suspected)
  • Medications: nameBeiVerordnung + dosierungFrueh/Mittag/Abend/Nacht
  • Appointments: beginn/ende as epoch ms

task_groups: [
  {
    "group_name": "server-health",
    "summary": "Check tomedo server reachability and version",
    "recipe_ids": ["tomedo-serverstatus"]
  },
  {
    "group_name": "patient-reads",
    "summary": "Fetch patient data by known patient_id (all Tier 0)",
    "recipe_ids": [
      "tomedo-patient-detail",
      "tomedo-patient-diagnoses",
      "tomedo-patient-medications",
      "tomedo-patient-next-appointment",
      "tomedo-patient-visits",
      "tomedo-patient-full-context"
    ]
  },
  {
    "group_name": "patient-search",
    "summary": "Search patients by name (Tier 1, LLM required)",
    "recipe_ids": ["tomedo-patient-search-by-name"]
  }
]
consumer_tags:   ["02:orchestrator", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### ExtensionCatalogue: `ext-tomedo-crawl` (class 23)

```
name:        "ext-tomedo-crawl"
description: "tomedo-crawl sidecar integration — phone lookup, RAG search, crawl management."
version:     "1.0"
overview_doc: |
  This catalogue covers all components for the tomedo-crawl sidecar service
  running on loopback port 13181.

  BASE URL: http://127.0.0.1:13181/
  AUTH:     None — loopback binding is the security boundary.
  NOTE:     Sidecar must be running. Check /health before any operation.

  SIDECAR API SURFACE:
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ GET  /health                → service status, indexed_docs, Ollama      │
  │ POST /caller                → register incoming call + phone number      │
  │ GET  /caller/{call_id}      → poll async phone-lookup result             │
  │ DELETE /caller/{call_id}    → deregister call on hangup                  │
  │ GET  /query?text=...        → RAG semantic search (top-K chunks)         │
  │ POST /crawl/trigger         → trigger immediate re-crawl                 │
  │ GET  /config                → read all sidecar config keys               │
  └─────────────────────────────────────────────────────────────────────────┘

  WHY THE SIDECAR EXISTS:
  The tomedo REST API has no server-side phone-number search endpoint
  (confirmed: searchByAttributes?telefonNummern=true → empty dict).
  The sidecar builds a local phone_index SQLite table during each crawl
  and serves sub-100ms phone lookups from it.

  PHONE LOOKUP FLOW:
  1. POST /caller {call_id, phone_number} → 202 (async start)
  2. GET /caller/{call_id} → poll until status != 'pending'
     status values: 'found', 'not_found', 'error'
  3. DELETE /caller/{call_id} on hangup

  RAG QUERY:
  • Requires indexed_docs > 0 AND ollama_running:true
  • Returns results sorted by L2 distance (lower = more similar)
  • patient_id=-1 searches all patients

  TASK GROUPS:
  1. Health:        tomedo-crawl-health
  2. Phone lookup:  tomedo-phone-lookup
  3. RAG search:    tomedo-rag-query, tomedo-rag-query-for-patient
  4. Management:    tomedo-crawl-trigger, tomedo-crawl-config-read

task_groups: [
  {
    "group_name": "sidecar-health",
    "summary": "Check sidecar status, Ollama state, indexed doc count",
    "recipe_ids": ["tomedo-crawl-health"]
  },
  {
    "group_name": "phone-lookup",
    "summary": "Identify incoming caller by phone number (local SQLite index)",
    "recipe_ids": ["tomedo-phone-lookup"]
  },
  {
    "group_name": "rag-search",
    "summary": "Semantic search across patient context chunks",
    "recipe_ids": ["tomedo-rag-query", "tomedo-rag-query-for-patient"]
  },
  {
    "group_name": "crawl-management",
    "summary": "Control the crawl pipeline and read configuration",
    "recipe_ids": ["tomedo-crawl-trigger", "tomedo-crawl-config-read"]
  }
]
consumer_tags:   ["02:orchestrator", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

## Step 7 — Component Summary & Seeding Order

### Complete Component Count (tomedo v3 stack)

| Class | Count | Names |
|-------|-------|-------|
| 0 — Tool | 2 | `tomedo-api`, `tomedo-crawl-api` |
| 1 — Leaf Skill | 14 | `skill-tomedo-serverstatus` … `skill-tomedo-format-context` |
| 2 — Domain Skill | 2 | `skill-tomedo`, `skill-tomedo-crawl` |
| 13 — ToolSkill | 14 | `ts-tomedo-serverstatus` … `ts-tomedo-crawl-config-read` |
| 21 — Recipe | 13 | `tomedo-serverstatus` … `tomedo-patient-search-by-name` |
| 22 — PythonCode | 20 | `pc-tomedo-serverstatus` … `pc-tomedo-filter-recent-patients` |
| 23 — ExtensionCatalogue | 2 | `ext-tomedo`, `ext-tomedo-crawl` |
| **Total** | **67** | |

---

### Tier Classification Summary

| Tier | Recipes | Reason |
|------|---------|--------|
| **Tier 0** | 12 | All read ops with known params — deterministic, no LLM needed |
| **Tier 1** | 1 | `tomedo-patient-search-by-name` — LLM composes URL-encoded name query |

---

### Seeding Order (bootstrapped in this order per group)

```
Group 1 — Tools (class 0):
  1. tomedo-api
  2. tomedo-crawl-api

Group 2 — ToolSkills (class 13):
  3. ts-tomedo-serverstatus
  4. ts-tomedo-patient-list
  5. ts-tomedo-patient-detail
  6. ts-tomedo-patient-relations
  7. ts-tomedo-patient-medications
  8. ts-tomedo-patient-appointments
  9. ts-tomedo-patient-visits
  10. ts-tomedo-patient-search
  11. ts-tomedo-crawl-health
  12. ts-tomedo-crawl-register-caller
  13. ts-tomedo-crawl-get-caller
  14. ts-tomedo-crawl-rag-query
  15. ts-tomedo-crawl-trigger
  16. ts-tomedo-crawl-config-read

Group 3 — PythonCode executors (class 22, with __execute_action__):
  17. pc-tomedo-serverstatus
  18. pc-tomedo-patient-list
  19. pc-tomedo-patient-detail
  20. pc-tomedo-patient-relations
  21. pc-tomedo-patient-medications
  22. pc-tomedo-patient-appointments
  23. pc-tomedo-patient-visits
  24. pc-tomedo-crawl-health
  25. pc-tomedo-crawl-register-caller
  26. pc-tomedo-crawl-get-caller
  27. pc-tomedo-crawl-rag-query
  28. pc-tomedo-crawl-trigger
  29. pc-tomedo-crawl-config-read

Group 4 — PythonCode pure-logic helpers (class 22, no __execute_action__):
  30. pc-tomedo-parse-diagnosen
  31. pc-tomedo-parse-medications
  32. pc-tomedo-parse-next-appointment
  33. pc-tomedo-epoch-to-date
  34. pc-tomedo-format-patient-context
  35. pc-tomedo-extract-phone-fields
  36. pc-tomedo-filter-recent-patients

Group 5 — Leaf Skills (class 1):
  37. skill-tomedo-serverstatus
  38. skill-tomedo-patient-list
  39. skill-tomedo-patient-detail
  40. skill-tomedo-patient-diagnoses
  41. skill-tomedo-patient-medications
  42. skill-tomedo-patient-appointments
  43. skill-tomedo-patient-visits
  44. skill-tomedo-patient-search-by-name
  45. skill-tomedo-crawl-health
  46. skill-tomedo-crawl-phone-lookup
  47. skill-tomedo-crawl-rag-query
  48. skill-tomedo-crawl-trigger
  49. skill-tomedo-crawl-config-read
  50. skill-tomedo-format-context

Group 6 — Domain Skills (class 2):
  51. skill-tomedo
  52. skill-tomedo-crawl

Group 7 — Recipes (class 21):
  53. tomedo-serverstatus
  54. tomedo-patient-detail
  55. tomedo-patient-diagnoses
  56. tomedo-patient-medications
  57. tomedo-patient-next-appointment
  58. tomedo-patient-visits
  59. tomedo-phone-lookup
  60. tomedo-rag-query
  61. tomedo-rag-query-for-patient
  62. tomedo-crawl-health
  63. tomedo-crawl-trigger
  64. tomedo-crawl-config-read
  65. tomedo-patient-full-context
  66. tomedo-patient-search-by-name  (Tier 1)

Group 8 — ExtensionCatalogues (class 23):
  67. ext-tomedo
  68. ext-tomedo-crawl
```

> **Note:** Seeding happens after all lower-dependency classes are seeded.
> ExtensionCatalogues are seeded last because they reference all other components
> as `child_component_ids`. All components are seeded with `source: 'system'` and
> `validation_status: 'validated'` directly (no Q1 queue pass needed for
> system-seeded components per builtin_bootstrap design).

---

### Idempotency Guard

All components use `ON CONFLICT (tenant_id, user_id, agent_id, project_id, name) DO NOTHING`
(or equivalent upsert) so repeated bootstraps are safe. Content hash is
computed on first insert and checked on subsequent runs.

---

### Key Design Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Both surfaces via `builtin.http` | No separate Rust capability needed — http already handles mTLS |
| 14 ToolSkills (not 2) | One per distinct URL pattern — maps to exact recipe steps |
| 13 PythonCode executors + 7 pure-logic helpers | Executors call `__execute_action__`; helpers transform data without I/O |
| 14 leaf skills | One per distinct approach — enforces the one-function-per-skill rule |
| 12 Tier-0 recipes | All known-ID read ops are deterministic — no LLM needed |
| 1 Tier-1 recipe | Name search only — LLM must compose URL-encoded query |
| `tomedo-patient-full-context` | Multi-step composed recipe; always chains the same 4 calls |
| Phone lookup via sidecar only | Confirmed: server-side phone search returns `{}` (non-functional) |
| German + English intent examples | Praxis staff speak German; orchestrator must handle both |

