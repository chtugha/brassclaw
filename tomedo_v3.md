# tomedo v3 — Extension Plan

> **Purpose:** This document defines every v3 artifact required to integrate the
> tomedo EMR REST API into BrassClaw Reborn as a first-class extension.
> It follows the same orchestrator-first, LLM-minimal design as `builtin_stuff_v3.md`.
>
> **Source of truth for the tomedo API:** live probe of server 192.168.10.9
> (2026-08-22) + api-connector.jar decompilation + tomedo Handbuch (support.tomedo.de).
>
> **Extension name:** `tomedo`
> **Extension slug:** `ext-tomedo` (class 23 ExtensionCatalogue)
>
> **Test patient:** ident `13550` — `Test, Toni`, DOB 1989-12-31. Use for all write tests.
> **DB name:** `tomedo_live`  **Server:** `192.168.10.9:8443` (mTLS)
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
> All tomedo API calls (GET/POST/PUT) with known parameters are Tier 0 for reads.
> Write operations are **Tier 1** — LLM confirms content, orchestrator executes.
> The **direct mTLS REST API at port 8443 accepts both reads AND writes** (confirmed
> live 2026-08-22) — no partner agreement needed. The tomedo.API gateway is an
> optional additional path but not required.
> LLM-assisted composition of tomedo objects (Python markers, SQL statistics,
> letter templates, CustomKarteiEinträge, patient forms) is Tier 1 — the LLM
> generates code/XML/JSON that the user then pastes into the tomedo UI.
> Every recipe below targets Tier 0 unless noted otherwise.
>
> **⚠️ CRITICAL SAFETY CONSTRAINT — `GET /patient?flach=true` is a BULK ENDPOINT:**
> This endpoint returns ~15 000 patient records in a single JSON response
> (typically 8–20 MB). Calling it directly from a recipe step or loading the
> response into LLM context WILL cause server memory exhaustion and a process
> crash. It is only permitted as input to `pc-tomedo-filter-recent-patients`
> (which immediately trims the list before any further use) or to feed an
> offline crawl pipeline. **No skill, recipe, or LLM prompt may invoke
> `ts-tomedo-patient-list` without an immediate filter/reduce step.**
>
> **⚠️ SECOND SAFETY RULE — NEVER use open-ended collection endpoints:**
> Any endpoint without a specific object ident (e.g. `GET /leistung?patient=X`,
> `GET /schein?patient=X`, `GET /patient/X/leistungen`) triggers unbounded server-side
> queries that can crash the tomedo process. Confirmed by live probe 2026-08-22.
> **Always use the patientenDetailsRelationen endpoint with limit params** to fetch
> related data — it is the only safe multi-object read endpoint.
>
> **Safe read pattern (confirmed working):**
> ```
> GET /{db}/patient/{id}                                          — single patient
> GET /{db}/patient/{id}/patientenDetailsRelationen?limitScheine=true&limitKartei=50&limitVerordnungen=50&limitZeiterfassungen=true&limitBehandlungsfaelle=true
> GET /{db}/patient/{id}/patientenDetailsRelationen/medikamentenPlan
> GET /{db}/patient/{id}/termine?flach=true
> GET /{db}/besuch/{id}/besucheForPatient                        — needs besuch ident, not patient ident
> GET /{db}/serverstatus
> ```
> **Never call without a specific ident:** `/leistung?patient=X`, `/schein?patient=X`,
> `/patient/X/leistungen`, `/patient/X/scheine`, `/patient/X/besuche` — these are
> unbounded and crash the server.
>
> **For all patient-search use-cases, prefer in order:**
> 1. `ts-tomedo-crawl-rag-query` — semantic/phonetic/phone search via sidecar RAG index (safe)
> 2. `ts-tomedo-patient-list` + `pc-tomedo-filter-recent-patients` — **bulk pull, only for crawl delta sync, never for interactive queries**
> 3. ~~`searchByAttributes`~~ — **confirmed broken**, returns `{}` for all queries
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
> ## API Reference Summary (confirmed by live probe 2026-08-22)
>
> **Test patient:** `ident=13550`, `Test Toni`, DOB 1989-12-31 — use this for all
> write/read probing. Never use real patients for API tests.
>
> **DB name confirmed:** `tomedo_live`
>
> **Direct access from BrassClaw host:** cert files in `/tmp/tomedo-certs/` —
> `client_certificate.pem`, `client_private_key.pem`, `root_certificate.pem`
> (copied from server `/opt/data/apiConnector/ssl/`).
>
> ### tomedo REST API (port 8443, mTLS) — READ endpoints
>
> | Method | Path | Status | Description |
> |--------|------|--------|-------------|
> | GET | `/{db}/serverstatus` | ✅ confirmed | Server health, version, backup status |
> | GET | `/{db}/patient/{id}` | ✅ confirmed | Full patient record incl. phone numbers |
> | GET | `/{db}/patient/{id}/patientenDetailsRelationen?limitScheine=true&limitKartei=50&...` | ✅ safe | Diagnoses, Kartei, Behandlungsfälle — **always use limit params** |
> | GET | `/{db}/patient/{id}/patientenDetailsRelationen/medikamentenPlan` | ✅ safe | Medication plan (array) |
> | GET | `/{db}/patient/{id}/termine?flach=true` | ✅ safe | Appointments (flat array) |
> | GET | `/{db}/besuch/{besuch_id}/besucheForPatient` | ✅ safe | Visit records — needs **besuch ident**, not patient ident |
> | GET | `/{db}/patient?flach=true` | ⚠️ BULK/CRASH | ~15k records — crawl pipeline + filter only, never interactive |
> | GET | `/{db}/patient/searchByAttributes?query={name}` | ❌ BROKEN | Returns `{}` for all queries (server-side bug, confirmed) |
> | GET | `/{db}/leistung?patient={id}` | 💀 CRASH | Unbounded query — crashes server, confirmed 2026-08-22 |
> | GET | `/{db}/schein?patient={id}` | 💀 CRASH | Unbounded query — crashes server |
> | GET | `/{db}/patient/{id}/leistungen` | 💀 CRASH | Unbounded query — crashes server |
> | GET | `/{db}/patient/{id}/scheine` | 💀 CRASH | Unbounded query — crashes server |
> | GET | `/{db}/patient/{id}/besuche` | 💀 CRASH | Unbounded query — crashes server |
>
> **`searchByAttributes` is confirmed broken** — returns `{}` for all queries.
> Use the crawl sidecar RAG endpoint for name/phonetic/phone search instead.
>
> **Leistungen are inside `kvSchein.ebmLeistungen[]` / `kvSchein.goaeLeistungen[]`**
> — not in patientenDetailsRelationen. Fetch path (confirmed live 2026-08-22):
> ```
> 1. GET /{db}/patient/{id}/patientenDetailsRelationen?limitScheine=true  → kvScheine[].ident
> 2. GET /{db}/kvschein/{schein_ident}  → ebmLeistungen[], goaeLeistungen[]
> ```
>
> **EBMLeistung field schema** (confirmed from test patient ident 13550, schein 192567376632348672):
> ```
> subentityType:     "EBMLeistung"
> ident:             (large int — the Leistung ID)
> datum:             epoch ms
> visible:           bool
> anzahl:            int (count, default 1)
> ebmKatalogEintrag: {ident: N}  ← catalog entry ident, NOT the EBM Ziffer string directly
> leistungserbringer:{ident: N}  ← Arzt ident
> betriebsstaette:   {ident: N}
> dokumentierenderNutzer: {ident: N}
> abrechnenderArzt:  {ident: N}
> ```
>
> **Key insight:** `ebmKatalogEintrag.ident` is an internal catalog ident (e.g. 270, 298),
> NOT the human-readable EBM Ziffer string (e.g. "03220"). To map catalog ident → EBM Ziffer,
> either look up `GET /{db}/ebmkatalogeintrag/{ident}` or read the Ziffer from the
> `l` Briefkommando which operates client-side on the open Schein.
>
> **The `$[l %nr 0d ,]$` Briefkommando** reads Leistungen from the currently-open
> Schein in the Mac client — it is client-side context only, not a REST path.
> The equivalent REST read is: GET /kvschein/{ident} → ebmLeistungen[].ebmKatalogEintrag.ident
>
> ### tomedo REST API (port 8443, mTLS) — WRITE endpoints (confirmed live 2026-08-22)
>
> **Critical finding: the direct mTLS REST API accepts writes.** The "read-only"
> assumption was wrong. All write endpoints return HTTP 200/204 with the same
> client cert used for reads. No partner agreement needed for writes.
>
> | Method | Path | Status | Returns | Notes |
> |--------|------|--------|---------|-------|
> | POST | `/{db}/karteieintrag` | ✅ HTTP 200 | `{new_ident}` | Creates KarteiEintrag; empty body creates blank entry |
> | POST | `/{db}/termin` | ✅ HTTP 200 | `{new_ident}` | Creates Termin; `removed:true` via PUT to cancel |
> | POST | `/{db}/patient` | ✅ HTTP 200 | `{new_ident}` | Creates patient; `gesperrt:1` (integer!) via PUT to block |
> | PUT  | `/{db}/karteieintrag/{id}` | ✅ HTTP 204 | — | Partial update; `visible:false` hides entry |
> | PUT  | `/{db}/termin/{id}` | ✅ HTTP 204 | — | Partial update; `removed:true` cancels |
> | PUT  | `/{db}/patient/{id}` | ✅ HTTP 204 | — | Partial update; `gesperrt:1` blocks patient |
> | DELETE | `/{db}/karteieintrag/{id}` | ❌ HTTP 405 | — | Not supported — use PUT `visible:false` |
> | DELETE | `/{db}/termin/{id}` | ❌ HTTP 405 | — | Not supported — use PUT `removed:true` |
>
> **Key type facts learned from live probe:**
> - `gesperrt` is **Integer** (not boolean) — send `1` not `true`
> - `removed` on Termin is **Boolean** — send `true`
> - `visible` on KarteiEintrag is **Boolean** — send `false` to hide
> - Error responses use HTTP 460 with full Java stack trace as body
> - All write responses are JSON; empty body POST creates a blank object with new ident
>
> **Write endpoint field schemas** (from GET responses on newly created objects):
> ```
> KarteiEintrag fields: datum, text, visible, primaer, karteiEintragTyp{kuerzel},
>   additionalText, status, betriebsstaette, diagnose, dokumentierenderNutzer, anhang[]
> Termin fields: beginn(epoch ms), ende(epoch ms), info, patient{ident}, terminArt,
>   behandler, removed, kalender[], warDa, telefon
> Patient fields: nachname, vorname, titel, geburtsDatum(epoch ms), gesperrt(int),
>   geburtsname, patientenDetails{kontaktdaten{...}, arzt, ...}
> ```
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
> ### tomedo LLM Service (OpenAI-compatible REST, via mTLS or LAN HTTP)
>
> | Method | Path | Tier | Description |
> |--------|------|------|-------------|
> | POST | `/{db}/llmservice/{user_ident}/v1/chat/completions` | 1 | OpenAI-compatible chat completions (DSGVO-compliant, zollsoft-hosted models) |
>
> **Why this matters:** The tomedo server exposes the same backend as the tomedo
> Kartei-Chat as a REST endpoint. BrassClaw can call it directly to run
> DSGVO-compliant LLM inference without external API keys or cloud services.
> All models are served via zollsoft-operated zero-retention infrastructure.
>
> **Endpoint forms:**
> - LAN HTTP: `http://tomedo.localnet:8080/tomedo_live/llmservice/{user_ident}/v1/chat/completions`
> - mTLS HTTPS: `https://{host}:8443/{db}/llmservice/{user_ident}/v1/chat/completions`
>
> **Models (confirmed by forum users, Sep–Nov 2025):**
> - `gemini-2.5-pro` — highest quality, higher latency
> - `gemini-2.5-flash` — fast, recommended for most tasks
> - `mistral-medium-2508` — EU/DSGVO-compliant Mistral model
>
> **Auth:** Same mTLS cert as the rest of the tomedo API (HTTPS variant).
> LAN HTTP variant requires no cert but must be on the practice LAN.
>
> **user_ident:** A numeric string — the tomedo user's ident from the statistics
> SQL table (`SELECT ident FROM t_benutzer WHERE login = ?`). Typically a
> low integer (e.g., "4").
>
> **Budget:** zollsoft imposes a monthly budget per user. Calls beyond the
> budget return an error response. Track usage and inform the user.
>
> **Request format:** OpenAI-compatible JSON:
> ```json
> {"model": "gemini-2.5-flash", "messages": [{"role":"user","content":"..."}], "stream": false}
> ```
>
> **Response:** OpenAI-compatible — content at `choices[0].message.content`.
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
>
> ## §write-paths — Write Paths to tomedo (Status as of 2026-08-22)
>
> **Probed:** 2026-08-22 via SSH + direct mTLS connection to 192.168.10.9:8443.
>
> ### Primary Write Path — Direct mTLS REST API (port 8443)
>
> **Status: ACTIVE — no partner agreement needed. Confirmed live 2026-08-22.**
>
> BrassClaw calls the tomedo REST API directly via HTTPS mTLS using the client
> certificate from `/opt/data/apiConnector/ssl/`. All write operations use the
> same `tomedo-api` tool with POST/PUT methods.
>
> **Confirmed working write endpoints:**
> - `POST /{db}/karteieintrag` — create KarteiEintrag → returns `{new_ident}`
> - `POST /{db}/termin` — create appointment → returns `{new_ident}`
> - `POST /{db}/patient` — create patient → returns `{new_ident}`
> - `PUT  /{db}/karteieintrag/{id}` — update; `visible:false` hides entry
> - `PUT  /{db}/termin/{id}` — update; `removed:true` cancels appointment
> - `PUT  /{db}/patient/{id}` — update; `gesperrt:1` (integer!) blocks patient
>
> **Type facts (from live probe):**
> - `gesperrt` is **Integer** — send `1` not `true`
> - `removed` on Termin is **Boolean** — send `true`
> - `visible` on KarteiEintrag is **Boolean** — send `false` to hide
> - Error responses: HTTP 460 with Java stack trace as body
> - DELETE is not supported (HTTP 405) — use PUT soft-delete instead
>
> **Write recipes are Tier 1:** LLM confirms content/intent, orchestrator POSTs/PUTs.
>
> ---
>
> ### Secondary Write Path — Official tomedo.API Gateway (optional, requires partner agreement)
>
> **Status:** API connector software **fully installed and running** on this server.
> Gateway is live at `https://api.tomedo.de`. Only Keycloak credentials are missing.
> **This is NOT required for v3** — use the direct mTLS path above instead.
>
> **Architecture (from decompiling `/opt/data/apiConnector/bin/api-connector.jar`):**
> ```
> BrassClaw (HTTPS, Keycloak JWT, E2E EC P-521)
>     ↕
> https://api.tomedo.de  (zollsoft Gateway Cloud)
>     ↕  CometD/Bayeux WebSocket
> api-connector.jar (port 8502, on tomedo server at 192.168.10.9)
>     ↕  mTLS client cert (already configured)
> tomedo server (port 8443)
> ```
>
> What is already in place: `api-connector.jar` running on port 8502, EC P-521 key
> pair generated, mTLS cert configured. Currently logs
> `"No Keycloak credentials are configured!"` every 5 minutes.
>
> To activate: sign the tomedo.API partner agreement with zollsoft GmbH and add
> Keycloak credentials to `/opt/data/apiConnector/config/properties.yaml`.
>
> ---
>
> ## §briefkommandos — Briefkommandos: Template Syntax and REST API Field Mapping
>
> **Source:** https://support.tomedo.de/handbuch/tomedo/kommunikation-mit-aerzten-patienten/briefschreibung/kommandos/uebersicht-nuetzlicher-briefkommandos/ (scraped live Aug 2026)
>
> **Primary purpose for BrassClaw:** Briefkommandos are the **data model documentation**
> for the REST API. Every `$[&p.some.path]$` keypath is directly readable via
> `GET /{db}/patient/{id}`. Every shorthand like `$[pn]$` or `$[l %nr 0d ,]$` reveals
> what field or computed value tomedo exposes — and what REST path to use server-side.
> Briefkommandos improve API endpoint design by showing exactly which fields exist,
> how they nest, and what computed/transient properties exist only client-side.
>
> **The derivation rule:** `$[&p.x.y.z]$` = REST JSON path `x.y.z` in `GET /{db}/patient/{id}`.
>
> **Computed/transient properties** (Briefkommando only, no REST equivalent):
> - `$[&p.currentAbrechnungsControllerObject.class.patInfoPanelMode]$` — EBM(1) vs GOÄ(2) mode
>   → server-side: check `currentKVSchein` vs `goaeScheine` presence in patientenDetailsRelationen
> - `$[l %nr ...]$` — reads from the **currently-open Schein on the Mac client**
>   → server-side: `GET /kvschein/{ident}` → `ebmLeistungen[]` (two-step safe path)
>
> ### Briefkommando Syntax Reference (all families, scraped Aug 2026)
>
> **Simple:** `$[kommandoname]$` — inserts a value directly
> **Keypath:** `$[&p.path.to.field]$` — CoreData traversal; `p`=patient, `pr`=Rechnung, `besuch`=visit
> **Parameterized:** `$[kommando param1 param2]$` — underscores become spaces in output
> **Conditional (2-branch):** `$[if expr op val trueText falseText]$`
> **Conditional (1-branch):** `$[if_then expr op val result]$` — empty string if false
> **Conditional (gender):** `$[if_frau femaleText maleText]$`
> **Multi-value if:** `$[if expr op val1 result1 val2 result2 ...]$` — matches first
> **Date:** `$[d source format]$` — S=system, B=last visit, E=last KarteiEintrag of type, L=last Leistung date
> **Date arithmetic:** `$[d S-2y yyyyMMdd]$` — subtract 2 years; `$[d S-152 yyyyMMdd]$` — subtract 152 days
> **Substring:** `$[bisZeichen N field]$` — first N chars of field
> **Count:** `$[c TYPE aq]$` — count KarteiEintrag of type in current quarter (aq-1=last, aq-2=prior)
> **x-Kommando:** `$[x TYPE count _ days flags]$` — flexible KarteiEintrag filter/retrieval
> **Leistung (client-side only):** `$[l format days separator]$` — reads from open Schein
> **Lab:** `$[laborwert TYPE KÜRZEL format position]$`
> **Form:** `$[formularEintrag KÜRZEL feldname N]$` / `$[customFormularEintrag KÜRZEL feld N]$`
> **Family:** `$[familie Beziehung placeholder]$` / `$[kommandorahmen ...]$`
> **Score:** `0$[v1]$+0$[v2]$` — CKE numeric fields; prepend 0 to avoid null errors
> **QR:** `$[qrCode -breite N -text URL]$`
> **Document:** `$[dokument Name -breite N]$` / `$[dokument Name -text]$`
> **Operators:** `zs_equals`, `zs_not_equal`, `zs_less_then`, `zs_contains`
> **Empty check:** use literal `<leer>` as the value to test for empty/null
>
> ### §briefkommandos-leistung — Leistung Kommandos → REST API equivalents
>
> This is the most important family for billing logic. The `l` kommando reads
> Leistungen from the **currently-open Schein on the Mac client** (client-side only).
> BrassClaw must replicate these checks server-side via the two-step kvschein path.
>
> | Briefkommando | What it does (client-side) | Server-side REST equivalent |
> |---|---|---|
> | `$[l %nr 0d ,]$` | All EBM Ziffern from today's open Schein | Step 1: `patientenDetailsRelationen?limitScheine=true` → `kvScheine[0].ident`; Step 2: `GET /kvschein/{ident}` → `ebmLeistungen[].ebmKatalogEintrag.ident`; Step 3: `GET /ebmkatalogeintrag/{ident}` → `.nummer` |
> | `$[l %nr 90d ,]$` | EBM Ziffern from Schein in last 90 days | Same path, filter `datum > now-90d` across all kvScheine |
> | `$[l EBM:_%nr]$` | EBM Ziffern with "EBM:" prefix | Same, format output |
> | `$[l %ops J ,_]$` | All OPS codes from selected Leistungen | `goaeLeistungen[].opsCode` (if present) |
> | `$[d L dd.MM.yyyy 30791]$` | Date last Ziffer 30791 was billed | Filter `ebmLeistungen` where resolved `.nummer='30791'`, sort desc, take `.datum` |
>
> **The EBM/GOÄ mode + Leistung pattern** (confirmed from live docs Aug 2026):
> ```
> $[if &p.currentAbrechnungsControllerObject.class.patInfoPanelMode zs_equals 1 EBM: 2 GOÄ:]$ $[l %nr 0d ,]$
> ```
> Outputs e.g. `EBM: 03220, 03221` from the open Schein.
> **`patInfoPanelMode` is a computed/transient property — not in the REST JSON.**
> BrassClaw server-side equivalent:
> 1. `patientenDetailsRelationen?limitScheine=true` → check `kvScheine` (non-empty = EBM) vs `goaeScheine` (non-empty = GOÄ)
> 2. Fetch `GET /kvschein/{ident}` for the most recent Schein → `ebmLeistungen[]`
> 3. Resolve each `ebmKatalogEintrag.ident` → `GET /ebmkatalogeintrag/{ident}` → `.nummer`
> 4. Prefix output "EBM:" or "GOÄ:" based on which Schein type was found
>
> ---
>
> ### §briefkommandos-karteieintrag — KarteiEintrag Kommandos → REST paths
>
> | Briefkommando | Purpose | Server-side REST path |
> |---|---|---|
> | `$[d E yyyyMMdd BMI]$` | Date of last BMI entry | `patientenDetailsRelationen.karteiEintraege[]` filter `karteiEintragTyp.kuerzel=BMI`, sort desc `.datum`, take first |
> | `$[d E yyyyMMdd LAB]$` | Date of last LAB entry | Same, filter `kuerzel=LAB` |
> | `$[c BES aq]$` | Count of BES entries this quarter | Count `karteiEintraege[]` where `kuerzel=BES` and `.datum` in current quarter |
> | `$[c BES aq-1]$` | Count last quarter | Same, previous quarter date range |
> | `$[c CAV]$` | Count of CAV entries | Count `karteiEintraege[]` where `kuerzel=CAV` |
> | `$[x BMI inf _ 152 ...]$` | Last BMI not older than 152d | `karteiEintraege[]` filter `kuerzel=BMI`, `datum > now-152d` |
> | `$[x NOT 1 _ 0-0 ...]$` | Text of first NOTiz entry | `karteiEintraege[kuerzel=NOT][0].text` |
> | `$[x (DDI;DIA) 1 _ inf ...]$` | Oldest active diagnosis | `diagnosen[]` sort asc, filter active |
> | `$[istMarkerGesetzt DMP/KHK]$` | Is marker set | `karteiEintraege[]` filter `kuerzel=DMP/KHK` and type=marker |
> | `$[karteiEintragValue_withArgs KÜRZEL customKarteiEintragEntries.V2 _N]$` | CKE field V2 | `karteiEintraege[kuerzel=KÜRZEL].customKarteiEintragEntries` field V2 |
> | `$[&p.inv_besuch.boolCol1]$` | Visit-dependent bool | `karteiEintraege[type=besuch].boolCol1` |
> | `$[&p.patientenDetails.boolCol1]$` | Patient-level bool | `patientenDetails.boolCol1` |
> | `$[laborwert LAB QUICK %w_%e L]$` | Lab value + unit | `karteiEintraege[kuerzel=LAB].laborauftrag.befunde[]` filter `lbTest.kuerzel=QUICK`; value=`lbTest.wert`, unit=`lbTest.einheit` |
> | `$[&p.patientenDetails.patientenDetailsRelationen.karteiEintraege.laborauftrag.befunde.lbTest.lbGNR.gebuehrennummer]$` | All lab GNR | Same path, collect all `.gebuehrennummer` values |
>
> ---
>
> ### §briefkommandos-field-table — Complete Field → Shorthand → REST path (scraped Aug 2026)
>
> **Patient:**
>
> | Field | Shorthand(s) | REST JSON path |
> |---|---|---|
> | Patient ID | `$[pid]$` | `ident` |
> | Family name | `$[pn]$` | `nachname` |
> | Given name | `$[pv]$` | `vorname` |
> | Title | `$[pt]$` | `titel` |
> | Full name | `$[pvoll]$` | — (computed) |
> | DOB | `$[pg]$` / `$[bes_gebDatum]$` / `$[pg0]$` / `$[pg2]$` | `geburtsDatum` (epoch ms) |
> | First initial + name | `$[bisZeichen 1 pv]$. $[pn]$` | — |
> | Street | `$[ps]$` / `$[patient_strasse]$` | `patientenDetails.kontaktdaten.adresse.strasse` |
> | Postcode | `$[pp]$` / `$[patient_plz]$` | `patientenDetails.kontaktdaten.adresse.plz` |
> | City | `$[po]$` / `$[patient_ort]$` | `patientenDetails.kontaktdaten.adresse.ort` |
> | Country | `$[pLand]$` | `patientenDetails.kontaktdaten.adresse.land` |
> | Country (2 chars) | `$[bisZeichen 2 pLand]$` | same |
> | Mobile | `$[phandy]$` | `patientenDetails.kontaktdaten.handyNummer` |
> | Phone | `$[ptel]$` | `patientenDetails.kontaktdaten.telefon` |
> | Fax | `$[patient_fax]$` | `patientenDetails.kontaktdaten.fax` |
> | E-mail | `$[pemail]$` | `patientenDetails.kontaktdaten.email` |
> | Gender | `$[pmw]$` / `$[pMW]$` / `$[pderdie]$` | — (computed) |
> | Occupation | `$[pb]$` | `patientenDetails.beruf` |
> | Employer | `$[agn]$` | `patientenDetails.arbeitgeberName` |
> | Insurance type | `$[patient_versichertenstatus]$` | — (GKV/PKV computed) |
> | Insurance number | `$[pversnr]$` | — |
> | Insurance name | `$[pk]$` | — |
> | Insurance IK | `$[kasse_ik]$` | — |
> | First registration | `$[erstaufnahme]$` | — |
> | Body height (BMI) | `$[koerpergroesseAusLetztemBMI]$` | `karteiEintraege[type=BMI].groesse` |
> | Body weight (BMI) | `$[gewichtAusLetztemBMI]$` | `karteiEintraege[type=BMI].gewicht` |
> | Systolic BP | `$[letzterBlutdruckSystolisch]$` | `karteiEintraege[type=BMI].blutdruckSystolisch` |
> | Diastolic BP | `$[etzterBlutdruckDiastolisch]$` | `karteiEintraege[type=BMI].blutdruckDiastolisch` |
> | First physician | `$[&p.patientenDetails.arzt]$` | `patientenDetails.arzt` |
> | PDF key | `$[&p.patientenDetails.PDFSchluessel]$` | `patientenDetails.PDFSchluessel` |
> | Has duplicate | `$[&self.besuch.patient.patientenDetails.hasDuplicates]$` | `patientenDetails.hasDuplicates` |
> | Maternity protection | `$[pmutterschutz]$` | — |
> | Delivery date | `$[d_geburt]$` | — |
> | Last menstruation | `$[letzteRegel]$` | — |
>
> **KV-Schein (via `patientenDetailsRelationen.currentKVSchein`):**
>
> | Field | Keypath Briefkommando | REST path |
> |---|---|---|
> | Current quarter | `$[&p.patientenDetails.patientenDetailsRelationen.currentKVSchein.quartalAsString]$` | `patientenDetailsRelationen.currentKVSchein.quartalAsString` |
> | Schein type code | `$[&p.patientenDetails.patientenDetailsRelationen.currentKVSchein.scheingruppe.code]$` | `patientenDetailsRelationen.currentKVSchein.scheingruppe.code` |
> | Kurativ/Präventiv | `$[&p.patientenDetails.patientenDetailsRelationen.currentKVSchein.kurativPraeventiv4221]$` | `patientenDetailsRelationen.currentKVSchein.kurativPraeventiv4221` |
> | Überweisungsauftrag | `$[&p.patientenDetails.patientenDetailsRelationen.currentKVSchein.auftragstext4205]$` | `patientenDetailsRelationen.currentKVSchein.auftragstext4205` |
> | Expected delivery (MTE) | `$[&p.patientenDetails.patientenDetailsRelationen.currentKVSchein.mutmasslicherTagDerEntbindung4206]$` | `patientenDetailsRelationen.currentKVSchein.mutmasslicherTagDerEntbindung4206` |
> | Referring physician | `$[&p.selektierterSchein.ueberweisenderArztName]$` | `selektierterSchein.ueberweisenderArztName` |
> | Karte nicht gesteckt | `besuch.karteNichtGesteckt` | — (computed, client-side) |
>
> **Invoice (pr = Rechnung):**
>
> | Field | Shorthand/Keypath | Notes |
> |---|---|---|
> | Paid | `$[&pr.bezahlt]$` | `1`=paid, `0`=unpaid |
> | Date paid | `$[&pr.bezahltDatum]$` | — |
> | Due date | `$[pr_faelligBis]$` | — |
> | Net sum | `$[pr_rechnungsSumme]$` | without VAT, without deductions |
> | Gross sum | `$[pr_rechnungsSummeReal]$` | with Mahngebühr, deductions, VAT |
> | VAT portion | `$[pr_rechnungsSummeMwst]$` | — |
> | Sum excl. VAT | `$[pr_rechnungsSummeRealOhneMwst]$` | — |
>
> **Letter/visit context:**
>
> | Field | Shorthand | Notes |
> |---|---|---|
> | Practice city | `$[ort]$` | Betriebsstätte |
> | Recipient address | `$[adressfeld_empfaenger]$` | — |
> | Recipient fax | `$[fax_empfaenger]$` | — |
> | Footer ("nachrichtlich an…") | `$[fusszeile]$` | — |
> | Images | `$[bilder]$` | Sono etc. |
> | Visit arrival time | `$[besuchAnkunftZeit]$` | — |
> | Today's date | `$[datum]$` / `$[d S dd.MM.yyyy]$` | — |
> | Last visit date | `$[d B]$` | — |
> | Weekday long | `$[d S EEEE]$` | "Donnerstag" |
> | Weekday short | `$[d S eee]$` | "Do" |
> | Current quarter | `$[d S q/yy]$` | "3/26" |
> | Upcoming appointments | `$[selektierteTermineListe %A:_%d]$` | — |
> | Past appointment (cert) | `$[selektierteTermineListe %x,den%doz_von_%v_Uhr_bis_%b_Uhr]$` | — |
>
> ---
>
> ### §briefkommandos-implications — What this means for BrassClaw API endpoint design
>
> When a user asks BrassClaw to check, display, or act on a piece of patient data,
> find the matching Briefkommando family, then use its REST equivalent:
>
> | User intent | Matching Briefkommando | BrassClaw REST action |
> |---|---|---|
> | "Which EBM codes billed today?" | `$[l %nr 0d ,]$` | `patientenDetailsRelationen?limitScheine=true` → `kvScheine[0].ident` → `GET /kvschein/{ident}` → resolve `ebmLeistungen` |
> | "Was Ziffer 03220 billed in 90d?" | `$[l %nr 90d ,]$` | Same, filter `datum > now-90d`, scan for Ziffer |
> | "When was 30791 last billed?" | `$[d L dd.MM.yyyy 30791]$` | Same, filter Ziffer=30791, max `.datum` |
> | "Is patient EBM or GOÄ?" | `$[if ...patInfoPanelMode...]$` | Check `kvScheine` vs `goaeScheine` presence in relations |
> | "Last BMI date?" | `$[d E yyyyMMdd BMI]$` | `karteiEintraege[]` filter `kuerzel=BMI`, max `.datum` |
> | "How many visits this quarter?" | `$[c BES aq]$` | Count `karteiEintraege[kuerzel=BES]` with date in current quarter |
> | "Lab QUICK value?" | `$[laborwert LAB QUICK %w_%e L]$` | `karteiEintraege[kuerzel=LAB].laborauftrag.befunde[]` filter `lbTest.kuerzel=QUICK` |
> | "CKE field value?" | `$[karteiEintragValue_withArgs KÜRZEL ...]$` | `karteiEintraege[kuerzel=KÜRZEL].customKarteiEintragEntries` |
> | "Marker DMP/KHK set?" | `$[istMarkerGesetzt DMP/KHK]$` | `karteiEintraege[]` filter type=marker, kuerzel matches |
> | "Patient on selective contract?" | `$[aktiveSelektivvertragsTeilnahmen]$` | `patientenDetails.teilnahmeauschlussSV` |
>
> When composing a Briefvorlage: check this table first → use shorthand if known →
> fall back to `$[&p.keypath]$` (= REST JSON path) → only call LLM for truly novel
> compound expressions.
>
> ---
>
> ## §llm-objects — LLM-Assisted tomedo Object Composition
>
> **Source:** tomedo support page (Aug 2026) + zollsoft forum posts (Sep 2025).
> **Summary:** tomedo provides context files (`.txt`/`.md`) that teach an LLM the
> syntax for each object type. The user pastes the context + their prompt into
> an external LLM (ChatGPT, Gemini, Ollama), gets back generated code, and pastes
> it into the tomedo UI. BrassClaw can automate the composition step: load the
> context, call the LLM, return the generated object ready to paste.
>
> **Available context files (from zollsoft):**
> | File | Object type | Output format | tomedo integration point |
> |------|-------------|--------------|--------------------------|
> | `pythonmarker_context.txt` | Automatic Python marker | Python code | Einstellungen > Praxisorganisation > Automatische Marker |
> | `statistikKompakt.txt` / `statistik_context.txt` | Custom statistics | SQL query | Statistik module |
> | `briefvorlage_context.txt` / `Briefvorlagen.txt` | Letter templates | HTML | Briefschreibung |
> | `briefkommandos_context.txt` / `BriefkommandosKompakt.txt` | Letter placeholders | Lookup reference | Used inside letter templates |
> | `patientenformular_context.txt` / `Patientenformulare.txt` | Patient forms | JSON (SurveyJS) | Patientenformulare > JSON tab |
> | `customkckeKonpakt.md` / `customkarteieintrag_context.txt` | CustomKarteiEinträge | XML | Kartei > CustomKarteiEintrag editor |
>
> **Tier classification:** All composition recipes are **Tier 1** — the LLM must
> compose the object. The orchestrator loads the context file content (via
> `builtin.file_read` or embedded as a skill body), then calls the LLM with the
> user's requirement as a prompt suffix.
>
> **Use cases confirmed by forum users (Dr. Baumann et al., Sep 2025):**
> 1. Python marker for birthday reminders, lab value alerts, diagnosis flags
> 2. SQL statistics for billing analysis (GOÄ/EBM ziffer queries)
> 3. CT/MRI report structured extraction templates (Briefvorlagen)
> 4. Tumour documentation CKEs with TNM, histology, therapy fields
> 5. Patient forms for intake anamnesis, consent, COVID triage
> 6. Sleep lab / pulmonology structured report templates
>
> **Architecture for BrassClaw:**
> ```
> channel: "orchestrator"  → Skill body contains full context file text embedded
> channel: "llm"           → LLM receives: context + user requirement
> channel: "orchestrator"  → PythonCode extracts generated code block from LLM response
> ```
> The orchestrator delivers the final output as a copy-pasteable code block
> for the user to insert into tomedo. No direct write to tomedo needed.
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
description:     "Make an authenticated HTTPS request to the tomedo EMR REST API.
                  Requires mTLS client certificate (PEM file path in tomedo_cert_pem
                  config). Base URL: https://{tomedo_host}:{tomedo_port}/{tomedo_db}/
                  Supports GET (reads) and POST/PUT (writes — confirmed live 2026-08-22).
                  Auth: Mutual TLS — no Authorization header needed.
                  Timeout: 15 000 ms per call (60 000 ms for the flat patient list).
                  For write calls always set Content-Type: application/json header."
capability_id:   "builtin.http"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "url":           {"type": "string", "description": "Full tomedo HTTPS URL"},
    "method":        {"type": "string", "enum": ["GET","POST","PUT"], "description": "HTTP method"},
    "headers":       {"type": "object", "description": "Include Content-Type: application/json for POST/PUT"},
    "body":          {"type": "string", "description": "JSON body string for POST/PUT requests"},
    "timeout_ms":    {"type": "number", "description": "Timeout in ms (default 15000, use 60000 for patient list)"},
    "cert_pem_path": {"type": "string", "description": "Path to mTLS client PEM file"}
  },
  "required": ["url", "method"]
}
param_template:  {"url": "", "method": "GET"}
preconditions:   "tomedo_cert_pem config key must be set.
                  tomedo_host and tomedo_port must be reachable.
                  Network: LAN-only — tomedo server is on the practice LAN (e.g. 192.168.10.9:8443).
                  For write ops (POST/PUT): Content-Type header must be application/json."
error_handling:  "HTTP non-200: surface status code + body to orchestrator.
                  HTTP 460: Java stack trace from tomedo — parse first line for error type.
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


### Step 1.3 — Tool: `tomedo-llm-api` (class 0)

```
name:            "tomedo-llm-api"
description:     "POST to the tomedo LLM service — an OpenAI-compatible /v1/chat/completions
                  endpoint exposed by the tomedo server itself.
                  This is the same backend as the tomedo Kartei-Chat, accessible via REST.
                  Models: gemini-2.5-pro, gemini-2.5-flash, mistral-medium-2508.
                  All models run on zollsoft-operated zero-retention infrastructure (DSGVO-compliant).

                  TWO endpoint variants (both supported):
                    HTTPS mTLS: https://{host}:{port}/{db}/llmservice/{user_ident}/v1/chat/completions
                    LAN HTTP:   http://tomedo.localnet:8080/tomedo_live/llmservice/{user_ident}/v1/chat/completions

                  Request body: {model, messages:[{role,content}], stream:false}
                  Response:     choices[0].message.content (OpenAI format)
                  Timeout:      60 000 ms (LLM inference — not a data fetch).
                  Budget:       Monthly per-user limit enforced by zollsoft. Budget errors surface
                                as non-200 or error body — always relay to user."
capability_id:   "builtin.http"
effect_type:     "read"
param_schema: {
  "type": "object",
  "properties": {
    "url":        {"type": "string",  "description": "Full LLM service URL including user_ident path segment"},
    "method":     {"type": "string",  "enum": ["POST"], "description": "Always POST"},
    "headers":    {"type": "object",  "description": "Must include Content-Type: application/json"},
    "body":       {"type": "string",  "description": "JSON string: {model, messages, stream:false}"},
    "timeout_ms": {"type": "number",  "description": "Use 60000 for LLM inference"}
  },
  "required": ["url", "body"]
}
param_template: {
  "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
  "method": "POST",
  "headers": {"Content-Type": "application/json"},
  "timeout_ms": 60000
}
preconditions:   "tomedo_user_ident config key must be set (numeric string from tomedo statistics/t_benutzer).
                  tomedo_llm_endpoint must be set (e.g. 'http://tomedo.localnet:8080/tomedo_live' or
                  'https://192.168.10.9:8443/live').
                  For HTTPS variant: tomedo_cert_pem must also be set.
                  For LAN HTTP variant: device must be on the practice LAN."
error_handling:  "Non-200: surface status + body — may indicate budget exhaustion.
                  TLS error (HTTPS variant): cert invalid or expired.
                  Timeout 60000 ms: inference too slow — retry with gemini-2.5-flash.
                  Empty choices[]: model error — surface to user."
consumer_tags:   ["00:rusty", "02:orchestrator", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

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
param_template: {"url": "{{vars.tomedo_base_url}}/serverstatus", "method": "GET", "timeout_ms": 10000}
preconditions:  "tomedo_cert_pem must be set. Server LAN-reachable."
error_handling: "Non-200 or connection refused → tomedo server offline."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.2 — ToolSkill: `ts-tomedo-patient-list` (class 13)

⚠️ **CRAWL-PIPELINE USE ONLY** — This ToolSkill returns ~15 000 records (8–20 MB).
It MUST only be dispatched from `pc-tomedo-filter-recent-patients` or an offline
crawl delta job. It is FORBIDDEN in interactive recipes or any step that passes
its output directly to the LLM context. For interactive patient lookup always use
`ts-tomedo-patient-search` (name) or `ts-tomedo-crawl-rag-query` (phone/fuzzy).

```
name:          "ts-tomedo-patient-list"
tool_name:     "tomedo-api"
description:   "GET /{db}/patient?flach=true. Returns ALL patients as a flat JSON array
                (~15 000 records, 8–20 MB). Fields per record: ident, nachname, vorname,
                titel, geburtsDatum (epoch ms, may be negative), ort, zuletztAufgerufen.
                Phone numbers are NOT included in the flat list.
                ⚠️ CRAWL-PIPELINE USE ONLY — never load this response into LLM context.
                Always pipe immediately to pc-tomedo-filter-recent-patients.
                Use timeout_ms: 60000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/patient?flach=true"},
  {name: "timeout_ms", param_type: "number", required: false,
   description: "Must be 60000 — full list response is large"}
]
param_template: {"url": "{{vars.tomedo_base_url}}/patient?flach=true", "method": "GET", "timeout_ms": 60000}
preconditions:  "tomedo_cert_pem must be set. MUST be followed by pc-tomedo-filter-recent-patients
                 or equivalent reduce step. DO NOT pass raw output to LLM or recipe context."
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
param_template: {"url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}", "method": "GET", "timeout_ms": 15000}
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
  "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}/patientenDetailsRelationen?limitScheine=true&limitKartei=50&limitVerordnungen=50&limitZeiterfassungen=true&limitBehandlungsfaelle=true",
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
  "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}/patientenDetailsRelationen/medikamentenPlan",
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
  "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}/termine?flach=true",
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
description:   "GET /{db}/besuch/{besuch_id}/besucheForPatient.
                Returns visit/consultation records for a patient.
                ⚠️ The URL segment is a BESUCH ident, not a patient ident.
                Obtain the besuch_ident from a prior patient-detail or
                patientenDetailsRelationen response (besuch[].ident).
                Use timeout_ms: 15000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/besuch/{besuch_ident}/besucheForPatient"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url": "{{vars.tomedo_base_url}}/besuch/{{vars.besuch_ident}}/besucheForPatient",
  "method": "GET",
  "timeout_ms": 15000
}
preconditions:  "besuch_ident must be a valid Besuch ident (NOT patient ident). Obtain from patientenDetailsRelationen response."
error_handling: "HTTP 404 → besuch_ident not found. Empty array → no visit records."
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
  "url": "{{vars.tomedo_base_url}}/patient/searchByAttributes?query={{vars.query}}",
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

---

### Step 2.15 — ToolSkill: `ts-tomedo-llm-chat` (class 13)

```
name:          "ts-tomedo-llm-chat"
tool_name:     "tomedo-llm-api"
description:   "POST /{db}/llmservice/{user_ident}/v1/chat/completions.
                Calls the tomedo LLM service with an OpenAI-compatible messages array.
                Returns a chat completion — extract content from choices[0].message.content.
                Timeout: 60000 ms.

                Supports three DSGVO-compliant models:
                  gemini-2.5-flash     — fast, recommended default
                  gemini-2.5-pro       — highest quality, higher latency
                  mistral-medium-2508  — Mistral EU-compliant alternative

                IMPORTANT: This ToolSkill is used for ALL prompt types (medical
                report extraction, translation, text analysis). The prompt content
                is the only differentiator — one ToolSkill, many PythonCode executors."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "Full URL: {llm_endpoint}/llmservice/{user_ident}/v1/chat/completions"},
  {name: "body",       param_type: "string", required: true,
   description: "JSON string: {model, messages:[{role,content}], stream:false}"},
  {name: "timeout_ms", param_type: "number", required: false,
   description: "Use 60000"}
]
param_template: {
  "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
  "method": "POST",
  "headers": {"Content-Type": "application/json"},
  "timeout_ms": 60000
}
preconditions:  "tomedo_user_ident and tomedo_llm_endpoint config keys must be set."
error_handling: "Non-200 → check for budget-exhaustion message in body. Timeout → try gemini-2.5-flash."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```


---

### Step 2.16 — ToolSkill: `ts-tomedo-karteieintrag-create` (class 13)

```
name:          "ts-tomedo-karteieintrag-create"
tool_name:     "tomedo-api"
description:   "POST /{db}/karteieintrag — create a new KarteiEintrag for a patient.
                Request body (JSON): {
                  patient:   {ident: N},
                  text:      'string',
                  datum:     epoch_ms,
                  visible:   true,
                  karteiEintragTyp: {kuerzel: 'BRIEF'|'AKTE'|...}
                }
                Returns: {ident: N} — the new KarteiEintrag ident.
                To soft-delete: PUT /{db}/karteieintrag/{ident} {visible: false}.
                Confirmed HTTP 200 on live tomedo 2026-08-22."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/karteieintrag"},
  {name: "method",  param_type: "string", required: true, description: "POST"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON: {patient:{ident:N}, text:'...', datum:epoch_ms, visible:true, karteiEintragTyp:{kuerzel:'...'}}"},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":    "{{vars.tomedo_base_url}}/karteieintrag",
  "method": "POST",
  "headers": {"Content-Type": "application/json"},
  "body":   "{\"patient\":{\"ident\":{{vars.patient_id}}},\"text\":\"{{vars.text}}\",\"datum\":{{vars.datum}},\"visible\":true,\"karteiEintragTyp\":{\"kuerzel\":\"{{vars.kuerzel}}\"}}"
}
preconditions:  "patient_id must be a known valid patient ident. datum is epoch ms. kuerzel is a valid Kartei type."
error_handling: "HTTP 460: tomedo rejected the body — parse first line of stack trace. HTTP 401: cert invalid."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.17 — ToolSkill: `ts-tomedo-karteieintrag-update` (class 13)

```
name:          "ts-tomedo-karteieintrag-update"
tool_name:     "tomedo-api"
description:   "PUT /{db}/karteieintrag/{ident} — partial update of an existing KarteiEintrag.
                Most common use: soft-delete with {visible: false}.
                Can also update: text, datum, karteiEintragTyp, additionalText.
                Returns: HTTP 204 No Content on success.
                Confirmed HTTP 204 on live tomedo 2026-08-22."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/karteieintrag/{karteieintrag_ident}"},
  {name: "method",  param_type: "string", required: true, description: "PUT"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON partial update, e.g. {visible: false} or {text: '...'}"},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":    "{{vars.tomedo_base_url}}/karteieintrag/{{vars.karteieintrag_ident}}",
  "method": "PUT",
  "headers": {"Content-Type": "application/json"},
  "body":   "{{vars.body_json}}"
}
preconditions:  "karteieintrag_ident must be an existing KarteiEintrag ident."
error_handling: "HTTP 460: field type mismatch (e.g. wrong type for boolean fields). HTTP 405: tried DELETE — not supported."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.18 — ToolSkill: `ts-tomedo-termin-create` (class 13)

```
name:          "ts-tomedo-termin-create"
tool_name:     "tomedo-api"
description:   "POST /{db}/termin — create a new appointment.
                Request body (JSON): {
                  patient:  {ident: N},
                  beginn:   epoch_ms,
                  ende:     epoch_ms,
                  info:     'reason string',
                  removed:  false,
                  warDa:    false
                }
                Returns: {ident: N} — the new Termin ident.
                To cancel: PUT /{db}/termin/{ident} {removed: true}.
                Confirmed HTTP 200 on live tomedo 2026-08-22."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/termin"},
  {name: "method",  param_type: "string", required: true, description: "POST"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON: {patient:{ident:N}, beginn:epoch_ms, ende:epoch_ms, info:'...', removed:false, warDa:false}"},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":    "{{vars.tomedo_base_url}}/termin",
  "method": "POST",
  "headers": {"Content-Type": "application/json"},
  "body":   "{\"patient\":{\"ident\":{{vars.patient_id}}},\"beginn\":{{vars.beginn}},\"ende\":{{vars.ende}},\"info\":\"{{vars.info}}\",\"removed\":false,\"warDa\":false}"
}
preconditions:  "patient_id must be valid. beginn/ende are epoch ms. LLM must confirm content before dispatch."
error_handling: "HTTP 460: field error. HTTP 200 with empty-shell object: body was malformed — verify JSON structure."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.19 — ToolSkill: `ts-tomedo-kvschein-get` (class 13)

```
name:          "ts-tomedo-kvschein-get"
tool_name:     "tomedo-api"
description:   "GET /{db}/kvschein/{schein_ident} — fetch a KV-Schein (billing quarter case)
                including its EBM and GOÄ Leistungen (billing codes).
                This is the ONLY safe way to read Leistungen — do NOT use
                /leistung?patient=X or /patient/{id}/leistungen (both crash the server).
                Safe Leistung read pattern:
                  1. GET patientenDetailsRelationen?limitScheine=true → kvScheine[].ident
                  2. GET /kvschein/{ident} → ebmLeistungen[], goaeLeistungen[]
                EBMLeistung fields: ident, datum(epoch ms), anzahl, ebmKatalogEintrag{ident},
                  leistungserbringer{ident}, visible.
                ebmKatalogEintrag.ident is an internal int (e.g. 270), NOT the EBM Ziffer string
                (e.g. '03220'). Map via GET /ebmkatalogeintrag/{ident} if needed.
                Confirmed HTTP 200 on live tomedo 2026-08-22."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/kvschein/{schein_ident}"},
  {name: "method",     param_type: "string", required: true, description: "GET"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url":        "{{vars.tomedo_base_url}}/kvschein/{{vars.schein_ident}}",
  "method":     "GET",
  "timeout_ms": 15000
}
preconditions:  "schein_ident must come from kvScheine[].ident in patientenDetailsRelationen response."
error_handling: "HTTP 404: schein_ident not found. Do NOT try /leistung?patient=X — that crashes the server."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.20 — ToolSkill: `ts-tomedo-ebmkatalogeintrag-get` (class 13)

```
name:          "ts-tomedo-ebmkatalogeintrag-get"
tool_name:     "tomedo-api"
description:   "GET /{db}/ebmkatalogeintrag/{ident} — resolve an internal EBM catalog
                ident to its human-readable EBM Ziffer string (e.g. 270 → '03220').
                Use after fetching ebmLeistungen from /kvschein to map internal ints
                to billing code strings.
                Response: {ident:N, nummer:'03220', bezeichnung:'Versichertenpauschale', ...}"
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/ebmkatalogeintrag/{ebm_catalog_ident}"},
  {name: "method",     param_type: "string", required: true, description: "GET"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url":        "{{vars.tomedo_base_url}}/ebmkatalogeintrag/{{vars.ebm_catalog_ident}}",
  "method":     "GET",
  "timeout_ms": 10000
}
preconditions:  "ebm_catalog_ident must be from ebmLeistungen[].ebmKatalogEintrag.ident."
error_handling: "HTTP 404: ident not in catalog. Surface raw ident to user if lookup fails."
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

⚠️ **CRAWL-PIPELINE USE ONLY** — Only call this from a crawl-delta recipe step.
Never use in interactive recipes. Always chain with `pc-tomedo-filter-recent-patients`
immediately after — never pass the raw body to context or the LLM.

```python
# Channel: orchestrator | Class: 22
# ⚠️ CRAWL-PIPELINE USE ONLY — ~15k records, 8-20 MB response.
# Dispatches ts-tomedo-patient-list.
# MUST be immediately followed by pc-tomedo-filter-recent-patients.
# Never pass result directly to LLM or recipe context.
import json as _j
_base = "{{vars.tomedo_base_url}}"
if not _base:
    result = {"error": "tomedo_base_url not configured"}
else:
    result = __execute_action__("tomedo-api", {
        "url": f"{_base}/patient?flach=true",
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
# ⚠️ Uses besuch_ident (NOT patient_id) — obtain from patientenDetailsRelationen.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/besuch/{{vars.besuch_ident}}/besucheForPatient",
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
body = _j.dumps({"call_id": int("{{vars.call_id}}"), "phone_number": "{{vars.phone_number}}"})
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

---

### Step 3.22 — PythonCode: `pc-tomedo-llm-arztbericht` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Submits a raw Arztbericht (medical report) to the tomedo LLM service
# and returns a structured extraction.
# Slots: {{vars.tomedo_llm_endpoint}}, {{vars.tomedo_user_ident}}, {{vars.bericht_text}}
import json as _j
body = _j.dumps({
    "model": "gemini-2.5-flash",
    "messages": [
        {"role": "system", "content": (
            "Du bist ein medizinischer Dokumentationsassistent. Extrahiere aus dem folgenden "
            "Arztbericht die wichtigsten Befunde, Diagnosen und Empfehlungen in strukturierter Form. "
            "Antworte auf Deutsch."
        )},
        {"role": "user", "content": "{{vars.bericht_text}}"}
    ],
    "stream": False
})
result = __execute_action__("tomedo-llm-api", {
    "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": body,
    "timeout_ms": 60000
})
```

---

### Step 3.23 — PythonCode: `pc-tomedo-llm-ct-befund` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Extracts structured findings from a CT/MRI radiology report text.
# Slots: {{vars.tomedo_llm_endpoint}}, {{vars.tomedo_user_ident}}, {{vars.befund_text}}
import json as _j
body = _j.dumps({
    "model": "gemini-2.5-flash",
    "messages": [
        {"role": "system", "content": (
            "Du bist ein Radiologe-Assistent. Extrahiere aus dem folgenden CT/MRT-Befund: "
            "1) Untersuchte Organe/Regionen, 2) Pathologische Befunde mit Lokalisation und Größe, "
            "3) Beurteilung/Diagnose, 4) Empfehlungen. Antworte strukturiert auf Deutsch."
        )},
        {"role": "user", "content": "{{vars.befund_text}}"}
    ],
    "stream": False
})
result = __execute_action__("tomedo-llm-api", {
    "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": body,
    "timeout_ms": 60000
})
```

---

### Step 3.24 — PythonCode: `pc-tomedo-llm-schlaflabor` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Extracts structured results from a sleep lab (Schlaflabor) report.
# Slots: {{vars.tomedo_llm_endpoint}}, {{vars.tomedo_user_ident}}, {{vars.bericht_text}}
import json as _j
body = _j.dumps({
    "model": "gemini-2.5-flash",
    "messages": [
        {"role": "system", "content": (
            "Du bist ein Schlafmedizin-Assistent. Extrahiere aus dem folgenden Schlaflaborbericht: "
            "AHI/RDI, Sauerstoffsättigung (min/mean), Schnarchindex, CPAP-Druck, Maskentyp, "
            "ESS-Score, Diagnose (z.B. OSAS Grad), Therapieempfehlung. "
            "Antworte strukturiert auf Deutsch."
        )},
        {"role": "user", "content": "{{vars.bericht_text}}"}
    ],
    "stream": False
})
result = __execute_action__("tomedo-llm-api", {
    "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": body,
    "timeout_ms": 60000
})
```

---

### Step 3.25 — PythonCode: `pc-tomedo-llm-laborbefund` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Interprets a lab report text and flags values outside the reference range.
# Slots: {{vars.tomedo_llm_endpoint}}, {{vars.tomedo_user_ident}}, {{vars.labor_text}}
import json as _j
body = _j.dumps({
    "model": "gemini-2.5-flash",
    "messages": [
        {"role": "system", "content": (
            "Du bist ein Labormedizin-Assistent. Extrahiere aus dem folgenden Laborbefund "
            "alle Messwerte mit Einheit und Referenzbereich. Markiere Werte außerhalb des "
            "Referenzbereichs deutlich. Gib eine klinische Einschätzung der auffälligen Befunde. "
            "Antworte strukturiert auf Deutsch."
        )},
        {"role": "user", "content": "{{vars.labor_text}}"}
    ],
    "stream": False
})
result = __execute_action__("tomedo-llm-api", {
    "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": body,
    "timeout_ms": 60000
})
```

---

### Step 3.26 — PythonCode: `pc-tomedo-llm-gutachten` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Drafts a medical expert opinion (Gutachten) request or pre-fills a Gutachtenauftrag.
# Slots: {{vars.tomedo_llm_endpoint}}, {{vars.tomedo_user_ident}},
#        {{vars.patient_context}}, {{vars.gutachten_anfrage}}
import json as _j
body = _j.dumps({
    "model": "gemini-2.5-pro",
    "messages": [
        {"role": "system", "content": (
            "Du bist ein erfahrener Facharzt. Erstelle auf Basis des Patientenkontextes einen "
            "professionellen Gutachtenauftrag oder eine ärztliche Stellungnahme. "
            "Verwende formale medizinische Sprache. Antworte auf Deutsch."
        )},
        {"role": "user", "content": (
            "Patientenkontext:\n{{vars.patient_context}}\n\n"
            "Anfrage/Fragestellung:\n{{vars.gutachten_anfrage}}"
        )}
    ],
    "stream": False
})
result = __execute_action__("tomedo-llm-api", {
    "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": body,
    "timeout_ms": 60000
})
```

---

### Step 3.27 — PythonCode: `pc-tomedo-llm-patientenbrief` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Drafts a patient letter (Patientenbrief) from a medical context summary.
# Slots: {{vars.tomedo_llm_endpoint}}, {{vars.tomedo_user_ident}},
#        {{vars.patient_kontext}}, {{vars.brief_anlass}}
import json as _j
body = _j.dumps({
    "model": "gemini-2.5-flash",
    "messages": [
        {"role": "system", "content": (
            "Du bist ein Arzt und schreibst einen verständlichen, freundlichen Brief an den Patienten. "
            "Verwende klare, patientenverständliche Sprache ohne übermäßigen Fachjargon. "
            "Antworte auf Deutsch."
        )},
        {"role": "user", "content": (
            "Patienteninfo:\n{{vars.patient_kontext}}\n\n"
            "Anlass des Briefes:\n{{vars.brief_anlass}}"
        )}
    ],
    "stream": False
})
result = __execute_action__("tomedo-llm-api", {
    "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": body,
    "timeout_ms": 60000
})
```

---

### Step 3.28 — PythonCode: `pc-tomedo-llm-uebersetzung` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Translates a medical text to/from German using the tomedo LLM service.
# Slots: {{vars.tomedo_llm_endpoint}}, {{vars.tomedo_user_ident}},
#        {{vars.quelltext}}, {{vars.zielsprache}}
import json as _j
body = _j.dumps({
    "model": "gemini-2.5-flash",
    "messages": [
        {"role": "system", "content": (
            "Du bist ein medizinischer Übersetzer. Übersetze den folgenden medizinischen Text "
            "nach {{vars.zielsprache}}. Bewahre die medizinische Fachterminologie und Präzision. "
            "Gib nur die Übersetzung zurück, ohne Kommentar."
        )},
        {"role": "user", "content": "{{vars.quelltext}}"}
    ],
    "stream": False
})
result = __execute_action__("tomedo-llm-api", {
    "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": body,
    "timeout_ms": 60000
})
```

---

### Step 3.29 — PythonCode: `pc-tomedo-llm-bga` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Interprets a blood gas analysis (Blutgasanalyse/BGA) and provides clinical context.
# Slots: {{vars.tomedo_llm_endpoint}}, {{vars.tomedo_user_ident}}, {{vars.bga_werte}}
import json as _j
body = _j.dumps({
    "model": "gemini-2.5-flash",
    "messages": [
        {"role": "system", "content": (
            "Du bist ein Intensivmedizin-Assistent. Analysiere die folgende Blutgasanalyse (BGA). "
            "Beurteile: pH, paCO2, paO2, HCO3, BE, SpO2, Laktat. "
            "Klassifiziere: Azidose/Alkalose, respiratorisch/metabolisch/gemischt, Kompensation. "
            "Gib eine klinische Handlungsempfehlung. Antworte strukturiert auf Deutsch."
        )},
        {"role": "user", "content": "{{vars.bga_werte}}"}
    ],
    "stream": False
})
result = __execute_action__("tomedo-llm-api", {
    "url": "{{vars.tomedo_llm_endpoint}}/llmservice/{{vars.tomedo_user_ident}}/v1/chat/completions",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": body,
    "timeout_ms": 60000
})
```

---

### Step 3.30 — PythonCode: `pc-tomedo-llm-extract-response` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts the text content from a tomedo LLM service response.
# {{vars.llm_response}} is the raw JSON response from the LLM service.
import json as _j
try:
    data = _j.loads("{{vars.llm_response}}")
    choices = data.get("choices", [])
    if choices:
        result = choices[0].get("message", {}).get("content", "")
    else:
        # Budget exhausted or model error — surface the raw body
        result = "Fehler: " + data.get("error", {}).get("message", str(data))
except Exception as e:
    result = "Fehler beim Parsen der LLM-Antwort: " + str(e)
```


---

### Step 3.31 — PythonCode: `pc-tomedo-karteieintrag-create` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1 (LLM confirms content before call)
# Creates a KarteiEintrag for patient_id with given text, datum, and kuerzel.
# IBS bakes in vars before execution.
import json as _j
_base = "{{vars.tomedo_base_url}}"
if not _base:
    result = {"error": "tomedo_base_url not configured"}
else:
    _body = _j.dumps({
        "patient": {"ident": int("{{vars.patient_id}}")},
        "text": "{{vars.text}}",
        "datum": int("{{vars.datum}}"),
        "visible": True,
        "karteiEintragTyp": {"kuerzel": "{{vars.kuerzel}}"}
    })
    result = __execute_action__("tomedo-api", {
        "url": f"{_base}/karteieintrag",
        "method": "POST",
        "headers": {"Content-Type": "application/json"},
        "body": _body,
        "timeout_ms": 15000
    })
```

---

### Step 3.32 — PythonCode: `pc-tomedo-karteieintrag-update` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1 (LLM or orchestrator confirms before update)
# Partial-updates an existing KarteiEintrag. Most common: visible:false to soft-delete.
# vars.body_json must be a valid JSON partial-update string, e.g. '{"visible":false}'.
import json as _j
_base = "{{vars.tomedo_base_url}}"
_ident = "{{vars.karteieintrag_ident}}"
if not _base or not _ident:
    result = {"error": "tomedo_base_url or karteieintrag_ident not configured"}
else:
    result = __execute_action__("tomedo-api", {
        "url": f"{_base}/karteieintrag/{_ident}",
        "method": "PUT",
        "headers": {"Content-Type": "application/json"},
        "body": "{{vars.body_json}}",
        "timeout_ms": 15000
    })
```

---

### Step 3.33 — PythonCode: `pc-tomedo-termin-create` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1 (LLM confirms content before call)
# Creates a Termin for patient_id. beginn/ende are epoch ms.
import json as _j
_base = "{{vars.tomedo_base_url}}"
if not _base:
    result = {"error": "tomedo_base_url not configured"}
else:
    _body = _j.dumps({
        "patient": {"ident": int("{{vars.patient_id}}")},
        "beginn": int("{{vars.beginn}}"),
        "ende": int("{{vars.ende}}"),
        "info": "{{vars.info}}",
        "removed": False,
        "warDa": False
    })
    result = __execute_action__("tomedo-api", {
        "url": f"{_base}/termin",
        "method": "POST",
        "headers": {"Content-Type": "application/json"},
        "body": _body,
        "timeout_ms": 15000
    })
```

---

### Step 3.34 — PythonCode: `pc-tomedo-kvschein-get` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 0
# Fetches a KV-Schein by ident including ebmLeistungen[] and goaeLeistungen[].
# schein_ident must come from patientenDetailsRelationen kvScheine[].ident.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/kvschein/{{vars.schein_ident}}",
    "method": "GET",
    "timeout_ms": 15000
})
```

---

### Step 3.35 — PythonCode: `pc-tomedo-ebmkatalogeintrag-get` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 0
# Resolves an EBM catalog ident to its human-readable Ziffer string.
# ebm_catalog_ident comes from ebmLeistungen[].ebmKatalogEintrag.ident.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/ebmkatalogeintrag/{{vars.ebm_catalog_ident}}",
    "method": "GET",
    "timeout_ms": 10000
})
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

⚠️ **CRAWL-PIPELINE USE ONLY** — This skill MUST NOT be selected for any
interactive patient-lookup query. The orchestrator must refuse to use it for
any user-facing request and must redirect to `skill-tomedo-patient-search-by-name`
(name lookup) or `skill-tomedo-crawl-phone-lookup` (phone lookup) instead.

```
name:        "skill-tomedo-patient-list"
class_code:  1
description: "Leaf skill: fetch the complete flat patient list from tomedo — CRAWL-PIPELINE USE ONLY."
body: |
  ⚠️ CRAWL-PIPELINE USE ONLY.
  This skill fetches ALL ~15 000 patients in one HTTP call (~8–20 MB response).
  It MUST NOT be used for interactive patient lookup, name search, or phone lookup.
  Calling it naively will exhaust server memory and crash the process.

  WHEN TO USE: Only when performing a crawl delta-sync (e.g. finding patients
  modified since last_crawl to update the vector index). Always chain the result
  immediately to pc-tomedo-filter-recent-patients to reduce the list before
  any further processing.

  FOR PATIENT SEARCH BY NAME → use skill-tomedo-patient-search-by-name
  FOR PATIENT SEARCH BY PHONE → use skill-tomedo-crawl-phone-lookup
  FOR RECENT PATIENTS → fetch list + pc-tomedo-filter-recent-patients (limit 20)

  PythonCode:
    Use pc-tomedo-patient-list, then immediately pipe through
    pc-tomedo-filter-recent-patients with limit={{vars.limit|20}}.
    Never pass the raw body anywhere else.
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
  Three integration surfaces:
    1. Direct tomedo REST API (mTLS HTTPS, port 8443) — patient data reads.
    2. tomedo-crawl sidecar (loopback HTTP, port 13181) — phone lookup, RAG,
       crawl control. Always check /health before querying the sidecar.
    3. tomedo LLM service (mTLS HTTPS or LAN HTTP) — DSGVO-compliant LLM
       inference via the tomedo server's built-in Kartei-Chat backend.

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

  tomedo LLM SERVICE (DSGVO-compliant, via tomedo server):
    • Arztbericht extraction:   skill-tomedo-llm-arztbericht (Tier 1)
    • CT/MRI report extraction: skill-tomedo-llm-ct-befund (Tier 1)
    • Sleep lab extraction:     skill-tomedo-llm-schlaflabor (Tier 1)
    • Lab report interpretation:skill-tomedo-llm-laborbefund (Tier 1)
    • Expert opinion draft:     skill-tomedo-llm-gutachten (Tier 1)
    • Patient letter draft:     skill-tomedo-llm-patientenbrief (Tier 1)
    • Medical translation:      skill-tomedo-llm-uebersetzung (Tier 1)
    • BGA interpretation:       skill-tomedo-llm-bga (Tier 1)
    CONFIG REQUIRED: tomedo_user_ident, tomedo_llm_endpoint
    Models: gemini-2.5-flash (default), gemini-2.5-pro, mistral-medium-2508
    Budget: monthly per-user limit from zollsoft — surface budget errors to user.

  AUTH REQUIREMENT:
  All direct tomedo API calls require the mTLS client certificate PEM file.
  Check tomedo_cert_pem config before any direct API call.
  LLM service (LAN HTTP variant) requires tomedo_user_ident only.

  TIER-0 ELIGIBILITY:
  All read operations with a known patient_id are Tier 0.
  Direct mTLS REST API (port 8443) supports GET + POST + PUT (confirmed 2026-08-22).
  Writes (POST/PUT) are Tier 1 — LLM confirms content before dispatch.
  Name search is Tier 1 (LLM composes the query from user intent).
  LLM object composition (Python markers, stats, letters, CKEs, forms) is Tier 1.
  LLM service calls (text analysis, translation, BGA) are Tier 1.

  LLM OBJECT COMPOSITION (§llm-objects):
  Use skill-tomedo-compose-python-marker, skill-tomedo-compose-statistic,
  skill-tomedo-compose-briefvorlage, skill-tomedo-compose-cke,
  skill-tomedo-compose-patientenformular for tomedo object generation.
  These are Tier 1 — LLM composes the output using embedded context files.
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


---

## Step 4a — Leaf Skills for Write Operations and Leistungen (class 1)

One leaf skill per distinct write/Leistung operation.
Write skills are Tier 1 — the LLM confirms content before the orchestrator dispatches.

---

### Step 4a.1 — Leaf Skill: `skill-tomedo-karteieintrag-create` (class 1)

```
name:        "skill-tomedo-karteieintrag-create"
class_code:  1
description: "Leaf skill: create a new KarteiEintrag (medical record entry) for a patient via POST — Tier 1."
body: |
  Create a KarteiEintrag for patient {{vars.patient_id}} using ts-tomedo-karteieintrag-create.
  URL: POST {{vars.tomedo_base_url}}/karteieintrag
  Required fields: patient.ident, text, datum (epoch ms), visible:true, karteiEintragTyp.kuerzel.

  Common kuerzel values:
    BRIEF   — Arztbrief / patient letter
    AKTE    — general Karteieintrag
    TEL     — telephone note
    LAB     — lab result note

  Before dispatching: show the user the text and kuerzel and ask for confirmation.
  After creation: the response contains {ident: N} — surface the new ident to the user.
  To undo: call skill-tomedo-karteieintrag-update with {visible: false}.

  PythonCode: use pc-tomedo-karteieintrag-create.
  datum: use current Unix epoch ms if not specified.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4a.2 — Leaf Skill: `skill-tomedo-karteieintrag-update` (class 1)

```
name:        "skill-tomedo-karteieintrag-update"
class_code:  1
description: "Leaf skill: update (or soft-delete) an existing KarteiEintrag via PUT — Tier 1."
body: |
  Update an existing KarteiEintrag by ident using ts-tomedo-karteieintrag-update.
  URL: PUT {{vars.tomedo_base_url}}/karteieintrag/{karteieintrag_ident}

  Common operations:
    Soft-delete:   {visible: false}
    Update text:   {text: "new text"}
    Change type:   {karteiEintragTyp: {kuerzel: "BRIEF"}}

  Before dispatching: confirm the change with the user.
  A successful update returns HTTP 204 (no body).
  DELETE is NOT supported — always use PUT {visible:false} to hide entries.

  PythonCode: use pc-tomedo-karteieintrag-update.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4a.3 — Leaf Skill: `skill-tomedo-termin-create` (class 1)

```
name:        "skill-tomedo-termin-create"
class_code:  1
description: "Leaf skill: create a new appointment (Termin) for a patient via POST — Tier 1."
body: |
  Create a Termin for patient {{vars.patient_id}} using ts-tomedo-termin-create.
  URL: POST {{vars.tomedo_base_url}}/termin
  Required fields: patient.ident, beginn (epoch ms), ende (epoch ms), info (string).

  Before dispatching: show the user the date/time and reason and ask for confirmation.
  After creation: the response contains {ident: N} — surface the new ident.
  To cancel: use ts-tomedo-termin-create with method PUT /termin/{ident} body {removed: true}.

  Date/time: convert human-readable time to epoch ms before passing.
  PythonCode: use pc-tomedo-termin-create.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4a.4 — Leaf Skill: `skill-tomedo-leistungen-read` (class 1)

```
name:        "skill-tomedo-leistungen-read"
class_code:  1
description: "Leaf skill: read Leistungen (billing codes) for a patient via safe two-step path — Tier 0."
body: |
  Read Leistungen (EBM/GOÄ billing codes) for patient {{vars.patient_id}}.

  ⚠️ NEVER call /leistung?patient=X, /patient/{id}/leistungen, or /schein?patient=X —
  all of these crash the tomedo server (unbounded queries, confirmed 2026-08-22).

  SAFE READ PATH (two steps):
  1. Use pc-tomedo-patient-relations with limitScheine=true to get kvScheine[].ident
  2. For each schein ident of interest: use pc-tomedo-kvschein-get to get
     ebmLeistungen[] and goaeLeistungen[]

  EBMLeistung fields:
    ident           — internal Leistung ID
    datum           — epoch ms
    anzahl          — count (default 1)
    ebmKatalogEintrag.ident — internal catalog int (NOT the EBM Ziffer string)
    leistungserbringer.ident — Arzt ident
    visible         — false if soft-deleted

  To resolve catalog ident → EBM Ziffer string (e.g. 270 → '03220'):
    Use pc-tomedo-ebmkatalogeintrag-get.

  The Briefkommando $[l %nr 0d ,]$ reads Leistungen from the currently-open
  Schein in the Mac client — it is NOT a REST path. Use this REST path instead
  when BrassClaw needs Leistung data server-side.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---


## Step 4b — Leaf Skills for LLM Object Composition (class 1)

One leaf skill per object type. Each embeds the full zollsoft context file text
in the skill body so the orchestrator can inject it into the LLM prompt.
All are Tier 1 — the LLM must compose the output.

---

### Step 4b.1 — Leaf Skill: `skill-tomedo-compose-python-marker` (class 1)

```
name:        "skill-tomedo-compose-python-marker"
class_code:  1
description: "Leaf skill: compose a tomedo automatic Python marker using the zollsoft context file."
body: |
  Use this skill when the user wants to create an automatic Python marker for tomedo.
  Python markers run automatically and can flag patients in the patient list or record.

  INTEGRATION POINT: tomedo > Einstellungen > Praxisorganisation > Automatische Marker
  OUTPUT FORMAT: Python code block
  HOW TO USE: Copy the generated Python code and paste it into a new/existing marker.

  CONTEXT FILE (pythonmarker_context.txt — embed full content from zollsoft download):
  The context teaches the LLM the tomedo Python marker API:
  - patient object structure (name, birthdate, diagnoses, medications, appointments)
  - Available Python functions and variables inside a marker script
  - Return convention: set `result` to the marker text (empty string = no marker)
  - Access to today's date, patient birthdate, last appointment, etc.

  PROMPT PATTERN:
  [Full context file content]
  ---
  Erstelle einen Python-Marker, der [user requirement in German].

  COMMON USE CASES (from forum users):
  - Birthday reminder: marker set if birthday within next N days
  - Lab value alert: marker if last lab value outside reference range
  - Diagnosis flag: marker if specific ICD code in diagnoses list
  - Appointment reminder: marker if no appointment in next 90 days
  - Medication warning: marker if specific medication combination present

  After the LLM generates the code, return it as a formatted code block
  with instructions: "Kopieren Sie diesen Code in tomedo unter
  Einstellungen > Praxisorganisation > Automatische Marker."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4b.2 — Leaf Skill: `skill-tomedo-compose-statistic` (class 1)

```
name:        "skill-tomedo-compose-statistic"
class_code:  1
description: "Leaf skill: compose a tomedo statistics SQL query using the zollsoft context file."
body: |
  Use this skill when the user wants to create a custom SQL statistics query for tomedo.
  Statistics queries run against the tomedo PostgreSQL database.

  INTEGRATION POINT: tomedo > Statistiken > Neue Statistik > SQL-Abfrage
  OUTPUT FORMAT: SQL query
  HOW TO USE: Paste the generated SQL into the SQL field of a new statistics entry.

  CONTEXT FILE (statistikKompakt.txt / statistik_context.txt — zollsoft download):
  The context teaches the LLM:
  - Available tomedo database table names and key columns
  - How to join patient, schein, leistung, diagnose tables
  - Date/period filtering conventions (e.g., current quarter, last year)
  - GOÄ/EBM ziffer access and filtering

  PROMPT PATTERN:
  [Full context file content]
  ---
  Erstelle eine SQL-Statistik, die [user requirement in German].

  COMMON USE CASES (from forum users):
  - GOÄ/EBM billing analysis for a time period
  - Patient list filtered by diagnosis (ICD code)
  - Medication frequency analysis
  - Appointment volume per doctor per quarter
  - HZV participation status overview

  After the LLM generates the SQL, return it as a formatted code block
  with instructions: "Fügen Sie diese SQL-Abfrage in tomedo unter
  Statistiken > Neue Statistik > SQL ein."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4b.3 — Leaf Skill: `skill-tomedo-compose-briefvorlage` (class 1)

```
name:        "skill-tomedo-compose-briefvorlage"
class_code:  1
description: "Leaf skill: compose a tomedo letter template (Briefvorlage) using the zollsoft context file."
body: |
  Use this skill when the user wants to create a professional letter template for tomedo.
  Letter templates use HTML with tomedo Briefkommando placeholders.

  INTEGRATION POINT: tomedo > Briefschreibung > Briefvorlagen
  OUTPUT FORMAT: HTML with Briefkommando placeholders ($[...]$)
  HOW TO USE: Create a new Briefvorlage and paste the generated HTML.

  CONTEXT FILE (briefvorlage_context.txt / Briefvorlagen.txt — zollsoft download):
  The context teaches the LLM:
  - Supported HTML tags and styling
  - Briefkommando placeholder syntax and common shorthands (see §briefkommandos)
  - Page layout, header/footer, logo positioning
  - Table formatting for lab values, medications, billing items

  BRIEFKOMMANDO LOOKUP (orchestrator-first, before asking the LLM):
  For standard patient fields, use the embedded mapping from skill-tomedo-lookup-briefkommando:
  - Patient name: $[pvoll]$, $[pn]$, $[pv]$
  - Address block: $[adressfeld_empfaenger]$
  - Salutation: $[anrede]$ or $[a Sehr_geehrte_Frau_%pn Sehr_geehrter_Herr_%pn ...]$
  - Date: $[d S dd.MM.yyyy]$ (today), $[d B]$ (last visit), $[pg]$ (DOB)
  - Phone: $[ptel]$, $[phandy]$
  - Diagnosis (from Kartei): $[&p.patientenDetails.patientenDetailsRelationen.diagnosen]$
  - Any REST field: $[&p.{json_field_path}]$
  For unknown or complex fields, call skill-tomedo-lookup-briefkommando first.

  PROMPT PATTERN:
  [Full context file content]
  ---
  Erstelle eine Briefvorlage für [user requirement in German].
  Verwende folgende Briefkommandos für Patientenfelder: [list from lookup above]

  COMMON USE CASES (from forum users):
  - Medical report (Befundbericht) with structured sections
  - Patient letter with address block and greeting
  - Referral letter (Überweisungsschreiben) with diagnosis and medication
  - CT/MRI report with formatted sections (from Dr. Baumann's prompts)
  - Sleep lab report extraction (Schlaflaborbericht-Auswertung)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4b.4 — Leaf Skill: `skill-tomedo-lookup-briefkommando` (class 1)

```
name:        "skill-tomedo-lookup-briefkommando"
class_code:  1
description: "Leaf skill: look up the correct Briefkommando placeholder for a given patient/practice data field. Orchestrator-first: check the embedded table before calling the LLM."
body: |
  Use this skill when the user needs to find the exact Briefkommando placeholder
  for a specific data field to use in a letter template.
  This is a LOOKUP skill — it does NOT generate a complete template.

  ORCHESTRATOR-FIRST APPROACH (Tier 0 for known fields):
  Before calling the LLM, check this embedded lookup table. If the field is
  listed, return the answer directly — no LLM call needed.

  EMBEDDED FIELD → BRIEFKOMMANDO TABLE (from §briefkommandos, confirmed live Aug 2026):
  | Field                  | Shorthand           | Keypath form                                          |
  |------------------------|---------------------|-------------------------------------------------------|
  | Patient ID             | $[pid]$             | $[&p.ident]$                                          |
  | Family name            | $[pn]$              | $[&p.nachname]$                                       |
  | Given name             | $[pv]$              | $[&p.vorname]$                                        |
  | Title                  | $[pt]$              | $[&p.titel]$                                          |
  | Full name              | $[pvoll]$           | —                                                     |
  | Date of birth          | $[pg]$              | —                                                     |
  | Street                 | $[ps]$              | —                                                     |
  | Postcode               | $[pp]$              | —                                                     |
  | City/town              | $[po]$              | —                                                     |
  | Country                | $[pLand]$           | $[&p.patientenDetails.kontaktdaten.adresse.land]$      |
  | Mobile phone           | $[phandy]$          | —                                                     |
  | Main phone             | $[ptel]$            | —                                                     |
  | E-Mail                 | $[pemail]$          | —                                                     |
  | Gender                 | $[pmw]$             | —                                                     |
  | Insurance type         | $[patient_versichertenstatus]$ | —                                          |
  | Insurance name         | $[pk]$              | —                                                     |
  | Insurance IK           | $[kasse_ik]$        | —                                                     |
  | Address block          | $[adressfeld_empfaenger]$ | —                                               |
  | Salutation             | $[anrede]$          | —                                                     |
  | Today's date           | $[datum]$ / $[d S]$ | —                                                     |
  | Last visit date        | $[d B]$             | —                                                     |
  | Body height (BMI)      | $[koerpergroesseAusLetztemBMI]$ | —                                          |
  | Body weight (BMI)      | $[gewichtAusLetztemBMI]$ | —                                                |
  | Referring physician    | —                   | $[&p.selektierterSchein.ueberweisenderArztName]$       |
  | First treating physician | —                 | $[&p.patientenDetails.arzt]$                          |
  | Invoice paid status    | —                   | $[&pr.bezahlt]$                                       |

  KEYPATH RULE: If the field is not in the table but is known from the REST API
  response (GET /patient/{id}), derive the Briefkommando as:
    $[&p.{json_field_path}]$
  e.g. if the REST field is patientenDetails.kontaktdaten.telefon2:
    → $[&p.patientenDetails.kontaktdaten.telefon2]$

  DATE KOMMANDO RULE:
  - Current date: $[d S dd.MM.yyyy]$
  - Patient DOB: $[pg]$ (short) or $[pg2]$ (with 4-digit year)
  - Last visit: $[d B]$
  - Last Karteieintrag of type X: $[d E dd.MM.yyyy X]$
  - Last billing code Y: $[d L dd.MM.yyyy Y]$

  CONDITIONAL RULE:
  - Gender branch: $[if_frau TextFrau TextMann]$
  - Empty check: $[if ptel zs_equals <leer> kein_Telefon ptel]$
  - if_then (only show if not empty): $[if_then pemail zs_not_equal <leer> pemail]$

  LLM FALLBACK (only if field not in table and keypath not derivable):
  CONTEXT FILE (briefkommandos_context.txt / BriefkommandosKompakt.txt — zollsoft):
  Acts as a knowledge base for unusual/compound Briefkommandos.
  PROMPT PATTERN:
  [Full context file content]
  ---
  Welches Briefkommando gibt [user's field description in German] aus?

  Return the answer as: "Briefkommando: $[...]$" with a one-line description.
  If multiple forms exist (shorthand + keypath), list both.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4b.5 — Leaf Skill: `skill-tomedo-compose-cke` (class 1)

```
name:        "skill-tomedo-compose-cke"
class_code:  1
description: "Leaf skill: compose a tomedo CustomKarteiEintrag (CKE) XML definition using the zollsoft context file."
body: |
  Use this skill when the user wants to create a structured data entry form
  (CustomKarteiEintrag) for the tomedo patient record (Kartei).
  CKEs standardise documentation of recurring events (vaccinations, tumour data, etc.).

  INTEGRATION POINT: tomedo > Kartei > CustomKarteiEintrag editor
  OUTPUT FORMAT: XML definition
  HOW TO USE: Import the XML into a new CKE type in tomedo.

  CONTEXT FILE (customkckeKonpakt.md / customkarteieintrag_context.txt — zollsoft download):
  The context teaches the LLM:
  - XML element structure for CKE fields
  - Supported field types: text, dropdown, date, number, boolean, checkbox
  - Conditional field visibility
  - Field labels (German/English)

  PROMPT PATTERN:
  [Full context file content]
  ---
  Erstelle einen CustomKarteiEintrag für [user requirement in German].

  COMMON USE CASES (from forum users — Dr. Baumann, Dr. Wanzar, Christoph Baumbach):
  - Vaccination documentation (vaccine name dropdown, date, batch number)
  - Tumour documentation: TNM formula, histology, therapy, staging
  - ILD/COPD/asthma structured follow-up form
  - Sleep lab result entry (ESS, RDI, pressure, mask, CPAP settings)
  - Allergy documentation with prick test parameters and sensitisation

  After the LLM generates the XML, return it as a formatted code block
  with instructions for how to import it into tomedo.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4b.6 — Leaf Skill: `skill-tomedo-compose-patientenformular` (class 1)

```
name:        "skill-tomedo-compose-patientenformular"
class_code:  1
description: "Leaf skill: compose a tomedo patient form (Patientenformular) JSON definition using the zollsoft context file."
body: |
  Use this skill when the user wants to create a digital patient questionnaire/form
  for tomedo. Patient forms use SurveyJS JSON format and can be filled on a tablet
  or via the patient portal.

  INTEGRATION POINT: tomedo > Patientenformulare > JSON tab
  OUTPUT FORMAT: JSON (SurveyJS schema)
  HOW TO USE: Create a new Patientenformular, switch to the JSON tab, paste the JSON.

  CONTEXT FILE (patientenformular_context.txt / Patientenformulare.txt — zollsoft download):
  The context teaches the LLM:
  - SurveyJS JSON structure (title, pages, elements)
  - Supported element types: text, boolean, radiogroup, dropdown, date, signature, HTML
  - Conditional visibility with visibleIf
  - Multi-language support (de/en)
  - Required field marking

  NOTE: The LLM output may need minor corrections for complex logic (scores,
  conditional groups, pre-filled fields). Review before production use.

  PROMPT PATTERN:
  [Full context file content]
  ---
  Erstelle ein Patientenformular für [user requirement in German].

  COMMON USE CASES (from forum users):
  - Intake anamnesis for respiratory diseases
  - COVID/infection triage questionnaire (children/adults)
  - Consent form for procedures
  - Pre-appointment questionnaire (medication history, allergies)
  - Allergy anamnesis with detailed symptom tracking

  After the LLM generates the JSON, return it with instructions:
  "Erstellen Sie ein neues Patientenformular in tomedo und fügen Sie
  diesen JSON-Code in den JSON-Tab ein."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---


## Step 4c — Leaf Skills for tomedo LLM Service (class 1)

One leaf skill per medical text analysis / generation task type.
Each maps directly to one PythonCode executor. All are Tier 1.
These skills enable BrassClaw to use the tomedo server's own DSGVO-compliant
LLM service for clinical text work without external API keys.

---

### Step 4c.1 — Leaf Skill: `skill-tomedo-llm-arztbericht` (class 1)

```
name:        "skill-tomedo-llm-arztbericht"
class_code:  1
description: "Leaf skill: extract structured findings from a medical report (Arztbericht) using the tomedo LLM service."
body: |
  Use this skill when the user wants to extract the key findings, diagnoses
  and recommendations from a free-text Arztbericht.

  USE CASE: A doctor receives a referral letter, discharge summary, or specialist
  report and wants it extracted into structured bullet points.

  APPROACH:
  1. Pre-load ts-tomedo-llm-chat (channel: rust)
  2. Execute pc-tomedo-llm-arztbericht (channel: orchestrator)
     Slot {{vars.bericht_text}}: the raw report text
  3. Execute pc-tomedo-llm-extract-response to extract choices[0].message.content
  4. Present the structured result to the user

  MODEL: gemini-2.5-flash (default) — switch to gemini-2.5-pro for complex reports
  DSGVO: All processing on zollsoft zero-retention infrastructure. No text leaves EU.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4c.2 — Leaf Skill: `skill-tomedo-llm-ct-befund` (class 1)

```
name:        "skill-tomedo-llm-ct-befund"
class_code:  1
description: "Leaf skill: extract structured findings from a CT or MRI radiology report using the tomedo LLM service."
body: |
  Use this skill when the user wants structured extraction from a CT or MRI
  radiology report (Befundbericht) — confirmed use case from Dr. Baumann's
  forum posts (Sep 2025, Thorax/Abdomen CT extraction).

  EXTRACTS FROM REPORT:
  - Examined organs/regions
  - Pathological findings with location and size measurements
  - Radiological diagnosis / Beurteilung
  - Follow-up recommendations

  APPROACH:
  1. Pre-load ts-tomedo-llm-chat (channel: rust)
  2. Execute pc-tomedo-llm-ct-befund (channel: orchestrator)
     Slot {{vars.befund_text}}: the raw radiology report text
  3. Execute pc-tomedo-llm-extract-response to get the content string
  4. Present structured extraction to the user

  MODEL: gemini-2.5-flash
  NOTE: For reports with complex measurements or rare findings, use gemini-2.5-pro.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4c.3 — Leaf Skill: `skill-tomedo-llm-schlaflabor` (class 1)

```
name:        "skill-tomedo-llm-schlaflabor"
class_code:  1
description: "Leaf skill: extract structured sleep lab results (AHI, CPAP, ESS, diagnosis) from a Schlaflaborbericht using the tomedo LLM service."
body: |
  Use this skill when the user wants structured extraction from a
  Schlaflaborbericht (polysomnography or polygraphy report).
  Confirmed use case: pulmonology practices extracting CPAP/OSAS data (Dr. Baumann).

  EXTRACTS FROM REPORT:
  - AHI / RDI value and classification
  - Minimum and mean SpO2
  - Snoring index (Schnarchindex)
  - CPAP pressure setting
  - Mask type
  - ESS score (Epworth Sleepiness Scale)
  - Primary diagnosis (e.g., OSAS Grad III)
  - Therapy recommendation

  APPROACH:
  1. Pre-load ts-tomedo-llm-chat (channel: rust)
  2. Execute pc-tomedo-llm-schlaflabor (channel: orchestrator)
     Slot {{vars.bericht_text}}: the raw sleep lab report text
  3. Execute pc-tomedo-llm-extract-response
  4. Present extracted values to user (ready for CKE entry)

  MODEL: gemini-2.5-flash
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4c.4 — Leaf Skill: `skill-tomedo-llm-laborbefund` (class 1)

```
name:        "skill-tomedo-llm-laborbefund"
class_code:  1
description: "Leaf skill: interpret a laboratory report — flag out-of-range values and provide clinical context — using the tomedo LLM service."
body: |
  Use this skill when the user wants to interpret a Laborbefund (lab result
  report) and identify values outside the reference range.

  OUTPUT INCLUDES:
  - Table: parameter, value, unit, reference range, status (normal/high/low)
  - Clinical assessment of abnormal values
  - Suggested follow-up actions

  APPROACH:
  1. Pre-load ts-tomedo-llm-chat (channel: rust)
  2. Execute pc-tomedo-llm-laborbefund (channel: orchestrator)
     Slot {{vars.labor_text}}: the raw laboratory report text
  3. Execute pc-tomedo-llm-extract-response
  4. Present the interpreted result to the user

  MODEL: gemini-2.5-flash
  NOTE: For complex panels (e.g., endocrinology, oncology markers), gemini-2.5-pro
  provides better reference range awareness.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4c.5 — Leaf Skill: `skill-tomedo-llm-gutachten` (class 1)

```
name:        "skill-tomedo-llm-gutachten"
class_code:  1
description: "Leaf skill: draft a medical expert opinion (Gutachtenauftrag / ärztliche Stellungnahme) using patient context and the tomedo LLM service."
body: |
  Use this skill when the user wants to draft a formal Gutachtenauftrag or
  ärztliche Stellungnahme (medical expert opinion / referral for assessment).

  INPUTS REQUIRED:
  - {{vars.patient_context}}: structured patient data (use pc-tomedo-format-patient-context
    to build this from tomedo data before calling this skill)
  - {{vars.gutachten_anfrage}}: the specific question or purpose of the Gutachten

  APPROACH:
  1. Fetch patient context first using tomedo-patient-full-context recipe
  2. Pre-load ts-tomedo-llm-chat (channel: rust)
  3. Execute pc-tomedo-llm-gutachten (channel: orchestrator)
  4. Execute pc-tomedo-llm-extract-response
  5. Present draft to user for review before use

  MODEL: gemini-2.5-pro (required — formal medical document, highest quality)
  NOTE: Always present as a DRAFT. The physician must review and sign.
  OUTPUT is NOT automatically written to tomedo — user copies the draft.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4c.6 — Leaf Skill: `skill-tomedo-llm-patientenbrief` (class 1)

```
name:        "skill-tomedo-llm-patientenbrief"
class_code:  1
description: "Leaf skill: draft a patient-facing letter (Patientenbrief) in plain language using patient context and the tomedo LLM service."
body: |
  Use this skill when the user wants to draft a letter TO the patient.
  The letter uses plain, patient-friendly German (no complex medical jargon).

  INPUTS REQUIRED:
  - {{vars.patient_kontext}}: patient name, diagnosis summary, relevant info
  - {{vars.brief_anlass}}: reason for the letter (e.g., "Terminbestätigung",
    "Laborbefund-Mitteilung", "Nachsorgeempfehlung nach Entlassung")

  APPROACH:
  1. Pre-load ts-tomedo-llm-chat (channel: rust)
  2. Execute pc-tomedo-llm-patientenbrief (channel: orchestrator)
  3. Execute pc-tomedo-llm-extract-response
  4. Present draft to user for review

  MODEL: gemini-2.5-flash
  NOTE: Draft only. User must review and optionally paste into a tomedo Briefvorlage.
  For letters requiring formal layout with Briefkommando placeholders, use
  skill-tomedo-compose-briefvorlage instead.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4c.7 — Leaf Skill: `skill-tomedo-llm-uebersetzung` (class 1)

```
name:        "skill-tomedo-llm-uebersetzung"
class_code:  1
description: "Leaf skill: translate a medical text to or from German using the tomedo LLM service (DSGVO-compliant, no external API)."
body: |
  Use this skill when the user wants to translate a medical text.
  Commonly: foreign-language patient records, international lab reports,
  English-language studies, or German discharge summaries for foreign patients.

  INPUTS REQUIRED:
  - {{vars.quelltext}}: the source text to translate
  - {{vars.zielsprache}}: target language (e.g., "Englisch", "Türkisch", "Arabisch",
    "Russisch", "Französisch", "Spanisch")

  APPROACH:
  1. Pre-load ts-tomedo-llm-chat (channel: rust)
  2. Execute pc-tomedo-llm-uebersetzung (channel: orchestrator)
  3. Execute pc-tomedo-llm-extract-response
  4. Present translation to user

  MODEL: gemini-2.5-flash (handles most European languages well)
  NOTE: For rare languages or complex medical abbreviations, gemini-2.5-pro
  is recommended. No patient data leaves EU — all via zollsoft infrastructure.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4c.8 — Leaf Skill: `skill-tomedo-llm-bga` (class 1)

```
name:        "skill-tomedo-llm-bga"
class_code:  1
description: "Leaf skill: interpret a blood gas analysis (Blutgasanalyse/BGA) using the tomedo LLM service."
body: |
  Use this skill when the user has BGA values and wants clinical interpretation.
  Confirmed use case: pneumology and intensive care practices (Dr. Baumann).

  BGA PARAMETERS INTERPRETED:
  - pH → acidosis / alkalosis classification
  - paCO2 → respiratory component
  - paO2 → oxygenation status
  - HCO3 / BE → metabolic component
  - SpO2 / FiO2 → if available
  - Lactate → perfusion / shock marker

  FULL CLASSIFICATION OUTPUT:
  - Primary disturbance (respiratory / metabolic / mixed)
  - Compensation status (compensated / partially / uncompensated)
  - Oxygenation assessment (Horovitz index if FiO2 given)
  - Clinical action recommendation (O2 therapy, ventilation adjustment, etc.)

  INPUT:
  - {{vars.bga_werte}}: BGA values as text (e.g. "pH 7.32, paCO2 52, paO2 68,
    HCO3 26, BE +2, SpO2 91%, Laktat 1.8")

  APPROACH:
  1. Pre-load ts-tomedo-llm-chat (channel: rust)
  2. Execute pc-tomedo-llm-bga (channel: orchestrator)
  3. Execute pc-tomedo-llm-extract-response
  4. Present interpretation to user

  MODEL: gemini-2.5-flash
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---


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

---

### Recipe: `tomedo-compose-python-marker` (class 21) — Tier 1

```
name:              "tomedo-compose-python-marker"
description:       "Generate a tomedo automatic Python marker using the zollsoft context file and LLM composition."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-compose-python-marker>", "<uuid:skill-tomedo>"],
    "label":   "Load Python-marker composition skill + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM receives zollsoft Python marker context + user requirement → generates Python code"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": [],
    "label":   "Extract code block from LLM response, format with paste instructions"
  }
]
intent_examples: [
  {"input": "python marker erstellen",                   "class": 2},
  {"input": "automatischer marker geburtstagshinweis",   "class": 2},
  {"input": "marker für patienten mit diabetes",         "class": 3},
  {"input": "create python marker tomedo",               "class": 2},
  {"input": "marker wenn kein termin in 90 tagen",       "class": 3},
  {"input": "automatischer hinweis in tagesliste",       "class": 2},
  {"input": "marker bei laborwert außerhalb normbereich","class": 3},
  {"input": "generate automatic marker tomedo",          "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-compose-statistic` (class 21) — Tier 1

```
name:              "tomedo-compose-statistic"
description:       "Generate a custom tomedo SQL statistics query using the zollsoft context file and LLM composition."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-compose-statistic>", "<uuid:skill-tomedo>"],
    "label":   "Load statistics composition skill + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM receives zollsoft statistics context + user requirement → generates SQL"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": [],
    "label":   "Extract SQL block, format with paste instructions for Statistiken module"
  }
]
intent_examples: [
  {"input": "statistik erstellen tomedo",                "class": 2},
  {"input": "sql auswertung patienten",                  "class": 2},
  {"input": "statistik alle patienten mit diagnose",     "class": 3},
  {"input": "generate statistics query tomedo",          "class": 2},
  {"input": "goä ziffer auswertung letztes quartal",     "class": 3},
  {"input": "ebm leistungsstatistik",                    "class": 2},
  {"input": "create custom statistic tomedo",            "class": 2},
  {"input": "medikamentenauswertung sql",                "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-compose-briefvorlage` (class 21) — Tier 1

```
name:              "tomedo-compose-briefvorlage"
description:       "Generate a tomedo letter template (Briefvorlage) HTML using the zollsoft context file and LLM composition."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-compose-briefvorlage>", "<uuid:skill-tomedo>"],
    "label":   "Load Briefvorlage composition skill + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM receives zollsoft Briefvorlage context + user requirement → generates HTML"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": [],
    "label":   "Extract HTML block, format with paste instructions for Briefschreibung"
  }
]
intent_examples: [
  {"input": "briefvorlage erstellen",                    "class": 2},
  {"input": "arztbrief vorlage tomedo",                  "class": 2},
  {"input": "befundbericht vorlage mit tabelle",         "class": 3},
  {"input": "create letter template tomedo",             "class": 2},
  {"input": "ct befund vorlage strukturiert",            "class": 3},
  {"input": "schlaflaborbericht vorlage",                "class": 2},
  {"input": "überweisungsschreiben vorlage",             "class": 2},
  {"input": "brief template with patient address",       "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-compose-cke` (class 21) — Tier 1

```
name:              "tomedo-compose-cke"
description:       "Generate a tomedo CustomKarteiEintrag (CKE) XML definition using the zollsoft context file and LLM composition."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-compose-cke>", "<uuid:skill-tomedo>"],
    "label":   "Load CKE composition skill + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM receives zollsoft CKE context + user requirement → generates XML"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": [],
    "label":   "Extract XML block, format with import instructions for CKE editor"
  }
]
intent_examples: [
  {"input": "cke erstellen tomedo",                      "class": 2},
  {"input": "custom karteieintrag impfung",              "class": 2},
  {"input": "tumordokumentation cke",                    "class": 2},
  {"input": "create custom kartei entry",                "class": 2},
  {"input": "cke für ild dokumentation",                 "class": 3},
  {"input": "strukturierter karteieintrag erstellen",    "class": 2},
  {"input": "customkarteieintrag xml generieren",        "class": 2},
  {"input": "schlafprotokoll cke cpap",                  "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-compose-patientenformular` (class 21) — Tier 1

```
name:              "tomedo-compose-patientenformular"
description:       "Generate a tomedo patient form (Patientenformular) JSON definition using the zollsoft context file and LLM composition."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-compose-patientenformular>", "<uuid:skill-tomedo>"],
    "label":   "Load Patientenformular composition skill + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM receives zollsoft patient form context + user requirement → generates JSON"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": [],
    "label":   "Extract JSON block, format with instructions for Patientenformulare JSON tab"
  }
]
intent_examples: [
  {"input": "patientenformular erstellen",               "class": 2},
  {"input": "fragebogen für patienten anlegen",          "class": 2},
  {"input": "anamnese fragebogen atemwege",              "class": 3},
  {"input": "create patient form tomedo",                "class": 2},
  {"input": "einverständniserklärung formular",          "class": 2},
  {"input": "triage fragebogen infekt kinder",           "class": 3},
  {"input": "patient questionnaire json tomedo",         "class": 2},
  {"input": "digital patient intake form",               "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-llm-arztbericht` (class 21) — Tier 1

```
name:              "tomedo-llm-arztbericht"
description:       "Extract structured findings, diagnoses, and recommendations from a medical report (Arztbericht) using the tomedo LLM service."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-llm-arztbericht>", "<uuid:skill-tomedo>"],
    "label":   "Load Arztbericht extraction leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-llm-chat>"],
    "label":   "Pre-load ts-tomedo-llm-chat binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-arztbericht>"],
    "label":   "POST to tomedo LLM service: structured Arztbericht extraction"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-extract-response>"],
    "label":   "Extract choices[0].message.content from LLM response"
  }
]
intent_examples: [
  {"input": "arztbericht auswerten",                     "class": 2},
  {"input": "befundbericht strukturieren",               "class": 2},
  {"input": "entlassbrief zusammenfassen",               "class": 2},
  {"input": "medical report extraction tomedo",          "class": 2},
  {"input": "diagnosen aus arztbericht extrahieren",     "class": 3},
  {"input": "überweisungsschreiben auswerten",           "class": 2},
  {"input": "befund zusammenfassen mit ki",              "class": 2},
  {"input": "extract findings from medical report",      "class": 2},
  {"input": "arztbrief ki analyse",                      "class": 2},
  {"input": "report text strukturieren",                 "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-llm-ct-befund` (class 21) — Tier 1

```
name:              "tomedo-llm-ct-befund"
description:       "Extract structured findings from a CT or MRI radiology report using the tomedo LLM service."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-llm-ct-befund>", "<uuid:skill-tomedo>"],
    "label":   "Load CT/MRI extraction leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-llm-chat>"],
    "label":   "Pre-load ts-tomedo-llm-chat binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-ct-befund>"],
    "label":   "POST to tomedo LLM service: CT/MRI structured extraction"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-extract-response>"],
    "label":   "Extract content from LLM response"
  }
]
intent_examples: [
  {"input": "ct befund auswerten",                       "class": 2},
  {"input": "mrt befund strukturieren",                  "class": 2},
  {"input": "radiologiebefund extrahieren",              "class": 2},
  {"input": "ct report extraction tomedo",               "class": 2},
  {"input": "thorax ct befund auswerten ki",             "class": 3},
  {"input": "röntgen befund zusammenfassen",             "class": 2},
  {"input": "extract ct findings",                       "class": 2},
  {"input": "mri report structured extraction",          "class": 2},
  {"input": "radiologie bericht analysieren",            "class": 2},
  {"input": "befund aus radiologie strukturieren",       "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-llm-schlaflabor` (class 21) — Tier 1

```
name:              "tomedo-llm-schlaflabor"
description:       "Extract structured sleep lab results (AHI, CPAP settings, ESS, diagnosis) from a Schlaflaborbericht using the tomedo LLM service."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-llm-schlaflabor>", "<uuid:skill-tomedo>"],
    "label":   "Load sleep-lab extraction leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-llm-chat>"],
    "label":   "Pre-load ts-tomedo-llm-chat binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-schlaflabor>"],
    "label":   "POST to tomedo LLM service: sleep lab structured extraction"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-extract-response>"],
    "label":   "Extract content from LLM response"
  }
]
intent_examples: [
  {"input": "schlaflaborbericht auswerten",              "class": 2},
  {"input": "osas diagnose aus bericht",                 "class": 2},
  {"input": "schlafapnoe befund strukturieren",          "class": 2},
  {"input": "polysomnographie auswertung ki",            "class": 2},
  {"input": "ahi ess aus schlaflabor extrahieren",       "class": 3},
  {"input": "cpap einstellung aus bericht",              "class": 2},
  {"input": "sleep lab report extraction",               "class": 2},
  {"input": "schlaflabor ki auswertung",                 "class": 2},
  {"input": "polygraphy report structured",              "class": 2},
  {"input": "sauerstoffsättigung nacht aus bericht",     "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-llm-laborbefund` (class 21) — Tier 1

```
name:              "tomedo-llm-laborbefund"
description:       "Interpret a laboratory report — flag out-of-range values and provide clinical context — using the tomedo LLM service."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-llm-laborbefund>", "<uuid:skill-tomedo>"],
    "label":   "Load lab-result interpretation leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-llm-chat>"],
    "label":   "Pre-load ts-tomedo-llm-chat binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-laborbefund>"],
    "label":   "POST to tomedo LLM service: lab result interpretation"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-extract-response>"],
    "label":   "Extract content from LLM response"
  }
]
intent_examples: [
  {"input": "laborbefund auswerten",                     "class": 2},
  {"input": "labor interpretieren ki",                   "class": 2},
  {"input": "welche laborwerte sind auffällig",          "class": 3},
  {"input": "lab report interpretation tomedo",          "class": 2},
  {"input": "blutwerte außerhalb referenzbereich",       "class": 2},
  {"input": "laborergebnis klinisch einschätzen",        "class": 2},
  {"input": "interpret lab results",                     "class": 2},
  {"input": "labor übersicht mit bewertung",             "class": 2},
  {"input": "auffällige laborwerte analysieren",         "class": 2},
  {"input": "pathologische laborwerte markieren",        "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-llm-gutachten` (class 21) — Tier 1

```
name:              "tomedo-llm-gutachten"
description:       "Draft a medical expert opinion (Gutachtenauftrag / ärztliche Stellungnahme) from patient context using the tomedo LLM service."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-llm-gutachten>", "<uuid:skill-tomedo>"],
    "label":   "Load Gutachten drafting leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-detail>", "<uuid:ts-tomedo-patient-relations>", "<uuid:ts-tomedo-patient-medications>"],
    "label":   "Pre-load patient data bindings for context assembly"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-detail>", "<uuid:pc-tomedo-patient-relations>", "<uuid:pc-tomedo-patient-medications>", "<uuid:pc-tomedo-format-patient-context>"],
    "label":   "Fetch patient context (detail + diagnoses + meds)"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-llm-chat>"],
    "label":   "Pre-load ts-tomedo-llm-chat binding"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-gutachten>"],
    "label":   "POST to tomedo LLM service (gemini-2.5-pro): draft Gutachtenauftrag"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-extract-response>"],
    "label":   "Extract draft text from LLM response"
  }
]
intent_examples: [
  {"input": "gutachtenauftrag erstellen",                "class": 2},
  {"input": "ärztliche stellungnahme verfassen",         "class": 2},
  {"input": "gutachten für patient schreiben",           "class": 3},
  {"input": "medical expert opinion draft tomedo",       "class": 2},
  {"input": "stellungnahme mdkgutachten",                "class": 2},
  {"input": "rentenversicherung gutachten patient",      "class": 3},
  {"input": "ärztliches attest automatisch",             "class": 2},
  {"input": "create medical assessment letter",          "class": 2},
  {"input": "begutachtung anfrage formulieren",          "class": 2},
  {"input": "sozialmedizinische stellungnahme ki",       "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-llm-patientenbrief` (class 21) — Tier 1

```
name:              "tomedo-llm-patientenbrief"
description:       "Draft a plain-language patient letter (Patientenbrief) using patient context from tomedo and the tomedo LLM service."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-llm-patientenbrief>", "<uuid:skill-tomedo>"],
    "label":   "Load Patientenbrief drafting leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-llm-chat>"],
    "label":   "Pre-load ts-tomedo-llm-chat binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-patientenbrief>"],
    "label":   "POST to tomedo LLM service: draft patient letter"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-extract-response>"],
    "label":   "Extract letter draft from LLM response"
  }
]
intent_examples: [
  {"input": "patientenbrief schreiben",                  "class": 2},
  {"input": "brief an patient verfassen ki",             "class": 2},
  {"input": "terminbestätigung brief patient",           "class": 2},
  {"input": "patient letter draft tomedo",               "class": 2},
  {"input": "laborbefund brief an patient",              "class": 3},
  {"input": "nachsorgeempfehlung schreiben patient",     "class": 3},
  {"input": "write patient letter in plain language",    "class": 2},
  {"input": "befundergebnis brief erstellen",            "class": 2},
  {"input": "entlassungsbrief für patient",              "class": 2},
  {"input": "brief in einfacher sprache patient",        "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-llm-uebersetzung` (class 21) — Tier 1

```
name:              "tomedo-llm-uebersetzung"
description:       "Translate a medical text to or from German using the tomedo LLM service (DSGVO-compliant, zollsoft infrastructure)."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-llm-uebersetzung>", "<uuid:skill-tomedo>"],
    "label":   "Load medical translation leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-llm-chat>"],
    "label":   "Pre-load ts-tomedo-llm-chat binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-uebersetzung>"],
    "label":   "POST to tomedo LLM service: translate medical text"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-extract-response>"],
    "label":   "Extract translation from LLM response"
  }
]
intent_examples: [
  {"input": "medizinischen text übersetzen",             "class": 2},
  {"input": "arztbrief ins englische übersetzen",        "class": 3},
  {"input": "befund auf türkisch übersetzen",            "class": 3},
  {"input": "translate medical text tomedo",             "class": 2},
  {"input": "fremdsprachiger befund deutsch",            "class": 2},
  {"input": "englischer entlassbrief übersetzen",        "class": 3},
  {"input": "medical translation for patient",           "class": 2},
  {"input": "ausländischer patient arztbrief",           "class": 2},
  {"input": "befund russisch übersetzen",                "class": 3},
  {"input": "dsgvo konform übersetzen ki",               "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-llm-bga` (class 21) — Tier 1

```
name:              "tomedo-llm-bga"
description:       "Interpret a blood gas analysis (BGA) — classify acid-base disturbance, oxygenation, and give clinical recommendation — using the tomedo LLM service."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-llm-bga>", "<uuid:skill-tomedo>"],
    "label":   "Load BGA interpretation leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-llm-chat>"],
    "label":   "Pre-load ts-tomedo-llm-chat binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-bga>"],
    "label":   "POST to tomedo LLM service: BGA interpretation"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-llm-extract-response>"],
    "label":   "Extract BGA interpretation from LLM response"
  }
]
intent_examples: [
  {"input": "bga auswerten",                             "class": 2},
  {"input": "blutgasanalyse interpretieren",             "class": 2},
  {"input": "bga azidose oder alkalose",                 "class": 3},
  {"input": "blood gas analysis interpretation",         "class": 2},
  {"input": "ph wert bga einschätzen",                   "class": 2},
  {"input": "respiratorische azidose bga",               "class": 2},
  {"input": "metabolische alkalose bga",                 "class": 2},
  {"input": "bga ki analyse",                            "class": 2},
  {"input": "sauerstoffsättigung bga bewerten",          "class": 2},
  {"input": "bga intensivstation auswertung",            "class": 3}
]
source: "system"
validation_status: "validated"
```


---

### Recipe: `tomedo-leistungen-read` (class 21) — Tier 0

```
name:              "tomedo-leistungen-read"
class_code:        21
description:       "Read Leistungen (billing codes) for a patient via the safe two-step KV-Schein path."
llm_call_required: false
rust_steps: [
  {uuid: "...", component_name: "ts-tomedo-patient-relations"},
  {uuid: "...", component_name: "ts-tomedo-kvschein-get"},
  {uuid: "...", component_name: "ts-tomedo-ebmkatalogeintrag-get"}
]
orchestrator_steps: [
  {uuid: "...", component_name: "pc-tomedo-patient-relations"},
  {uuid: "...", component_name: "pc-tomedo-kvschein-get"}
]
intent_examples: [
  {"input": "welche leistungen hat der patient",              "class": 3},
  {"input": "EBM-Ziffern für patient 13550",                  "class": 3},
  {"input": "zeige abgerechnete leistungen",                  "class": 3},
  {"input": "which billing codes for the patient",            "class": 2},
  {"input": "leistungen aus dem aktuellen schein",            "class": 3},
  {"input": "GOÄ leistungen für patient",                     "class": 3},
  {"input": "EBM ziffer 03220 vorhanden",                     "class": 2},
  {"input": "was wurde heute abgerechnet",                    "class": 2},
  {"input": "abrechnungsziffern anzeigen",                    "class": 3},
  {"input": "fetch billing codes patient",                    "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-karteieintrag-create` (class 21) — Tier 1

```
name:              "tomedo-karteieintrag-create"
class_code:        21
description:       "Create a new KarteiEintrag for a patient — LLM confirms content, orchestrator POSTs."
llm_call_required: true
rust_steps: [
  {uuid: "...", component_name: "ts-tomedo-karteieintrag-create"}
]
orchestrator_steps: [
  {uuid: "...", component_name: "pc-tomedo-karteieintrag-create"}
]
intent_examples: [
  {"input": "neuen karteieintrag erstellen",                  "class": 3},
  {"input": "karteinotiz für patient hinzufügen",             "class": 3},
  {"input": "create karteieintrag for patient",               "class": 2},
  {"input": "telefonnotiz in kartei eintragen",               "class": 3},
  {"input": "arztbrief in kartei speichern",                  "class": 3},
  {"input": "write medical note for patient",                 "class": 2},
  {"input": "neuen akte eintrag anlegen",                     "class": 3},
  {"input": "lab result note kartei",                         "class": 2},
  {"input": "karteieintrag mit text BRIEF anlegen",           "class": 3},
  {"input": "save consultation note to tomedo",               "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-karteieintrag-update` (class 21) — Tier 1

```
name:              "tomedo-karteieintrag-update"
class_code:        21
description:       "Update or soft-delete an existing KarteiEintrag — LLM confirms change, orchestrator PUTs."
llm_call_required: true
rust_steps: [
  {uuid: "...", component_name: "ts-tomedo-karteieintrag-update"}
]
orchestrator_steps: [
  {uuid: "...", component_name: "pc-tomedo-karteieintrag-update"}
]
intent_examples: [
  {"input": "karteieintrag ausblenden",                       "class": 3},
  {"input": "karteinotiz löschen",                            "class": 3},
  {"input": "hide karteieintrag",                             "class": 2},
  {"input": "karteieintrag text ändern",                      "class": 3},
  {"input": "eintrag in kartei korrigieren",                  "class": 3},
  {"input": "update medical record entry",                    "class": 2},
  {"input": "karteieintrag sichtbarkeit ändern",              "class": 3},
  {"input": "soft delete kartei note",                        "class": 2},
  {"input": "visible false karteieintrag",                    "class": 3},
  {"input": "edit existing record entry tomedo",              "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-termin-create` (class 21) — Tier 1

```
name:              "tomedo-termin-create"
class_code:        21
description:       "Create a new appointment (Termin) for a patient — LLM confirms datetime and info, orchestrator POSTs."
llm_call_required: true
rust_steps: [
  {uuid: "...", component_name: "ts-tomedo-termin-create"}
]
orchestrator_steps: [
  {uuid: "...", component_name: "pc-tomedo-termin-create"}
]
intent_examples: [
  {"input": "termin anlegen für patient",                     "class": 3},
  {"input": "neuen termin erstellen",                         "class": 3},
  {"input": "create appointment for patient",                 "class": 2},
  {"input": "terminbuchung patient 13550",                    "class": 3},
  {"input": "schedule appointment tomedo",                    "class": 2},
  {"input": "wiedervorstellung termin buchen",                "class": 3},
  {"input": "nächsten termin anlegen",                        "class": 3},
  {"input": "book follow-up appointment",                     "class": 2},
  {"input": "termin für kontrolluntersuchung",                "class": 3},
  {"input": "new termin datum uhrzeit",                       "class": 3}
]
source: "system"
validation_status: "validated"
```


## Step 6 — ExtensionCatalogues (class 23)

Three catalogues: tomedo REST API, tomedo-crawl sidecar, tomedo LLM service,
and one additional catalogue for the cert-fetch setup extension.

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
  AUTH:     Mutual TLS (mTLS) client certificate — PEM files from /opt/data/apiConnector/ssl/.
            No Authorization header needed. Use tomedo-cert-fetch recipe for first-time setup.
  PROTOCOL: HTTPS. GET=reads, POST/PUT=writes (confirmed live 2026-08-22).

  CONFIRMED API SURFACE (probed live 2026-08-22):
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ GET  /serverstatus                     → server version + revision       │
  │ GET  /patient?flach=true              → flat list (~15k) — BULK/CRASH   │
  │ GET  /patient/{id}                    → full record + phone numbers      │
  │ GET  /patient/{id}/patientenDetails.. → diagnoses, Kartei, Behandlung   │
  │ GET  /patient/{id}/.../medikamentenPlan→ medication plan + dosing       │
  │ GET  /patient/{id}/termine?flach=true → appointments                    │
  │ GET  /besuch/{id}/besucheForPatient   → visit records                   │
  │ GET  /kvschein/{ident}                → KV-Schein + ebmLeistungen[]     │
  │ GET  /ebmkatalogeintrag/{ident}       → EBM catalog ident → Ziffer str  │
  │ POST /karteieintrag                   → create; returns {ident}         │
  │ POST /termin                          → create appointment; returns {ident}│
  │ PUT  /karteieintrag/{id}              → update; visible:false=soft-del  │
  │ PUT  /termin/{id}                     → update; removed:true=cancel     │
  │ PUT  /patient/{id}                    → update; gesperrt:1(int)=block   │
  └─────────────────────────────────────────────────────────────────────────┘
  BROKEN:  GET /patient/searchByAttributes → returns {} for all queries.
  CRASH:   GET /leistung?patient=X, /patient/{id}/leistungen — unbounded queries.
  Leistungen safe path: patientenDetailsRelationen?limitScheine=true → GET /kvschein/{ident}.
  Briefkommandos ($[l %nr 0d ,]$) reveal the data model — see §briefkommandos.

  WRITE TYPE FACTS: gesperrt=Integer(1), removed=Boolean, visible=Boolean.
  Error responses use HTTP 460 with Java stack trace.

  LLM OBJECT COMPOSITION (§llm-objects):
  Composition recipes generate tomedo objects (Python markers, SQL stats,
  letter templates, CKEs, patient forms) using zollsoft context files + LLM.
  Output is copy-pasteable.

  TASK GROUPS:
  1. Health checks:    tomedo-serverstatus
  2. Patient reads:    tomedo-patient-detail, tomedo-patient-diagnoses,
                       tomedo-patient-medications, tomedo-patient-next-appointment,
                       tomedo-patient-visits
  3. Leistungen read:  tomedo-leistungen-read (Tier 0, two-step safe path)
  4. Patient search:   tomedo-patient-search-by-name (Tier 1)
  5. Full context:     tomedo-patient-full-context (composed)
  6. Writes (Tier 1):  tomedo-karteieintrag-create, tomedo-karteieintrag-update,
                       tomedo-termin-create
  7. Object composition (Tier 1, LLM):
     tomedo-compose-python-marker, tomedo-compose-statistic,
     tomedo-compose-briefvorlage, tomedo-compose-cke,
     tomedo-compose-patientenformular

  KEY DATA SHAPES:
  • geburtsDatum: epoch ms, may be negative (before 1970)
  • Phone fields: patientenDetails.kontaktdaten.{telefon,telefon2,handyNummer,fax}
  • Diagnoses: diagnosen[].freitext + typ ('G'=confirmed, 'V'=suspected)
  • Medications: nameBeiVerordnung + dosierungFrueh/Mittag/Abend/Nacht
  • Appointments: beginn/ende as epoch ms
  • EBMLeistung: ebmKatalogEintrag.ident is internal int, NOT the Ziffer string

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
    "group_name": "leistungen-read",
    "summary": "Read EBM/GOÄ billing codes via safe two-step kvschein path (Tier 0)",
    "recipe_ids": ["tomedo-leistungen-read"]
  },
  {
    "group_name": "patient-search",
    "summary": "Search patients by name (Tier 1, LLM required)",
    "recipe_ids": ["tomedo-patient-search-by-name"]
  },
  {
    "group_name": "writes",
    "summary": "Create/update KarteiEinträge and Termine via direct mTLS REST (Tier 1)",
    "recipe_ids": [
      "tomedo-karteieintrag-create",
      "tomedo-karteieintrag-update",
      "tomedo-termin-create"
    ]
  },
  {
    "group_name": "llm-object-composition",
    "summary": "Generate tomedo configuration objects (markers, SQL, templates, CKEs, forms) via LLM — Tier 1",
    "recipe_ids": [
      "tomedo-compose-python-marker",
      "tomedo-compose-statistic",
      "tomedo-compose-briefvorlage",
      "tomedo-compose-cke",
      "tomedo-compose-patientenformular"
    ]
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

### ExtensionCatalogue: `ext-tomedo-llm` (class 23)

```
name:        "ext-tomedo-llm"
description: "tomedo LLM service integration — DSGVO-compliant medical text analysis using the tomedo server's built-in OpenAI-compatible LLM endpoint."
version:     "1.0"
overview_doc: |
  This catalogue covers all components for calling the tomedo LLM service —
  the same backend as the tomedo Kartei-Chat, accessible as a REST endpoint.

  BASE URL (choose one):
    HTTPS mTLS: https://{host}:8443/{db}/llmservice/{user_ident}/v1/chat/completions
    LAN HTTP:   http://tomedo.localnet:8080/tomedo_live/llmservice/{user_ident}/v1/chat/completions
  AUTH:     Same mTLS cert as the rest of the tomedo API (HTTPS variant).
            LAN HTTP variant: no cert required, must be on practice LAN.
  NOTE:     Monthly budget limit per user (zollsoft enforced). Budget errors surface
            as non-200 responses — always relay to user.

  WHY USE THIS SERVICE:
  The tomedo LLM service provides DSGVO-compliant LLM inference without external
  API keys. All models run on zollsoft-operated zero-retention infrastructure.
  No patient data leaves the EU. This is ideal for medical text analysis,
  report extraction, letter drafting, and BGA interpretation.

  AVAILABLE MODELS:
  • gemini-2.5-flash     — recommended default (fast, good quality)
  • gemini-2.5-pro       — highest quality (use for formal documents, Gutachten)
  • mistral-medium-2508  — EU/DSGVO Mistral alternative

  REQUIRED CONFIG KEYS:
  • tomedo_user_ident     — numeric user ID from tomedo (t_benutzer.ident)
  • tomedo_llm_endpoint   — base URL without /llmservice/... path

  TOOL: tomedo-llm-api (wraps builtin.http POST)
  TOOLSKILL: ts-tomedo-llm-chat (one ToolSkill for ALL prompt types)
  PYTHONCODE EXECUTORS: one per clinical use case (one function per skill rule)
  RESPONSE EXTRACTION: pc-tomedo-llm-extract-response (pure logic, no I/O)

  TASK GROUPS:
  1. Report extraction:   tomedo-llm-arztbericht, tomedo-llm-ct-befund,
                          tomedo-llm-schlaflabor, tomedo-llm-laborbefund
  2. Text generation:     tomedo-llm-gutachten, tomedo-llm-patientenbrief
  3. Text processing:     tomedo-llm-uebersetzung
  4. Clinical analysis:   tomedo-llm-bga

task_groups: [
  {
    "group_name": "report-extraction",
    "summary": "Extract structured data from medical reports (Arztbericht, CT, Schlaflabor, Labor)",
    "recipe_ids": [
      "tomedo-llm-arztbericht",
      "tomedo-llm-ct-befund",
      "tomedo-llm-schlaflabor",
      "tomedo-llm-laborbefund"
    ]
  },
  {
    "group_name": "text-generation",
    "summary": "Draft formal medical documents and patient letters via LLM (Tier 1)",
    "recipe_ids": ["tomedo-llm-gutachten", "tomedo-llm-patientenbrief"]
  },
  {
    "group_name": "text-processing",
    "summary": "Translate medical texts without leaving the EU (DSGVO-compliant)",
    "recipe_ids": ["tomedo-llm-uebersetzung"]
  },
  {
    "group_name": "clinical-analysis",
    "summary": "Interpret clinical values — blood gas analysis (BGA)",
    "recipe_ids": ["tomedo-llm-bga"]
  }
]
consumer_tags:   ["02:orchestrator", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### ExtensionCatalogue: `ext-tomedo-cert-fetch` (class 23)

```
name:        "ext-tomedo-cert-fetch"
description: "tomedo mTLS certificate setup — fetch client cert, private key, and CA cert from the tomedo server via SSH and store them locally so BrassClaw can make mTLS API calls."
version:     "1.0"
overview_doc: |
  This catalogue covers the one-time (and re-runnable) setup operation that
  retrieves the mTLS certificate bundle from the tomedo server's
  /opt/data/apiConnector/ssl/ directory and writes the three PEM files to
  a local path on the BrassClaw host.

  WITHOUT THESE CERTS: BrassClaw cannot call the tomedo REST API at all.
  Run this setup recipe first before any other tomedo recipe.

  CERT FILES ON TOMEDO SERVER (confirmed path 2026-08-22):
  ┌────────────────────────────────────────────────────────────────────────┐
  │ /opt/data/apiConnector/ssl/client_certificate.pem   — client cert      │
  │ /opt/data/apiConnector/ssl/client_private_key.pem   — EC P-521 key     │
  │ /opt/data/apiConnector/ssl/root_certificate.pem     — CA cert          │
  └────────────────────────────────────────────────────────────────────────┘

  WHAT THE RECIPE DOES:
  1. SSH to the tomedo server (using tomedo_ssh_host, tomedo_ssh_user,
     tomedo_ssh_key_path config keys)
  2. cat each of the three PEM files
  3. Write them to tomedo_cert_dir (default: ~/.brassclaw/tomedo-certs/)
  4. Set tomedo_cert_pem config key to the local path of the combined cert+key
  5. Verify by calling GET /serverstatus with the new certs

  REQUIRED CONFIG KEYS (set before running):
  • tomedo_ssh_host     — e.g. 192.168.10.9
  • tomedo_ssh_user     — e.g. technik
  • tomedo_ssh_key_path — path to SSH private key (or use tomedo_ssh_password)
  • tomedo_cert_dir     — local dir to write PEM files (default: ~/.brassclaw/tomedo-certs/)

  WRITTEN CONFIG KEYS (after successful run):
  • tomedo_cert_pem     — path to client cert PEM (combined cert + key)
  • tomedo_base_url     — https://{tomedo_ssh_host}:8443/tomedo_live (auto-set)

  TOOL USED: builtin.shell (SSH + cat + file write)
  TIER: 1 — user must confirm SSH credentials before execution.

  SECURITY: The private key is an EC P-521 key. It is written to disk with
  mode 0600. BrassClaw must never log the key content. The SSH password
  must come from a BrassClaw secret (not a plaintext config key).

  TASK GROUPS:
  1. Setup: tomedo-cert-fetch

task_groups: [
  {
    "group_name": "cert-setup",
    "summary": "Fetch mTLS certs from tomedo server via SSH and configure BrassClaw for HTTPS API access",
    "recipe_ids": ["tomedo-cert-fetch"]
  }
]
consumer_tags:   ["02:orchestrator", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Tool: `tomedo-cert-fetch-tool` (class 0)

```
name:            "tomedo-cert-fetch-tool"
description:     "Fetch the three mTLS PEM files from the tomedo server's
                  /opt/data/apiConnector/ssl/ directory via SSH and write them
                  to a local directory on the BrassClaw host.

                  Uses builtin.shell to run:
                    ssh {user}@{host} 'cat /opt/data/apiConnector/ssl/{file}.pem'
                  and then writes the output to local paths.

                  REQUIRED inputs:
                    ssh_host     — tomedo server hostname/IP
                    ssh_user     — SSH username (e.g. technik)
                    ssh_key_path — path to local SSH private key, OR
                    ssh_password — SSH password (from BrassClaw secret)
                    cert_dir     — local directory to write PEM files to

                  Files written:
                    {cert_dir}/client_certificate.pem
                    {cert_dir}/client_private_key.pem   (chmod 0600)
                    {cert_dir}/root_certificate.pem

                  ⚠️ Never log the private key content. Write with 0600 permissions."
capability_id:   "builtin.shell"
effect_type:     "write"
param_schema: {
  "type": "object",
  "properties": {
    "command":     {"type": "string", "description": "Shell command to execute (constructed by LLM from template)"}
  },
  "required": ["command"]
}
preconditions:   "SSH access to tomedo server must be available.
                  cert_dir must be writable.
                  tomedo_ssh_host, tomedo_ssh_user, and SSH credentials must be configured."
error_handling:  "SSH auth failure: surface error, ask user to verify credentials.
                  File write failure: surface error + suggest mkdir.
                  Connection refused: server not reachable — check VPN/LAN."
consumer_tags:   ["00:rusty", "02:orchestrator", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### ToolSkill: `ts-tomedo-cert-fetch` (class 13)

```
name:          "ts-tomedo-cert-fetch"
tool_name:     "tomedo-cert-fetch-tool"
description:   "SSH to the tomedo server and fetch the three mTLS PEM files:
                client_certificate.pem, client_private_key.pem, root_certificate.pem
                from /opt/data/apiConnector/ssl/.
                Writes them to {{vars.tomedo_cert_dir}}.
                Also sets tomedo_cert_pem and tomedo_base_url config keys on success.
                Tier 1 — LLM constructs the SSH command from user-provided host/user/key."
param_schema:  [
  {name: "command", param_type: "string", required: true,
   description: "Shell commands to SSH-fetch the three PEM files and write locally"}
]
param_template: {
  "command": "ssh -i {{vars.tomedo_ssh_key_path}} -o StrictHostKeyChecking=no {{vars.tomedo_ssh_user}}@{{vars.tomedo_ssh_host}} 'cat /opt/data/apiConnector/ssl/client_certificate.pem' > {{vars.tomedo_cert_dir}}/client_certificate.pem && ssh -i {{vars.tomedo_ssh_key_path}} -o StrictHostKeyChecking=no {{vars.tomedo_ssh_user}}@{{vars.tomedo_ssh_host}} 'cat /opt/data/apiConnector/ssl/client_private_key.pem' > {{vars.tomedo_cert_dir}}/client_private_key.pem && chmod 0600 {{vars.tomedo_cert_dir}}/client_private_key.pem && ssh -i {{vars.tomedo_ssh_key_path}} -o StrictHostKeyChecking=no {{vars.tomedo_ssh_user}}@{{vars.tomedo_ssh_host}} 'cat /opt/data/apiConnector/ssl/root_certificate.pem' > {{vars.tomedo_cert_dir}}/root_certificate.pem"
}
preconditions:  "tomedo_ssh_host, tomedo_ssh_user, and tomedo_ssh_key_path (or password) must be set.
                 tomedo_cert_dir must exist (create with mkdir -p if needed).
                 SSH access to the tomedo server must be available."
error_handling: "Permission denied (SSH): invalid credentials. No route to host: server unreachable.
                 Empty file written: cat failed — file path may have changed."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### PythonCode: `pc-tomedo-cert-fetch` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# §shell-guard: command string contains runtime vars — llm_call_required: true.
# Fetches the three tomedo mTLS PEM files from the server via SSH.
# IBS bakes in all vars before execution.
_host  = "{{vars.tomedo_ssh_host}}"
_user  = "{{vars.tomedo_ssh_user}}"
_key   = "{{vars.tomedo_ssh_key_path}}"
_dir   = "{{vars.tomedo_cert_dir}}"
if not _host or not _user or not _dir:
    result = {"error": "tomedo_ssh_host, tomedo_ssh_user, and tomedo_cert_dir must all be set"}
else:
    _key_flag = f"-i {_key}" if _key else ""
    _cmd = (
        f"mkdir -p {_dir} && "
        f"ssh {_key_flag} -o StrictHostKeyChecking=no -o BatchMode=yes {_user}@{_host} "
        f"'cat /opt/data/apiConnector/ssl/client_certificate.pem' "
        f"> {_dir}/client_certificate.pem && "
        f"ssh {_key_flag} -o StrictHostKeyChecking=no -o BatchMode=yes {_user}@{_host} "
        f"'cat /opt/data/apiConnector/ssl/client_private_key.pem' "
        f"> {_dir}/client_private_key.pem && "
        f"chmod 0600 {_dir}/client_private_key.pem && "
        f"ssh {_key_flag} -o StrictHostKeyChecking=no -o BatchMode=yes {_user}@{_host} "
        f"'cat /opt/data/apiConnector/ssl/root_certificate.pem' "
        f"> {_dir}/root_certificate.pem && "
        f"echo OK"
    )
    result = __execute_action__("tomedo-cert-fetch-tool", {"command": _cmd})
```

---

### Leaf Skill: `skill-tomedo-cert-fetch` (class 1)

```
name:        "skill-tomedo-cert-fetch"
class_code:  1
description: "Leaf skill: fetch the tomedo mTLS client certs from the server via SSH — one-time setup, Tier 1."
body: |
  Fetch the three mTLS PEM files from the tomedo server's /opt/data/apiConnector/ssl/
  directory via SSH and write them to {{vars.tomedo_cert_dir}} on the local host.

  This is a one-time setup operation. Run it once before any other tomedo recipe.

  REQUIRED CONFIG KEYS:
    tomedo_ssh_host     — tomedo server IP or hostname (e.g. 192.168.10.9)
    tomedo_ssh_user     — SSH username on the tomedo server (e.g. technik)
    tomedo_ssh_key_path — path to local SSH private key, OR use password auth
    tomedo_cert_dir     — local dir for PEM files (default: ~/.brassclaw/tomedo-certs/)

  AFTER SUCCESSFUL FETCH:
    Set tomedo_cert_pem  = {tomedo_cert_dir}/client_certificate.pem
    Set tomedo_base_url  = https://{tomedo_ssh_host}:8443/tomedo_live
    Verify by calling skill-tomedo-serverstatus.

  ⚠️ SECURITY: The private key is written with chmod 0600. Never log or display
  the key content. SSH password must come from a BrassClaw secret (not plaintext config).

  Before dispatching: show the user the SSH command and ask for confirmation.
  PythonCode: use pc-tomedo-cert-fetch.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-cert-fetch` (class 21) — Tier 1

```
name:              "tomedo-cert-fetch"
class_code:        21
description:       "One-time setup: fetch mTLS certs from the tomedo server via SSH and configure BrassClaw for HTTPS API access."
llm_call_required: true
rust_steps: [
  {uuid: "...", component_name: "ts-tomedo-cert-fetch"},
  {uuid: "...", component_name: "ts-tomedo-serverstatus"}
]
orchestrator_steps: [
  {uuid: "...", component_name: "pc-tomedo-cert-fetch"},
  {uuid: "...", component_name: "pc-tomedo-serverstatus"}
]
intent_examples: [
  {"input": "tomedo zertifikate einrichten",                  "class": 3},
  {"input": "mTLS certs für tomedo holen",                    "class": 3},
  {"input": "fetch tomedo API certificates",                  "class": 2},
  {"input": "tomedo verbindung einrichten",                   "class": 3},
  {"input": "setup tomedo connection",                        "class": 2},
  {"input": "tomedo SSL setup",                               "class": 2},
  {"input": "zertifikate vom tomedo server laden",            "class": 3},
  {"input": "BrassClaw tomedo konfigurieren",                 "class": 3},
  {"input": "configure tomedo API access",                    "class": 2},
  {"input": "tomedo cert setup first time",                   "class": 2}
]
source: "system"
validation_status: "validated"
```


## Step 7 — Component Summary & Seeding Order

### Complete Component Count (tomedo v3 stack)

| Class | Count | Names |
|-------|-------|-------|
| 0 — Tool | 4 | `tomedo-api`, `tomedo-crawl-api`, `tomedo-llm-api`, `tomedo-cert-fetch-tool` |
| 1 — Leaf Skill | 37 | `skill-tomedo-serverstatus` … `skill-tomedo-cert-fetch` |
| 2 — Domain Skill | 2 | `skill-tomedo`, `skill-tomedo-crawl` |
| 13 — ToolSkill | 21 | `ts-tomedo-serverstatus` … `ts-tomedo-cert-fetch` |
| 21 — Recipe | 32 | `tomedo-serverstatus` … `tomedo-cert-fetch` |
| 22 — PythonCode | 36 | `pc-tomedo-serverstatus` … `pc-tomedo-cert-fetch` |
| 23 — ExtensionCatalogue | 4 | `ext-tomedo`, `ext-tomedo-crawl`, `ext-tomedo-llm`, `ext-tomedo-cert-fetch` |
| **Total** | **136** | |

---

### Tier Classification Summary

| Tier | Recipes | Reason |
|------|---------|--------|
| **Tier 0** | 15 | Read ops with known params + Leistungen two-step path — deterministic |
| **Tier 1** | 17 | 1 search + 3 writes + 1 cert-fetch + 5 composition + 8 LLM service recipes |

---

### Seeding Order (bootstrapped in this order per group)

```
Group 1 — Tools (class 0):
  1. tomedo-api
  2. tomedo-crawl-api
  3. tomedo-llm-api

Group 2 — ToolSkills (class 13):
  4. ts-tomedo-serverstatus
  5. ts-tomedo-patient-list
  6. ts-tomedo-patient-detail
  7. ts-tomedo-patient-relations
  8. ts-tomedo-patient-medications
  9. ts-tomedo-patient-appointments
  10. ts-tomedo-patient-visits
  11. ts-tomedo-patient-search
  12. ts-tomedo-crawl-health
  13. ts-tomedo-crawl-register-caller
  14. ts-tomedo-crawl-get-caller
  15. ts-tomedo-crawl-rag-query
  16. ts-tomedo-crawl-trigger
  17. ts-tomedo-crawl-config-read
  18. ts-tomedo-llm-chat

Group 3 — PythonCode executors (class 22, with __execute_action__):
  19. pc-tomedo-serverstatus
  20. pc-tomedo-patient-list
  21. pc-tomedo-patient-detail
  22. pc-tomedo-patient-relations
  23. pc-tomedo-patient-medications
  24. pc-tomedo-patient-appointments
  25. pc-tomedo-patient-visits
  26. pc-tomedo-crawl-health
  27. pc-tomedo-crawl-register-caller
  28. pc-tomedo-crawl-get-caller
  29. pc-tomedo-crawl-rag-query
  30. pc-tomedo-crawl-trigger
  31. pc-tomedo-crawl-config-read
  32. pc-tomedo-llm-arztbericht
  33. pc-tomedo-llm-ct-befund
  34. pc-tomedo-llm-schlaflabor
  35. pc-tomedo-llm-laborbefund
  36. pc-tomedo-llm-gutachten
  37. pc-tomedo-llm-patientenbrief
  38. pc-tomedo-llm-uebersetzung
  39. pc-tomedo-llm-bga

Group 3b — PythonCode executors — new write + Leistungen reads:
  40. pc-tomedo-karteieintrag-create
  41. pc-tomedo-karteieintrag-update
  42. pc-tomedo-termin-create
  43. pc-tomedo-kvschein-get
  44. pc-tomedo-ebmkatalogeintrag-get

Group 3c — PythonCode executor — cert-fetch setup:
  45. pc-tomedo-cert-fetch

Group 4 — PythonCode pure-logic helpers (class 22, no __execute_action__):
  46. pc-tomedo-parse-diagnosen
  47. pc-tomedo-parse-medications
  48. pc-tomedo-parse-next-appointment
  49. pc-tomedo-epoch-to-date
  50. pc-tomedo-format-patient-context
  51. pc-tomedo-extract-phone-fields
  52. pc-tomedo-filter-recent-patients
  53. pc-tomedo-llm-extract-response

Group 5 — Leaf Skills (class 1) — REST API reads + crawl:
  54. skill-tomedo-serverstatus
  55. skill-tomedo-patient-list
  56. skill-tomedo-patient-detail
  57. skill-tomedo-patient-diagnoses
  58. skill-tomedo-patient-medications
  59. skill-tomedo-patient-appointments
  60. skill-tomedo-patient-visits
  61. skill-tomedo-patient-search-by-name
  62. skill-tomedo-crawl-health
  63. skill-tomedo-crawl-phone-lookup
  64. skill-tomedo-crawl-rag-query
  65. skill-tomedo-crawl-trigger
  66. skill-tomedo-crawl-config-read
  67. skill-tomedo-format-context

Group 5a — Leaf Skills (class 1) — writes + Leistungen:
  68. skill-tomedo-karteieintrag-create
  69. skill-tomedo-karteieintrag-update
  70. skill-tomedo-termin-create
  71. skill-tomedo-leistungen-read

Group 5b — Leaf Skills (class 1) — LLM object composition:
  72. skill-tomedo-compose-python-marker
  73. skill-tomedo-compose-statistic
  74. skill-tomedo-compose-briefvorlage
  75. skill-tomedo-lookup-briefkommando
  76. skill-tomedo-compose-cke
  77. skill-tomedo-compose-patientenformular

Group 5c — Leaf Skills (class 1) — LLM service:
  78. skill-tomedo-llm-arztbericht
  79. skill-tomedo-llm-ct-befund
  80. skill-tomedo-llm-schlaflabor
  81. skill-tomedo-llm-laborbefund
  82. skill-tomedo-llm-gutachten
  83. skill-tomedo-llm-patientenbrief
  84. skill-tomedo-llm-uebersetzung
  85. skill-tomedo-llm-bga

Group 5d — Leaf Skill (class 1) — cert-fetch setup:
  86. skill-tomedo-cert-fetch

Group 6 — Domain Skills (class 2):
  87. skill-tomedo
  88. skill-tomedo-crawl

Group 7 — Recipes (class 21):
  89. tomedo-serverstatus                           (Tier 0)
  90. tomedo-patient-detail                         (Tier 0)
  91. tomedo-patient-diagnoses                      (Tier 0)
  92. tomedo-patient-medications                    (Tier 0)
  93. tomedo-patient-next-appointment               (Tier 0)
  94. tomedo-patient-visits                         (Tier 0)
  95. tomedo-phone-lookup                           (Tier 0)
  96. tomedo-rag-query                              (Tier 0)
  97. tomedo-rag-query-for-patient                  (Tier 0)
  98. tomedo-crawl-health                           (Tier 0)
  99. tomedo-crawl-trigger                          (Tier 0)
  100. tomedo-crawl-config-read                     (Tier 0)
  101. tomedo-patient-full-context                  (Tier 0, multi-step)
  102. tomedo-leistungen-read                       (Tier 0 — kvschein two-step)
  103. tomedo-patient-search-by-name                (Tier 1 — LLM query)
  104. tomedo-karteieintrag-create                  (Tier 1 — LLM confirms, orchestrator POSTs)
  105. tomedo-karteieintrag-update                  (Tier 1 — LLM confirms, orchestrator PUTs)
  106. tomedo-termin-create                         (Tier 1 — LLM confirms, orchestrator POSTs)
  107. tomedo-compose-python-marker                 (Tier 1 — LLM compose, context file)
  108. tomedo-compose-statistic                     (Tier 1 — LLM compose, context file)
  109. tomedo-compose-briefvorlage                  (Tier 1 — LLM compose, context file)
  110. tomedo-compose-cke                           (Tier 1 — LLM compose, context file)
  111. tomedo-compose-patientenformular             (Tier 1 — LLM compose, context file)
  112. tomedo-llm-arztbericht                       (Tier 1 — tomedo LLM service)
  113. tomedo-llm-ct-befund                         (Tier 1 — tomedo LLM service)
  114. tomedo-llm-schlaflabor                       (Tier 1 — tomedo LLM service)
  115. tomedo-llm-laborbefund                       (Tier 1 — tomedo LLM service)
  116. tomedo-llm-gutachten                         (Tier 1 — tomedo LLM service)
  117. tomedo-llm-patientenbrief                    (Tier 1 — tomedo LLM service)
  118. tomedo-llm-uebersetzung                      (Tier 1 — tomedo LLM service)
  119. tomedo-llm-bga                               (Tier 1 — tomedo LLM service)
  120. tomedo-cert-fetch                            (Tier 1 — one-time setup, LLM confirms SSH)

Group 8 — ExtensionCatalogues (class 23):
  121. ext-tomedo
  122. ext-tomedo-crawl
  123. ext-tomedo-llm
  124. ext-tomedo-cert-fetch
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
| Four surfaces via `builtin.http` + `builtin.shell` | http handles mTLS GET/POST/PUT + plain HTTP + LLM POST; shell handles SSH cert-fetch setup |
| 21 ToolSkills | One per distinct URL/method pattern — reads, writes, Leistungen, cert-fetch |
| 28 PythonCode executors + 8 pure-logic helpers | Executors call `__execute_action__`; helpers transform data without I/O |
| 37 leaf skills | One per distinct approach — REST reads + writes + Leistungen + composition + LLM + cert-fetch |
| 15 Tier-0 recipes | All known-ID read ops + Leistungen two-step path — deterministic, no LLM |
| 17 Tier-1 recipes | 1 search + 3 writes + 1 cert-fetch + 5 composition + 8 LLM service inference |
| `tomedo-patient-full-context` | Multi-step composed recipe; always chains the same 4 calls |
| Phone lookup via sidecar only | Confirmed: server-side phone search returns `{}` (non-functional) |
| German + English intent examples | Praxis staff speak German; orchestrator must handle both |
| Direct mTLS REST API supports GET + POST + PUT | Confirmed live 2026-08-22 — no partner agreement needed for writes |
| Leistungen via kvschein two-step only | `GET /leistung?patient=X` crashes server — must go patientenDetailsRelationen → kvschein |
| Briefkommandos = REST data model docs | Every `$[&p.x.y.z]$` maps to REST JSON path; `$[l ...]$` reveals kvschein safe path |
| `$[l %nr 0d ,]$` → kvschein path | Client-side Leistung kommando reveals the server-side safe read: patientenDetailsRelationen → GET /kvschein/{ident} → ebmLeistungen[] → ebmkatalogeintrag |
| EBM/GOÄ mode via Schein presence | `patInfoPanelMode` is transient/computed — detect EBM vs GOÄ by checking kvScheine vs goaeScheine in relations |
| Cert-fetch extension | One-time SSH setup fetches mTLS PEM bundle from server; required before any tomedo REST call |
| LLM object composition uses embedded context files | zollsoft provides `pythonmarker_context.txt` etc; BrassClaw embeds in skill bodies |
| tomedo LLM service = one ToolSkill, 8 PythonCode | Same endpoint for all LLM calls; prompt content is the differentiator |
| tomedo LLM service budget tracking | Monthly per-user limit enforced by zollsoft; budget errors always surfaced to user |

