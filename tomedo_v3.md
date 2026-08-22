# tomedo v3 — Extension Plan

> [!CAUTION]
> ## ⛔ LIVE PRODUCTION SYSTEM — READ THIS BEFORE WRITING ANYTHING
>
> This plan operates against a **live production tomedo server** with real patient data.
> Every POST and PUT is immediately visible to Mac clients in the practice.
> The following actions have each caused a server-wide client crash loop confirmed
> in production (2026-08-22), requiring `sudo systemctl restart tomedo-server` to recover:
>
> | Action | Consequence |
> |--------|-------------|
> | `POST /karteieintrag` with **any** of the 4 mandatory relation fields missing (`karteiEintragTyp`, `mediaTyp`, `dokumentierenderNutzer`, `betriebsstaette`) | Null ident in sync record → `JSON2CoreData.m:349` assert crash on **every connected Mac client**, crash loop until server restart |
> | `POST /karteieintrag` with a blank or partial body | Same crash loop — change record is permanent, `visible:false` does NOT stop the sync replay |
> | Including `letzterNutzer` in a **PUT** body (update) | Corrupts the sync record → same crash loop |
> | Calling `GET /leistung?patient=X`, `GET /patient/{id}/leistungen`, or `GET /schein?patient=X` | Unbounded DB query — crashes the server process entirely |
> | Any write without running all 3 steps of the karteieintrag pattern | Entry exists but client never sees it, or client crashes — see §karteieintrag-write-rule |
>
> **Recovery from crash loop:** `ssh technik@192.168.10.9` → `sudo systemctl restart tomedo-server`
>
> **Before any write:** confirm with the user, verify all 4 relation idents are non-null integers,
> and never use kürzel strings (e.g. `"ANM"`) — only numeric idents (e.g. `6`).
>
> **Test patient for all write tests:** ident `13550` — `Test, Toni`. Never write to real patients during development.


> **Purpose:** This document defines every v3 artifact required to integrate the
> tomedo EMR REST API into BrassClaw Reborn as a first-class extension.
> It follows the same orchestrator-first, LLM-minimal design as `builtin_stuff_v3.md`.
>
> **Scope:** Direct mTLS REST API only (port 8443). Basic patient data reads +
> karteieintrag writes. Cert-fetch setup. No crawl sidecar, no LLM service,
> no composition layer — those are separate plans.
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
> **The orchestrator IS the execution engine.** Rust makes tools available.
> The LLM is consulted ONLY when a task requires creative reasoning, composition,
> or an irreversible decision the user must confirm. Everything else is Tier 0.
>
> **The two-channel execution model (MANDATORY for every tomedo recipe):**
> ```
> channel: "rust"           → pre-loads the ToolSkill binding (does NOT execute — availability only)
> channel: "orchestrator"   → PythonCode calls __execute_action__() to ACTUALLY run the tool
> ```
> A Tier-0 recipe MUST have BOTH channels. A `rust`-only step with no matching
> `orchestrator` PythonCode executor is a Q1 hard error (§tier0-orchestrator-channel Rule 2).
>
> **Granularity rules (strictly enforced):**
> - One ToolSkill = one URL pattern + one HTTP method (not one "feature")
> - One PythonCode executor = exactly one `__execute_action__` call
> - Pure-logic PythonCode helpers = zero I/O, zero `__execute_action__` calls
> - Three narrowly-scoped skills that share the same Rust tool beat one monolithic skill
>
> **Tier rules:**
> - All GET reads with a known ID/date (deterministic inputs) → **Tier 0**
> - All POST/PUT writes → **Tier 1** (LLM confirms irreversible content before dispatch)
> - Cert-fetch (SSH with user credentials) → **Tier 1**
> - Name search (user-supplied string) → **Tier 1** (LLM URL-encodes the query)
> - Automated nightly audit, date arithmetic, JSON parsing → **Tier 0** (no LLM)
>
> **⚠️ SAFETY — never use open-ended collection endpoints:**
> `GET /leistung?patient=X`, `/patient/{id}/leistungen`, `/schein?patient=X`,
> `/patient/{id}/besuche` → all crash the server (unbounded queries, confirmed 2026-08-22).
> Always use `patientenDetailsRelationen` with limit params.
>
> **Auth guard (§tomedo-auth):**
> All tomedo REST calls require `tomedo_base_url` and `tomedo_cert_pem` to be set.
> Run `tomedo-cert-fetch` once before any other recipe.
>
> **Orchestrator call hierarchy:**
> ```
> Rust makes builtin.http available → ToolSkill binds it →
> PythonCode calls __execute_action__("tomedo-api", {...})
> ```
>
> ---
>
> ## API Reference Summary (confirmed by live probe 2026-08-22)
>
> **Test patient:** `ident=13550`, `Test Toni`, DOB 1989-12-31.
> **DB name:** `tomedo_live`
> **Cert files:** `/opt/data/apiConnector/ssl/` on the server → fetched via `tomedo-cert-fetch`.
>
> ### tomedo REST API (port 8443, mTLS) — READ endpoints
>
> | Method | Path | Status | Description |
> |--------|------|--------|-------------|
> | GET | `/{db}/serverstatus` | ✅ | Server health, version |
> | GET | `/{db}/patient/{id}` | ✅ | Full record: name, DOB, phone numbers |
> | GET | `/{db}/patient/{id}/patientenDetailsRelationen?limitScheine=true&limitKartei=50&limitVerordnungen=50&limitZeiterfassungen=true&limitBehandlungsfaelle=true` | ✅ safe | Diagnoses, Kartei, Behandlungsfälle — **always use limit params** |
> | GET | `/{db}/patient/{id}/patientenDetailsRelationen/medikamentenPlan` | ✅ safe | Medication plan |
> | GET | `/{db}/patient/{id}/termine?flach=true` | ✅ safe | Appointments flat array |
> | GET | `/{db}/besuch/{patient_id}/besucheForPatient` | ✅ safe | Visit records — **patient ident** (confirmed live) |
> | GET | `/{db}/kvschein/{schein_ident}` | ✅ safe | KV-Schein + ebmLeistungen[] (safe Leistung path) |
> | GET | `/{db}/ebmkatalogeintrag/{ident}` | ✅ safe | Resolve EBM catalog ident → Ziffer string |
> | GET | `/{db}/patient/searchByAttributes?query={name}` | ❌ BROKEN | Returns `{}` — name search only, Tier 1 |
> | GET | `/{db}/patient?flach=true` | 💀 BULK | ~15k records — never use interactively |
> | GET | `/{db}/leistung?patient={id}` | 💀 CRASH | Unbounded — crashes server |
> | GET | `/{db}/schein?patient={id}` | 💀 CRASH | Unbounded — crashes server |
> | GET | `/{db}/patient/{id}/leistungen` | 💀 CRASH | Unbounded — crashes server |
>
> **Leistungen safe path** (confirmed live 2026-08-22):
> ```
> 1. GET /{db}/patient/{id}/patientenDetailsRelationen?limitScheine=true → kvScheine[].ident
> 2. GET /{db}/kvschein/{schein_ident} → ebmLeistungen[], goaeLeistungen[]
> 3. GET /{db}/ebmkatalogeintrag/{catalog_ident} → {code: "03220", ...}
> ```
>
> **EBMLeistung fields:** `ident`, `datum` (epoch ms), `visible`, `anzahl`,
> `ebmKatalogEintrag.ident` (internal int — NOT the Ziffer string),
> `leistungserbringer.ident`, `betriebsstaette.ident`
>
> **Patient basic data fields:**
> | Field | REST JSON path |
> |-------|---------------|
> | Family name | `nachname` |
> | Given name | `vorname` |
> | DOB | `geburtsDatum` (epoch ms, may be negative) |
> | Main phone | `patientenDetails.kontaktdaten.telefon` |
> | Mobile | `patientenDetails.kontaktdaten.handyNummer` |
> | Secondary phone | `patientenDetails.kontaktdaten.telefon2` |
> | Tertiary phone | `patientenDetails.kontaktdaten.telefon3` |
> | Additional phones | `patientenDetails.kontaktdaten.weitereTelefonummern[]` |
> | Email | `patientenDetails.kontaktdaten.email` |
> | Street | `patientenDetails.kontaktdaten.adresse.strasse` |
> | Postcode | `patientenDetails.kontaktdaten.adresse.plz` |
> | City | `patientenDetails.kontaktdaten.adresse.ort` |
>
> **Diagnoses** (from `patientenDetailsRelationen.diagnosen[]`):
> `freitext` = human-readable text (primary field), `typ`: `G`=confirmed, `V`=suspected, null=use freitext only
>
> ### tomedo REST API (port 8443, mTLS) — WRITE endpoints (confirmed live 2026-08-22)
>
> **Critical finding: the direct mTLS REST API accepts writes.** The "read-only"
> assumption was wrong. All write endpoints return HTTP 200/204 with the same
> client cert used for reads. No partner agreement needed for writes.
>
> | Method | Path | Status | Returns | Notes |
> |--------|------|--------|---------|-------|
> | POST | `/{db}/karteieintrag` | ✅ HTTP 200 | `{new_ident}` | Step 1 of 3-step create — see §karteieintrag-write-rule |
> | PUT  | `/{db}/patient/{id}` | ✅ HTTP 204 | — | Step 2 of 3-step create (DB join row) + partial update; `gesperrt:1` blocks patient |
> | PUT  | `/{db}/patientendetailsrelationen/{id}` | ✅ HTTP 204 | — | Step 3 of 3-step create — writes `PatientenDetailsRelationen` change record → Mac client picks up entry |
> | POST | `/{db}/termin` | ✅ HTTP 200 | `{new_ident}` | Creates Termin; `removed:true` via PUT to cancel |
> | POST | `/{db}/patient` | ✅ HTTP 200 | `{new_ident}` | Creates patient; `gesperrt:1` (integer!) via PUT to block |
> | PUT  | `/{db}/karteieintrag/{id}` | ✅ HTTP 204 | — | Partial update; `visible:false` hides entry |
> | PUT  | `/{db}/termin/{id}` | ✅ HTTP 204 | — | Partial update; `removed:true` cancels |
> | POST | `/{db}/ebmleistung` | ✅ HTTP 200 | `{new_ident}` | Create EBMLeistung — **use `/ebmleistung` NOT `/leistung`** (dtype, ebmKatalogEintrag stored correctly only via this endpoint — confirmed 2026-08-22) |
> | PUT  | `/{db}/kvschein/{id}` | ✅ HTTP 204 | — | Step 2: link EBMLeistung to Schein — `{"ident":<schein>,"ebmLeistungen":[{"ident":<leistung>}]}` |
> | DELETE | `/{db}/karteieintrag/{id}` | ❌ HTTP 405 | — | Not supported — use PUT `visible:false` |
> | DELETE | `/{db}/termin/{id}` | ❌ HTTP 405 | — | Not supported — use PUT `removed:true` |
>
> **Key type facts learned from live probe:**
> - `gesperrt` is **Integer** (not boolean) — send `1` not `true`
> - `removed` on Termin is **Boolean** — send `true`
> - `visible` on KarteiEintrag is **Boolean** — send `false` to hide
> - Error responses use HTTP 460 with full Java stack trace as body
> - **🚫 BLANK POST IS A PRODUCTION INCIDENT — CATEGORICALLY FORBIDDEN:**
>   Any `POST /karteieintrag` with a missing or incomplete payload (blank body, or any POST
>   omitting `karteiEintragTyp`, `mediaTyp`, `dokumentierenderNutzer`, `betriebsstaette`)
>   creates a DB entry AND a sync change-log record. The change-log record is permanent —
>   `visible:false` does NOT remove it from the sync queue. The `ZSTransferFetchedDataThread`
>   picks up the bare entry within seconds and crashes every connected client in a loop.
>   **Blank POST + immediate PUT is equally forbidden** — the client picks up the blank state
>   before the PUT completes, regardless of speed (confirmed 2026-08-22).
>   **The only recovery is `sudo systemctl restart tomedo-server` via SSH.**
>   Incidents: `192566688419938304`, `192569545098526720`, `192570559489900544` — three
>   server restarts caused by this pattern.
>
> - **🚫 PARTIAL ATOMIC POST IS ALSO A CRASH (discovered via client error 2026-08-22+):**
>   An atomic POST that omits **any** of the four mandatory relation fields
>   (`karteiEintragTyp`, `mediaTyp`, `dokumentierenderNutzer`, `betriebsstaette`) causes
>   the server to store the entry with those relations as **null**. When
>   `ZSTransferFetchedDataThread` syncs the entry to Mac clients, `JSON2CoreData` calls
>   `transferJSONDictionary:ofType:toEntity:` on each relation slot and asserts
>   `ident != NULL` (JSON2CoreData.m:349). A null ident triggers this assert and
>   crashes every connected client.
>   **Crash signature:** `ERROR: Assertion failed: ident != ((void*)0)` in
>   `JSON2CoreData.m:349` / `ZSTransferFetchedDataThread`.
>   The earlier "atomic POST is safe" conclusion (2026-08-22) was premature — those
>   test entries omitted `mediaTyp`, `dokumentierenderNutzer`, `betriebsstaette` and
>   caused this exact crash. The only difference from blank POST is the trigger is
>   deferred by milliseconds (the sync scan happens on the next `ZSTransferFetchedDataThread`
>   tick), making it appear "safe" until client logs are checked.
>   **Recovery (temporary):** `sudo systemctl restart tomedo-server` via SSH — clears the
>   in-memory sync queue but does NOT remove bad rows from the `change` table. Clients that
>   reconnect fresh will crash again until the DB is cleaned.
>   **Recovery (permanent):** Direct SQL DELETE on the `change` table, the `karteieintrag`
>   table, and the `patientendetailsrelationen_karteieintraege` join table.
>   The `change` table is **append-only** — `PUT /patientendetailsrelationen` only appends
>   new rows, it cannot overwrite or remove existing ones. There is no REST endpoint for
>   deleting `change` rows (DELETE /change → HTTP 404; GET /change → HTTP 410 Gone).
>   ```sql
>   -- Run in this order (FK constraints: join table first, then entries, then change)
>   DELETE FROM patientendetailsrelationen_karteieintraege
>     WHERE karteieintraege_ident IN (<bad_idents>);
>   DELETE FROM karteieintrag
>     WHERE ident IN (<bad_idents>);
>   DELETE FROM change
>     WHERE clientid IS NULL AND changedate IS NULL
>     AND revision IN (<revision_numbers>);
>   ```
>   Identify revision numbers via:
>   `SELECT revision, entitytype, entityid, value FROM change WHERE clientid IS NULL AND changedate IS NULL ORDER BY revision;`
>
> - **✅ PATIENT LINK SOLVED — THREE-STEP (confirmed live 2026-08-22+):**
>   The patient association is a CoreData join-table relationship managed server-side.
>   It is **never in the KarteiEintrag JSON** — neither in GET responses nor POST/PUT bodies.
>   The `"patient": {"ident": N}` body field is silently dropped by `deepMerge`.
>   `?patient=N` and `?patientenId=N` query params also have no effect.
>   Two distinct server-side mechanisms must both be triggered:
>   - The **DB join table** (`patientendetailsrelationen_karteieintraege`) — written by `PUT /patient/{id}`
>   - The **sync change record** (`change` table, `entitytype="PatientenDetailsRelationen"`) — written by
>     `PUT /patientendetailsrelationen/{patient_id}` — this is what the Mac client's
>     `ZSTransferFetchedDataThread` reads to update its local CoreData kartei list
>   Without step 3, the entry exists on the server and in the DB join table, but the Mac client
>   never shows it because no `PatientenDetailsRelationen` change record exists in the sync queue.
>
>   **Correct three-step write:**
>   1. `POST /{db}/karteieintrag` with all 4 mandatory relation fields → returns `{new_ident}`
>   2. `PUT /{db}/patient/{patient_id}` → `{"patientenDetails":{"patientenDetailsRelationen":{"karteiEintraege":[{"ident":N}]}}}` → HTTP 204 (writes DB join row)
>   3. `PUT /{db}/patientendetailsrelationen/{patient_id}` → `{"ident": patient_id, "karteiEintraege": [{"ident": N}]}` → HTTP 204 (writes sync change record)
>   After step 3, entry appears in Mac client kartei immediately via `ZSTransferFetchedDataThread`.
>   Confirmed live on patient 13550, entry `192572189961617408` (STG type), 2026-08-22.
>
> **§karteieintrag-write-rule — CONFIRMED WORKING (three-step):**
> - **Blank POST** → client crash loop + server restart required — FORBIDDEN
> - **Partial atomic POST** (any of 4 relation fields missing) → null ident sync crash — FORBIDDEN
> - **Full atomic POST alone** → entry created, no crash, but NOT linked to patient — INCOMPLETE
> - **Full atomic POST + PUT /patient only** → DB join row written, but Mac client never shows it — INCOMPLETE
> - **✅ Full atomic POST + PUT /patient + PUT /patientendetailsrelationen** → entry created, patient-linked, visible in Mac client
>
> **If a crash loop starts:** `sudo systemctl restart tomedo-server` via SSH.
>
> **Known orphaned entries (all `visible:false`, do not modify or re-use):**
> | ident | problem |
> |-------|---------|
> | `192566688419938304` | blank POST, REST 404, crash loop — restarted |
> | `192569545098526720` | blank POST + PUT, not patient-linked |
> | `192569614971437056` | partial atomic POST (missing mediaTyp/nutzer/betriebsstaette) → null-ident sync crash — crash loop |
> | `192570321189470208` | partial atomic POST (missing mediaTyp/nutzer/betriebsstaette) → null-ident sync crash — crash loop |
> | `192570474777542656` | partial atomic POST `?patient=` param test → null-ident sync crash — crash loop |
> | `192570559489900544` | blank POST + PUT, crash loop — restarted |
> | `192571190832267264` | full POST + `?patient=13550` query param — not linked, soft-deleted |
> | `192571191184588800` | full POST + `?patientenId=13550` query param — not linked, soft-deleted |
> | `192571574431776768` | ✅ **link test success** — full POST + PUT patient link — linked, soft-deleted |
>
> **✅ Confirmed three-step write pattern (patient 13550, entry `192572189961617408`, 2026-08-22):**
> ```
> Step 1 — POST /{db}/karteieintrag
> ```
> ```json
> {
>   "datum":                 <epoch_ms>,
>   "text":                  "...",
>   "visible":               true,
>   "primaer":               false,
>   "letzterNutzer":         {"ident": <nutzer_ident>},   ← set same as dokumentierenderNutzer
>   "karteiEintragTyp":      {"ident": N},         ← MANDATORY — null crashes clients
>   "mediaTyp":              {"ident": 1},          ← MANDATORY — null crashes clients
>   "dokumentierenderNutzer":{"ident": N},          ← MANDATORY — null crashes clients
>   "betriebsstaette":       {"ident": 1}           ← MANDATORY — null crashes clients
> }
> ```
> → HTTP 200, returns `{new_ident}`
> ```
> Step 2 — PUT /{db}/patient/{patient_id}
> ```
> ```json
> {"patientenDetails": {"patientenDetailsRelationen": {"karteiEintraege": [{"ident": <new_ident>}]}}}
> ```
> → HTTP 204. Writes DB join row in `patientendetailsrelationen_karteieintraege`.
> ```
> Step 3 — PUT /{db}/patientendetailsrelationen/{patient_id}
> ```
> ```json
> {"ident": <patient_id>, "karteiEintraege": [{"ident": <new_ident>}]}
> ```
> → HTTP 204. Writes `PatientenDetailsRelationen` change record to `change` table.
> Mac client's `ZSTransferFetchedDataThread` picks this up and adds entry to local CoreData kartei list.
> Entry is immediately visible in all Mac clients.
>
> **Why step 3 is different from step 2:**
> `PUT /patient/{id}` writes entity type `"Patient"` to `change` — Mac client ignores this for kartei display.
> `PUT /patientendetailsrelationen/{id}` writes entity type `"PatientenDetailsRelationen"` — exactly what
> `ZSTransferFetchedDataThread` watches for to update the local kartei list.
> Confirmed by direct comparison of `change` table records for manual Mac entry vs API entry.
>
> **Note on `letzterNutzer`:** Include in step 1 POST body (same ident as `dokumentierenderNutzer`).
> The Mac client sets this automatically. Without it, the entry may not display in some kartei views.
> **DO NOT PUT `letzterNutzer` after creation** — it is a read-only field via PUT and will corrupt the
> sync record, causing `JSON2CoreData.m:349` crash loop. Set it only in the initial POST.
>
> **Note on `betriebsstaette`:** Use `{"ident": 1}` (the practice default) for new entries.
> The reference entry `3007099` uses ident 2 (a second Betriebsstätte) — match to the
> `betriebsstaette` of the current Betriebsstätte context if known, otherwise use 1.
>
> Reference entry (real, user-created ANM, patient-linked): `3007099` — `betriebsstaette: 2`.
>
> **KarteiEintragTyp ident reference** (confirmed from live data, this server):
> | ident | kürzel | Beschreibung |
> |-------|--------|-------------|
> | 2 | BEF | Befund |
> | 3 | DDI | Dauerdiagnose |
> | 4 | DIA | Diagnose (Akutdiagnose) |
> | 6 | ANM | Anmerkung (plain text note) |
> | 8 | MED | Medikament |
> | 9 | BILD | Bildaufnahme |
> | 18 | BES | Besuch (auto-created, do not POST manually) |
> | 20 | LAB | Labor |
> | 26 | BMI | BMI/Vitalwerte |
> | 29 | MAR | Marcumar |
> | 58 | MEP | Medikamentenplan |
> | 90 | REC | Rezept |
>
> **mediaTyp ident reference** (confirmed from live data):
> | ident | Beschreibung |
> |-------|-------------|
> | 1 | Text (plain text) |
> | 2 | Anhang/Bild |
> | 3 | Diagnose |
> | 6 | Labor |
> | 16 | BMI/Vitalwerte |
>
> **Write endpoint field schemas** (from GET responses on live objects):
> ```
> KarteiEintrag POST fields: patient{ident}, datum(epoch ms), text, visible(bool),
>   primaer(bool), karteiEintragTyp{ident}, mediaTyp{ident},
>   dokumentierenderNutzer{ident}, betriebsstaette{ident},
>   additionalText, status, diagnose{ident}, anhang[]
> KarteiEintrag PUT fields: same as POST minus patient (patient is POST-only)
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
> ## §write-paths — Write Paths to tomedo (Status as of 2026-08-22)
>
> **Direct mTLS REST API (port 8443) — ACTIVE, no partner agreement needed.**
>
> | Method | Path | Returns | Notes |
> |--------|------|---------|-------|
> | POST | `/{db}/karteieintrag` | `{new_ident}` | Three-step create — see §karteieintrag-write-rule |
> | PUT  | `/{db}/patient/{id}` | HTTP 204 | Step 2 (DB join row) + partial update |
> | PUT  | `/{db}/patientendetailsrelationen/{id}` | HTTP 204 | Step 3 (sync change record → Mac client) |
> | POST | `/{db}/termin` | `{new_ident}` | Create appointment |
> | PUT  | `/{db}/karteieintrag/{id}` | HTTP 204 | Update; `visible:false` = soft-delete |
> | PUT  | `/{db}/termin/{id}` | HTTP 204 | Update; `removed:true` = cancel |
>
> **Type facts:** `gesperrt`=Integer(1), `removed`=Boolean, `visible`=Boolean.
> Error responses: HTTP 460 + Java stack trace. DELETE not supported (HTTP 405).
>
> **§karteieintrag-write-rule — THREE-STEP (confirmed live 2026-08-22):**
> All four relation fields + `letzterNutzer` are MANDATORY in the POST body.
> Omitting any one → null-ident in sync record → `JSON2CoreData.m:349` crash loop on every Mac client.
> - Step 1: `POST /karteieintrag` with all 5 fields → `{new_ident}`
> - Step 2: `PUT /patient/{patient_id}` `{patientenDetails:{patientenDetailsRelationen:{karteiEintraege:[{ident:N}]}}}` → HTTP 204 (DB join row only — Mac client NOT notified)
> - Step 3: `PUT /patientendetailsrelationen/{patient_id}` `{ident:patient_id, karteiEintraege:[{ident:N}]}` → HTTP 204 (writes `PatientenDetailsRelationen` change record — Mac client shows entry immediately)
>
> If crash loop starts: `sudo systemctl restart tomedo-server` via SSH.
>
> **KarteiEintragTyp idents** (confirmed, this server):
> `2=BEF, 3=DDI, 4=DIA, 6=ANM, 8=MED, 9=BILD, 18=BES, 20=LAB, 50=STG`
> **mediaTyp idents:** `1=Text, 2=Anhang, 3=Diagnose, 6=Labor, 16=BMI`
>
> ---


## Step 1 — Tool Rows (class 0)

Two tools: `tomedo-api` for all REST API calls, `tomedo-cert-fetch-tool` for
the one-time SSH cert setup. Both are extension-level class 0 declarations.

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

## Step 2 — ToolSkills (class 13)

One ToolSkill per distinct call pattern. Each binds exactly one tool.
The cert-fetch ToolSkill (`ts-tomedo-cert-fetch`) is defined in Step 6
alongside its ExtensionCatalogue.

---

### Step 2.1 — ToolSkill: `ts-tomedo-serverstatus` (class 13)

```
name:          "ts-tomedo-serverstatus"
tool_name:     "tomedo-api"
description:   "GET /{db}/serverstatus. Checks if the tomedo server is reachable and
                returns its software version and revision.
                Response: {status:'OK', softwareVersion:null, revision:N}
                softwareVersion is always null on this server (confirmed live).
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

### Step 2.2 — ToolSkill: `ts-tomedo-patient-detail` (class 13)

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
description:   "GET /{db}/besuch/{patient_id}/besucheForPatient.
                Returns visit/consultation records for a patient.
                ✅ The URL segment IS the patient ident (confirmed live: /besuch/13550/besucheForPatient
                returned 7 records). No prior besuch_ident lookup needed.
                Use timeout_ms: 15000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/besuch/{patient_id}/besucheForPatient"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url": "{{vars.tomedo_base_url}}/besuch/{{vars.patient_id}}/besucheForPatient",
  "method": "GET",
  "timeout_ms": 15000
}
preconditions:  "patient_id must be a valid patient ident. No besuch_ident lookup required."
error_handling: "HTTP 404 → patient_id not found. Empty array → no visit records."
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

### Step 2.9 — ToolSkill: `ts-tomedo-karteieintrag-create` (class 13)

```
name:          "ts-tomedo-karteieintrag-create"
tool_name:     "tomedo-api"
description:   "Step 1 of 3: POST /{db}/karteieintrag — create a new KarteiEintrag entry.
                ALL FOUR relation fields + letzterNutzer are mandatory in the body.
                Omitting any relation field causes null-ident sync crash (JSON2CoreData.m:349).
                Use ident-based references only (NOT kuerzel — not resolved server-side).
                Steps 2+3 (patient link + sync) handled by ts-tomedo-patient-link-karteieintrag
                and ts-tomedo-patientendetailsrelationen-link.
                Request body (JSON) — no patient field (deepMerge drops it):
                {
                  text:                  'string',
                  datum:                 epoch_ms,
                  visible:               true,
                  primaer:               false,
                  letzterNutzer:         {ident: N},  ← same as dokumentierenderNutzer
                  karteiEintragTyp:      {ident: N},  ← 50=STG, 6=ANM, 2=BEF, 4=DIA, 20=LAB
                  mediaTyp:              {ident: 1},  ← 1=Text (mandatory)
                  dokumentierenderNutzer:{ident: N},  ← Nutzer ident (mandatory)
                  betriebsstaette:       {ident: 1}   ← practice default (mandatory)
                }
                Returns: {new_ident} — pass to steps 2+3 for patient linking.
                Confirmed HTTP 200 on live tomedo 2026-08-22."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/karteieintrag"},
  {name: "method",  param_type: "string", required: true, description: "POST"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON with all 4 mandatory relation fields. No patient field."},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":    "{{vars.tomedo_base_url}}/karteieintrag",
  "method": "POST",
  "headers": {"Content-Type": "application/json"},
  "body":   "{\"datum\":{{vars.datum}},\"text\":\"{{vars.text}}\",\"visible\":true,\"primaer\":false,\"letzterNutzer\":{\"ident\":{{vars.nutzer_ident}}},\"karteiEintragTyp\":{\"ident\":{{vars.kartei_typ_ident}}},\"mediaTyp\":{\"ident\":{{vars.media_typ_ident}}},\"dokumentierenderNutzer\":{\"ident\":{{vars.nutzer_ident}}},\"betriebsstaette\":{\"ident\":{{vars.betriebsstaette_ident}}}}"
}
preconditions:  "All five fields required. kartei_typ_ident e.g.=50(STG), media_typ_ident=1(Text), nutzer_ident=logged-in doctor ident, betriebsstaette_ident=1."
error_handling: "HTTP 460: tomedo rejected the body. If ANY relation field is null/missing: client crash loop — restart server immediately via SSH."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.16b — ToolSkill: `ts-tomedo-patient-link-karteieintrag` (class 13)

```
name:          "ts-tomedo-patient-link-karteieintrag"
tool_name:     "tomedo-api"
description:   "Step 2 of 3: PUT /{db}/patient/{patient_id} — writes the DB join row linking
                a KarteiEintrag to a patient in patientendetailsrelationen_karteieintraege.
                Call AFTER ts-tomedo-karteieintrag-create succeeds and you have new_ident.
                NOTE: this step alone is NOT sufficient for Mac client display — step 3
                (ts-tomedo-patientendetailsrelationen-link) must also be called.
                Body: {patientenDetails: {patientenDetailsRelationen: {karteiEintraege: [{ident: N}]}}}
                Returns: HTTP 204 No Content on success.
                Confirmed HTTP 204 on live tomedo (patient 13550, 2026-08-22)."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/patient/{patient_id}"},
  {name: "method",  param_type: "string", required: true, description: "PUT"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON: {patientenDetails:{patientenDetailsRelationen:{karteiEintraege:[{ident:N}]}}}"},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":    "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}",
  "method": "PUT",
  "headers": {"Content-Type": "application/json"},
  "body":   "{\"patientenDetails\":{\"patientenDetailsRelationen\":{\"karteiEintraege\":[{\"ident\":{{vars.new_karteieintrag_ident}}}]}}}"
}
preconditions:  "new_karteieintrag_ident must be the ident returned from ts-tomedo-karteieintrag-create. patient_id must be a valid patient ident."
error_handling: "HTTP 460: patient_id or new_ident invalid. HTTP 204 = success (no body returned)."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.16c — ToolSkill: `ts-tomedo-patientendetailsrelationen-link` (class 13)

```
name:          "ts-tomedo-patientendetailsrelationen-link"
tool_name:     "tomedo-api"
description:   "Step 3 of 3: PUT /{db}/patientendetailsrelationen/{patient_id} — writes the
                PatientenDetailsRelationen change record to the change table, which triggers
                ZSTransferFetchedDataThread on Mac clients to add the entry to their local
                CoreData kartei list. Without this step the entry exists on the server and in
                the DB join table but is NEVER shown in the Mac client kartei view.
                Body: {ident: patient_id, karteiEintraege: [{ident: new_ident}]}
                Returns: HTTP 204 No Content on success.
                Confirmed live: patient 13550, entry 192572189961617408, 2026-08-22."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/patientendetailsrelationen/{patient_id}"},
  {name: "method",  param_type: "string", required: true, description: "PUT"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON: {ident: patient_id, karteiEintraege: [{ident: new_karteieintrag_ident}]}"},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":    "{{vars.tomedo_base_url}}/patientendetailsrelationen/{{vars.patient_id}}",
  "method": "PUT",
  "headers": {"Content-Type": "application/json"},
  "body":   "{\"ident\":{{vars.patient_id}},\"karteiEintraege\":[{\"ident\":{{vars.new_karteieintrag_ident}}}]}"
}
preconditions:  "new_karteieintrag_ident must be from step 1. patient_id must be valid. Call after step 2."
error_handling: "HTTP 460: invalid ident. HTTP 204 = success. Mac client will display entry on next sync tick."
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
                ident to its human-readable EBM Ziffer string (e.g. 298 → '03230').
                Use after fetching ebmLeistungen from /kvschein to map internal ints
                to billing code strings.
                Response: {ident:N, code:'03230', kurztext:'...', ...}
                ⚠️ The field is 'code', NOT 'nummer' (confirmed live: ident 298 → code '03230')."
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

---

### Step 2.21 — ToolSkill: `ts-tomedo-termin-update` (class 13)

```
name:          "ts-tomedo-termin-update"
tool_name:     "tomedo-api"
description:   "PUT /{db}/termin/{ident} — partial update of an existing Termin.
                Most common use: cancel with {removed: true}.
                Can also reschedule: {beginn: epoch_ms, ende: epoch_ms}.
                Or update info text: {info: 'new reason'}.
                removed is Boolean (true/false). DELETE is not supported (HTTP 405).
                Returns: HTTP 204 No Content on success."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/termin/{termin_ident}"},
  {name: "method",  param_type: "string", required: true, description: "PUT"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON partial update, e.g. {removed: true} or {beginn: epoch_ms, ende: epoch_ms}"},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":    "{{vars.tomedo_base_url}}/termin/{{vars.termin_ident}}",
  "method": "PUT",
  "headers": {"Content-Type": "application/json"},
  "body":   "{{vars.body_json}}"
}
preconditions:  "termin_ident must be an existing Termin ident."
error_handling: "HTTP 460: field type mismatch. HTTP 204 = success. HTTP 405 = tried DELETE — not supported."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.22 — ToolSkill: `ts-tomedo-ebmleistung-create` (class 13)

```
name:          "ts-tomedo-ebmleistung-create"
tool_name:     "tomedo-api"
description:   "POST /{db}/ebmleistung — create a new EBMLeistung (billing code entry) on a KV-Schein.
                ⚠️ Use /ebmleistung NOT /leistung — the /leistung endpoint stores dtype='Leistung'
                and drops ebmKatalogEintrag. Only /ebmleistung stores dtype='EBMLeistung' correctly
                (confirmed live 2026-08-22).
                Mandatory fields: datum(epoch ms), visible(true), anzahl(1),
                ebmKatalogEintrag{ident}, leistungserbringer{ident}, betriebsstaette{ident},
                dokumentierenderNutzer{ident}, letzterNutzer{ident}, abrechnenderArzt{ident}.
                Returns HTTP 200 with {new_leistung_ident}.
                Step 2: PUT /kvschein/{schein_ident} with {ident, ebmLeistungen:[{ident:N}]} → HTTP 204."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/ebmleistung"},
  {name: "method",  param_type: "string", required: true, description: "POST"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON with all mandatory fields"},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":     "{{vars.tomedo_base_url}}/ebmleistung",
  "method":  "POST",
  "headers": {"Content-Type": "application/json"},
  "body":    "{{vars.body_json}}"
}
preconditions:  "ebmKatalogEintrag.ident must be from ebmkatalogeintrag table. All 5 relation idents must be valid."
error_handling: "HTTP 460: missing or invalid field. HTTP 200 + {ident} = success."
category:       "tomedo"
source:         "system"
validation_status: "validated"
```

---

### Step 2.23 — ToolSkill: `ts-tomedo-kvschein-link-leistung` (class 13)

```
name:          "ts-tomedo-kvschein-link-leistung"
tool_name:     "tomedo-api"
description:   "PUT /{db}/kvschein/{schein_ident} — link a newly created EBMLeistung to a KV-Schein.
                Must be called as step 2 immediately after POST /ebmleistung.
                Body: {ident:<schein_ident>, ebmLeistungen:[{ident:<leistung_ident>}]}.
                Sets invkvschein_ident on the leistung row and writes KVSchein change record
                so Mac clients pick up the new Leistung immediately.
                Returns HTTP 204 on success."
param_schema:  [
  {name: "url",     param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/kvschein/{schein_ident}"},
  {name: "method",  param_type: "string", required: true, description: "PUT"},
  {name: "body",    param_type: "string", required: true,
   description: "JSON: {ident:<schein_ident>, ebmLeistungen:[{ident:<leistung_ident>}]}"},
  {name: "headers", param_type: "object", required: true,
   description: "Must include Content-Type: application/json"}
]
param_template: {
  "url":     "{{vars.tomedo_base_url}}/kvschein/{{vars.schein_ident}}",
  "method":  "PUT",
  "headers": {"Content-Type": "application/json"},
  "body":    "{\"ident\":{{vars.schein_ident}},\"ebmLeistungen\":[{\"ident\":{{vars.leistung_ident}}}]}"
}
preconditions:  "leistung_ident must be a freshly created EBMLeistung ident. schein_ident must be a visible KVSchein."
error_handling: "HTTP 460 'No object with ID X found for class EBMLeistung': leistung_ident wrong or used /leistung instead of /ebmleistung for POST. HTTP 204 = success."
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

### Step 3.2 — PythonCode: `pc-tomedo-patient-detail` (class 22)

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
# ⚠️ Response body may contain embedded control characters (\x00-\x08 etc.) —
# strip them before json.loads() or parsing will fail (confirmed live 2026-08-22).
import re as _re
_raw = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}/patientenDetailsRelationen?limitScheine=true&limitKartei=50&limitVerordnungen=50&limitZeiterfassungen=true&limitBehandlungsfaelle=true",
    "method": "GET",
    "timeout_ms": 15000
})
# Strip control characters from body if present (preserves \t \n \r)
if isinstance(_raw, dict) and "body" in _raw:
    _raw["body"] = _re.sub(r'[\x00-\x08\x0b\x0c\x0e-\x1f]', '', _raw["body"])
result = _raw
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
# Dispatches ts-tomedo-patient-visits using patient_id directly.
# ✅ /besuch/{patient_id}/besucheForPatient takes patient ident — NO prior besuch lookup needed.
# Confirmed live: /besuch/13550/besucheForPatient → 7 visit records (2026-08-22).
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/besuch/{{vars.patient_id}}/besucheForPatient",
    "method": "GET",
    "timeout_ms": 15000
})
```

---

### Step 3.8 — Pure-logic PythonCode: `pc-tomedo-parse-diagnosen` (class 22)

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

### Step 3.12 — Pure-logic PythonCode: `pc-tomedo-extract-phone-fields` (class 22)

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

### Step 3.9 — PythonCode: `pc-tomedo-karteieintrag-create` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1 (LLM confirms content before call)
# Step 1 of 3-step karteieintrag create. Creates the entry; does NOT link to patient.
# Step 2 (DB join row) → pc-tomedo-karteieintrag-link
# Step 3 (sync change record) → pc-tomedo-patientendetailsrelationen-link
#
# CRITICAL CRASH RULE (JSON2CoreData.m:349 / ZSTransferFetchedDataThread):
# ALL FOUR relation fields MUST be present with valid idents.
# If ANY of karteiEintragTyp, mediaTyp, dokumentierenderNutzer, betriebsstaette
# is null or absent → null-ident on sync → assert crash on every Mac client.
# Use ident-based references (NOT kuerzel — not resolved server-side).
# letzterNutzer MUST be set in this POST (same ident as dokumentierenderNutzer).
# DO NOT PUT letzterNutzer after creation — read-only via PUT, corrupts sync record.
#
# Confirmed idents (this server): karteiEintragTyp 6=ANM, mediaTyp 1=Text,
# dokumentierenderNutzer=39205411185754113, betriebsstaette=1 (practice default).
# IBS bakes in vars before execution.
import json as _j
_base = "{{vars.tomedo_base_url}}"
if not _base:
    result = {"error": "tomedo_base_url not configured"}
else:
    _body = _j.dumps({
        "datum":                 int("{{vars.datum}}"),
        "text":                  "{{vars.text}}",
        "visible":               True,
        "primaer":               False,
        "letzterNutzer":         {"ident": int("{{vars.nutzer_ident}}")},
        "karteiEintragTyp":      {"ident": int("{{vars.kartei_typ_ident}}")},
        "mediaTyp":              {"ident": int("{{vars.media_typ_ident}}")},
        "dokumentierenderNutzer":{"ident": int("{{vars.nutzer_ident}}")},
        "betriebsstaette":       {"ident": int("{{vars.betriebsstaette_ident}}")}
    })
    result = __execute_action__("tomedo-api", {
        "url": f"{_base}/karteieintrag",
        "method": "POST",
        "headers": {"Content-Type": "application/json"},
        "body": _body,
        "timeout_ms": 15000
    })
# result contains {new_ident} — pass to pc-tomedo-karteieintrag-link (step 2)
```

---

### Step 3.31b — PythonCode: `pc-tomedo-karteieintrag-link` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Step 2 of 3-step karteieintrag create.
# Writes the DB join row in patientendetailsrelationen_karteieintraege via
# PUT /patient/{id}. This step does NOT trigger Mac client sync — step 3 does.
# Confirmed live: patient 13550, entry 192572189961617408 → HTTP 204.
# IBS bakes in vars before execution.
import json as _j
_base = "{{vars.tomedo_base_url}}"
_patient = "{{vars.patient_id}}"
_entry   = "{{vars.new_karteieintrag_ident}}"
if not _base or not _patient or not _entry:
    result = {"error": "tomedo_base_url, patient_id, or new_karteieintrag_ident not configured"}
else:
    _body = _j.dumps({
        "patientenDetails": {
            "patientenDetailsRelationen": {
                "karteiEintraege": [{"ident": int(_entry)}]
            }
        }
    })
    result = __execute_action__("tomedo-api", {
        "url": f"{_base}/patient/{_patient}",
        "method": "PUT",
        "headers": {"Content-Type": "application/json"},
        "body": _body,
        "timeout_ms": 15000
    })
# HTTP 204 = success (DB join row written). Must follow with step 3 to make
# entry visible in Mac client (pc-tomedo-patientendetailsrelationen-link).
```

---

### Step 3.31c — PythonCode: `pc-tomedo-patientendetailsrelationen-link` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1
# Step 3 of 3-step karteieintrag create (THE CRITICAL STEP for Mac client visibility).
# Writes a PatientenDetailsRelationen change record to the change table via
# PUT /patientendetailsrelationen/{patient_id}.
# ZSTransferFetchedDataThread on Mac clients watches for entitytype="PatientenDetailsRelationen"
# in the change table — this is the ONLY mechanism that triggers the kartei list update.
#
# CRASH RULE: The body MUST contain a valid integer patient ident and a valid integer
# karteieintrag ident. A null or missing ident in either field causes
# JSON2CoreData.m:349 "ident != NULL" assert crash on every connected Mac client.
#
# Confirmed live: patient 13550, entry 192572189961617408 → HTTP 204 → entry
# immediately visible in Mac client kartei view. (2026-08-22)
# IBS bakes in vars before execution.
import json as _j
_base    = "{{vars.tomedo_base_url}}"
_patient = "{{vars.patient_id}}"
_entry   = "{{vars.new_karteieintrag_ident}}"
if not _base or not _patient or not _entry:
    result = {"error": "tomedo_base_url, patient_id, or new_karteieintrag_ident not configured"}
else:
    _body = _j.dumps({
        "ident": int(_patient),
        "karteiEintraege": [{"ident": int(_entry)}]
    })
    result = __execute_action__("tomedo-api", {
        "url": f"{_base}/patientendetailsrelationen/{_patient}",
        "method": "PUT",
        "headers": {"Content-Type": "application/json"},
        "body": _body,
        "timeout_ms": 15000
    })
# HTTP 204 = success. Entry is now visible in Mac client kartei view immediately.
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

### Step 3.33b — PythonCode: `pc-tomedo-termin-update` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1 (LLM or orchestrator confirms before update)
# Partial-updates an existing Termin. Most common: {removed: true} to cancel.
# vars.body_json must be a valid JSON partial-update string.
import json as _j
_base = "{{vars.tomedo_base_url}}"
_ident = "{{vars.termin_ident}}"
if not _base or not _ident:
    result = {"error": "tomedo_base_url or termin_ident not configured"}
else:
    result = __execute_action__("tomedo-api", {
        "url": f"{_base}/termin/{_ident}",
        "method": "PUT",
        "headers": {"Content-Type": "application/json"},
        "body": "{{vars.body_json}}",
        "timeout_ms": 15000
    })
```

---

### Step 3.10 — PythonCode: `pc-tomedo-patient-search` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 1 (LLM composes query string)
# Executes patient name search. query must be URL-encoded.
# NOTE: searchByAttributes returns {} (empty dict) on no match — not an empty array.
# NOTE: This endpoint is known to return {} on some server configurations.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/patient/searchByAttributes?query={{vars.query}}",
    "method": "GET",
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


---

### Step 3.36 — PythonCode: `pc-tomedo-ebmleistung-create` (class 22)

```
# Channel: orchestrator | Class: 22
# POST /ebmleistung — create new EBMLeistung, return new ident.
# IBS bakes in {{vars.body_json}} before execution.
result = __execute_action__("tomedo-api", {
    "url":     "{{vars.tomedo_base_url}}/ebmleistung",
    "method":  "POST",
    "headers": {"Content-Type": "application/json"},
    "body":    "{{vars.body_json}}"
})
```

---

### Step 3.37 — PythonCode: `pc-tomedo-kvschein-link-leistung` (class 22)

```
# Channel: orchestrator | Class: 22
# PUT /kvschein/{schein_ident} — link EBMLeistung to KV-Schein, writes sync change record.
# IBS bakes in {{vars.schein_ident}} and {{vars.leistung_ident}} before execution.
result = __execute_action__("tomedo-api", {
    "url":     "{{vars.tomedo_base_url}}/kvschein/{{vars.schein_ident}}",
    "method":  "PUT",
    "headers": {"Content-Type": "application/json"},
    "body":    "{\"ident\":{{vars.schein_ident}},\"ebmLeistungen\":[{\"ident\":{{vars.leistung_ident}}}]}"
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
  A successful response returns {status:'OK', softwareVersion:null, revision:N}.
  Note: softwareVersion is always null on this server.
  A non-200 or connection error means the server is offline or the cert is invalid.
  This is a Tier-0 health check — no LLM required.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.2 — Leaf Skill: `skill-tomedo-patient-detail` (class 1)

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
  ✅ The URL uses the PATIENT ident directly — no prior besuch_ident lookup needed.
  Confirmed live: GET /besuch/13550/besucheForPatient returns 7 visit records (2026-08-22).
  Call pc-tomedo-patient-visits:
    URL: {{vars.tomedo_base_url}}/besuch/{{vars.patient_id}}/besucheForPatient
  Returns visit records for the patient's consultation history.
  Each record: ankunft (epoch ms), ident, kvFall, privatFall.
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
  The query must be URL-encoded. Name-only search — phone search is non-functional.
  NOTE: searchByAttributes is confirmed broken in some configurations (returns `{}`).
  The LLM must compose the query from the user's intent (Tier 1).
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.9 — Domain Skill: `skill-tomedo` (class 2)

```
name:        "skill-tomedo"
class_code:  2
description: "Domain skill: tomedo EMR REST API integration — patient reads, karteieintrag writes, and automated evening documentation audit."
body: |
  tomedo is a medical practice management system (EMR). This domain skill routes
  to the correct leaf skill for each REST API operation.

  ══════════════════════════════════════════════════════════════
  ORCHESTRATOR-FIRST DESIGN RULE (mandatory for all tomedo ops):
  ══════════════════════════════════════════════════════════════
  • Rust makes tools available (channel: "rust" pre-loads the ToolSkill binding).
  • The ORCHESTRATOR executes every tool call via __execute_action__() in a
    PythonCode step (channel: "orchestrator"). Rust NEVER executes on its own.
  • The LLM is involved ONLY for: content composition, user-supplied strings,
    irreversible action confirmation, or ambiguous disambiguation.
  • Every deterministic read (known patient_id, known date, known ident) is Tier 0.
  • One ToolSkill = one URL pattern + one HTTP method. One PythonCode executor =
    one __execute_action__ call. Pure-logic helpers = zero I/O.
  ══════════════════════════════════════════════════════════════

  FIRST-RUN SETUP:
    1. Run tomedo-cert-fetch (Tier 1 — SSH credentials required).
    2. Run tomedo-karteieintragtyp-list (Tier 0) to discover the ANA ident.
       Set config key tomedo_ana_typ_ident to the result before running the audit.
    Required config after setup: tomedo_cert_pem, tomedo_base_url,
    tomedo_ana_typ_ident (for audit completeness checks).

  OPERATION ROUTING:

  Server health (Tier 0):
    → skill-tomedo-serverstatus                     → tomedo-serverstatus

  KarteiEintragTyp lookup — resolve ANA ident (Tier 0, run once):
    → skill-tomedo-karteieintragtyp-list            → tomedo-karteieintragtyp-list

  Patient lookup by name (Tier 1 — LLM URL-encodes query):
    → skill-tomedo-patient-search-by-name           → tomedo-patient-search-by-name

  Full patient summary — automated (Tier 0, orchestrator runs 4 reads):
    → use recipe: tomedo-patient-summary

  Individual patient data (known patient_id — all Tier 0):
    • Name + DOB + phones:  skill-tomedo-patient-detail           → tomedo-patient-detail
    • Diagnoses:            skill-tomedo-patient-diagnoses        → tomedo-patient-diagnoses
    • Medications:          skill-tomedo-patient-medications      → tomedo-patient-medications
    • Next appointment:     skill-tomedo-patient-appointments     → tomedo-patient-next-appointment
    • Visit history:        skill-tomedo-patient-visits           → tomedo-patient-visits
      ✅ visits uses patient_id directly — no besuch_ident lookup needed

  EBM/GOÄ Leistungen (Tier 0, safe two-step path):
    → skill-tomedo-leistungen-read                                → tomedo-leistungen-read
    ⚠️ NEVER use /leistung?patient=X — crashes server

  KarteiEintrag writes (Tier 1 — LLM confirms before dispatch):
    • Full create (any type):    skill-tomedo-karteieintrag-create   → tomedo-karteieintrag-create
    • Quick ANM note (optimised):                                     → tomedo-karteieintrag-anmerkung
    • Update/soft-delete:        skill-tomedo-karteieintrag-update   → tomedo-karteieintrag-update

  Appointment writes (Tier 1 — LLM confirms before dispatch):
    • Create appointment:        skill-tomedo-termin-create          → tomedo-termin-create
    • Cancel/reschedule:         skill-tomedo-termin-update          → tomedo-termin-update

  Abenddokumentation-Audit (ALL Tier 0 — no LLM):
    • Today's patient list:      skill-tomedo-tagesliste-get         → tomedo-tagesliste-get
    • Per-patient data fetch:    skill-tomedo-abend-audit-fetch-patient → tomedo-abend-audit-fetch-patient
    • Privat completeness check: skill-tomedo-abend-audit-check-privat
    • GKV completeness check:    skill-tomedo-abend-audit-check-gkv
    • HZV completeness check:    skill-tomedo-abend-audit-check-hzv
    • Full nightly audit:                                             → tomedo-abend-audit
      (fetches tagesliste → classifies insurance → checks all patients → reports missing)

  AUTH: All API calls require tomedo_cert_pem config key set (mTLS).
  Run tomedo-cert-fetch recipe once for first-time setup.
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
description: "Leaf skill: create a new KarteiEintrag for a patient — three-step: POST entry, PUT patient DB join, PUT patientendetailsrelationen sync record — Tier 1."
body: |
  Create a KarteiEintrag for patient {{vars.patient_id}} using the confirmed three-step pattern.

  ⚠️ CRASH RULE: ALL FOUR relation fields mandatory in step 1. Omitting any one →
  null-ident sync crash in ZSTransferFetchedDataThread / JSON2CoreData.m:349 on Mac clients.
  Use ident-based references (NOT kuerzel — kuerzel not resolved server-side).
  letzterNutzer MUST be set in step 1 POST body (same ident as dokumentierenderNutzer).
  DO NOT PUT letzterNutzer after creation — read-only via PUT, corrupts sync record.

  STEP 1 — Create the entry (pc-tomedo-karteieintrag-create):
    POST {{vars.tomedo_base_url}}/karteieintrag
    Required vars:
      text                   — entry text
      datum                  — epoch ms (use current time if not specified)
      kartei_typ_ident       — use ident: 6=ANM, 2=BEF, 4=DIA, 20=LAB
      media_typ_ident        — 1 (Text)
      nutzer_ident           — dokumentierenderNutzer ident (e.g. 39205294877179905)
      betriebsstaette_ident  — 1 (practice default)
    Returns: {new_ident}

  STEP 2 — Write DB join row (pc-tomedo-karteieintrag-link):
    PUT {{vars.tomedo_base_url}}/patient/{{vars.patient_id}}
    Body: {patientenDetails:{patientenDetailsRelationen:{karteiEintraege:[{ident:<new_ident>}]}}}
    Returns: HTTP 204. (DB join table written, Mac client NOT yet notified.)

  STEP 3 — Write sync change record (pc-tomedo-patientendetailsrelationen-link):
    PUT {{vars.tomedo_base_url}}/patientendetailsrelationen/{{vars.patient_id}}
    Body: {ident: <patient_id>, karteiEintraege: [{ident: <new_ident>}]}
    Returns: HTTP 204. ZSTransferFetchedDataThread picks up the PatientenDetailsRelationen
    change record and immediately adds the entry to every Mac client's kartei list.
    ⚠️ Both ident values in this body MUST be valid integers — null ident → crash loop.

  Before dispatching step 1: show the user the text and entry type and ask for confirmation.
  After step 3: surface the new ident to the user and confirm it is visible in kartei.
  To undo: call skill-tomedo-karteieintrag-update with {visible: false}.
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

  Common operations (body_json examples):
    Soft-delete:   {"visible": false}
    Update text:   {"text": "corrected text"}
    Change type:   {"karteiEintragTyp": {"ident": 6}}   ← use ident NOT kuerzel

  ⚠️ Always use ident-based references — kuerzel strings are not resolved server-side.
  ⚠️ DO NOT include letzterNutzer in a PUT body — it is read-only via PUT and
     corrupts the sync record, causing JSON2CoreData.m:349 crash loop.

  Before dispatching: confirm the change with the user.
  A successful update returns HTTP 204 (no body).
  DELETE is NOT supported — always use PUT {"visible": false} to hide entries.

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
  To cancel an existing Termin: use skill-tomedo-termin-update (PUT /termin/{ident} {removed:true}).

  Date/time: convert human-readable time to epoch ms before passing.
  Duration: typical appointments are 10–30 min; use 1800000 ms (30 min) as default if not specified.
  PythonCode: use pc-tomedo-termin-create.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4a.5 — Leaf Skill: `skill-tomedo-termin-update` (class 1)

```
name:        "skill-tomedo-termin-update"
class_code:  1
description: "Leaf skill: update or cancel an existing Termin via PUT — Tier 1."
body: |
  Update or cancel an existing Termin by ident using ts-tomedo-termin-update.
  URL: PUT {{vars.tomedo_base_url}}/termin/{{vars.termin_ident}}

  Common operations (body_json examples):
    Cancel:        {"removed": true}
    Reschedule:    {"beginn": epoch_ms, "ende": epoch_ms}
    Update info:   {"info": "new reason text"}

  ⚠️ removed is Boolean (true/false) — not an integer.
  DELETE is NOT supported — use PUT {"removed": true} to cancel.

  Before dispatching: confirm the action with the user.
  A successful update returns HTTP 204 (no body).
  PythonCode: use pc-tomedo-termin-update.
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

### Step 4a.6 — Leaf Skill: `skill-tomedo-ebmleistung-create` (class 1)

```
name:        "skill-tomedo-ebmleistung-create"
class_code:  1
description: "Leaf skill: add an EBM billing code (Ziffer) to a patient's KV-Schein — two-step: POST /ebmleistung, PUT /kvschein link — Tier 1."
body: |
  Add EBM Ziffer {{vars.ebm_code}} to patient {{vars.patient_id}}'s KV-Schein {{vars.schein_ident}}.

  ⚠️ ENDPOINT RULE: POST to /ebmleistung (NOT /leistung).
  /leistung stores dtype='Leistung' and drops ebmKatalogEintrag — confirmed broken 2026-08-22.
  Only /ebmleistung stores dtype='EBMLeistung' with ebmKatalogEintrag correctly.

  STEP 1 — Create the EBMLeistung (pc-tomedo-ebmleistung-create):
    POST {{vars.tomedo_base_url}}/ebmleistung
    Body (all fields mandatory):
    {
      "datum":                  <epoch_ms>,
      "visible":                true,
      "anzahl":                 1,
      "ebmKatalogEintrag":      {"ident": <ebm_catalog_ident>},   ← internal ident, NOT Ziffer string
      "leistungserbringer":     {"ident": <nutzer_ident>},
      "betriebsstaette":        {"ident": 1},
      "dokumentierenderNutzer": {"ident": <nutzer_ident>},
      "letzterNutzer":          {"ident": <nutzer_ident>},
      "abrechnenderArzt":       {"ident": <nutzer_ident>}
    }
    Returns: {new_leistung_ident}

  To resolve EBM Ziffer string → internal catalog ident:
    Query: SELECT ident FROM ebmkatalogeintrag WHERE code = '<ziffer>';
    Or use pc-tomedo-ebmkatalogeintrag-get if only the internal ident is known (reverse: ident→code).
    Known idents: 1=01100, 270=03003, 298=03230 (confirmed live this server).

  STEP 2 — Link to KV-Schein (pc-tomedo-kvschein-link-leistung):
    PUT {{vars.tomedo_base_url}}/kvschein/{{vars.schein_ident}}
    Body: {"ident": <schein_ident>, "ebmLeistungen": [{"ident": <new_leistung_ident>}]}
    Returns: HTTP 204. Sets invkvschein_ident on leistung row, writes KVSchein change record.
    Mac client picks up the new Leistung via ZSTransferFetchedDataThread immediately.

  Before dispatching step 1: confirm the Ziffer, patient, and Schein with the user.
  To undo: DELETE directly from leistung table (no REST delete endpoint — HTTP 405).
    Also DELETE the KVSchein change row linking to this leistung from the change table.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---


## Step 4b — (LLM Object Composition — removed, separate plan)

*(Steps 4b and 4c are not part of this plan. LLM composition and LLM service
skills will be defined in the tomedo-llm and tomedo-compose extension plans.)*

*(All 4b and 4c skill bodies removed — see note above.)*

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
description:       "Fetch visit/consultation records for a patient using patient_id directly. Single-step — no prior besuch_ident lookup needed (confirmed live 2026-08-22)."
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
    "label":   "Execute: GET /besuch/{patient_id}/besucheForPatient → visit records"
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
  {"input": "krankenakte besuche",                        "class": 2},
  {"input": "wann war patient zuletzt hier",              "class": 3},
  {"input": "letzter besuch patient 13550",               "class": 3},
  {"input": "show consultation history for patient",      "class": 2},
  {"input": "patient vorstellungen anzeigen",             "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-search-by-name` (class 21) — Tier 1

```
name:              "tomedo-patient-search-by-name"
description:       "Search patients by name — LLM composes the URL-encoded query from user intent, orchestrator executes the GET."
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
    "label":   "LLM URL-encodes the patient name from user intent (e.g. 'Müller' → 'M%C3%BCller')"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-search>"],
    "label":   "Pre-load ts-tomedo-patient-search binding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-search>"],
    "label":   "Execute: GET /patient/searchByAttributes?query={encoded_name}"
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
  {"input": "find Herbert in tomedo",                     "class": 3},
  {"input": "patient Schmidt nachschlagen",               "class": 3},
  {"input": "lookup patient by last name",                "class": 2},
  {"input": "search for patient Weber",                   "class": 3},
  {"input": "patienten mit nachnamen Bauer finden",       "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-leistungen-read` (class 21) — Tier 0

```
name:              "tomedo-leistungen-read"
description:       "Read Leistungen (EBM/GOÄ billing codes) for a patient via the safe two-step KV-Schein path. Never uses the crash-inducing /leistung?patient=X endpoint."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-leistungen-read>", "<uuid:skill-tomedo>"],
    "label":   "Load leistungen-read leaf + domain skill"
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
    "label":   "Execute: GET /patient/{id}/patientenDetailsRelationen?limitScheine=true → kvScheine[].ident"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-kvschein-get>"],
    "label":   "Pre-load ts-tomedo-kvschein-get binding"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-kvschein-get>"],
    "label":   "Execute: GET /kvschein/{schein_ident} → ebmLeistungen[], goaeLeistungen[]"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-ebmkatalogeintrag-get>"],
    "label":   "Pre-load ts-tomedo-ebmkatalogeintrag-get binding"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-ebmkatalogeintrag-get>"],
    "label":   "Execute: GET /ebmkatalogeintrag/{ident} → EBM Ziffer string (orchestrator calls rust via __execute_action__)"
  }
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
  {"input": "fetch billing codes patient",                    "class": 2},
  {"input": "KV-Schein leistungen lesen",                     "class": 2},
  {"input": "abgerechnete EBM ziffern heute",                 "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-karteieintrag-create` (class 21) — Tier 1

```
name:              "tomedo-karteieintrag-create"
description:       "Create a new KarteiEintrag for a patient — three-step write: POST entry, PUT patient DB join row, PUT patientendetailsrelationen sync record. LLM confirms content before any dispatch."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-karteieintrag-create>", "<uuid:skill-tomedo>"],
    "label":   "Load karteieintrag-create leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM confirms entry text, type (ANM/BEF/DIA/LAB), and patient with user before dispatch"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-karteieintrag-create>"],
    "label":   "Pre-load ts-tomedo-karteieintrag-create binding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-karteieintrag-create>"],
    "label":   "Step 1: POST /karteieintrag with all 4 mandatory relation fields → {new_ident}"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-link-karteieintrag>"],
    "label":   "Pre-load ts-tomedo-patient-link-karteieintrag binding"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-karteieintrag-link>"],
    "label":   "Step 2: PUT /patient/{id} → writes DB join row"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patientendetailsrelationen-link>"],
    "label":   "Pre-load ts-tomedo-patientendetailsrelationen-link binding"
  },
  {
    "step_id": "step-7",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patientendetailsrelationen-link>"],
    "label":   "Step 3: PUT /patientendetailsrelationen/{id} → writes sync record, Mac client shows entry"
  }
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
  {"input": "save consultation note to tomedo",               "class": 2},
  {"input": "befund in kartei eintragen patient",             "class": 3},
  {"input": "diagnosenotiz anlegen",                          "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-karteieintrag-update` (class 21) — Tier 1

```
name:              "tomedo-karteieintrag-update"
description:       "Update or soft-delete an existing KarteiEintrag — LLM confirms change, orchestrator PUTs. Use {visible:false} to hide; DELETE is not supported."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-karteieintrag-update>", "<uuid:skill-tomedo>"],
    "label":   "Load karteieintrag-update leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM confirms the update (text change, soft-delete, or type change) with user"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-karteieintrag-update>"],
    "label":   "Pre-load ts-tomedo-karteieintrag-update binding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-karteieintrag-update>"],
    "label":   "Execute: PUT /karteieintrag/{ident} with body_json"
  }
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
  {"input": "edit existing record entry tomedo",              "class": 2},
  {"input": "karteieintrag 192572 ausblenden",                "class": 3},
  {"input": "kartei eintrag text berichtigen",                "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-termin-create` (class 21) — Tier 1

```
name:              "tomedo-termin-create"
description:       "Create a new appointment (Termin) for a patient — LLM confirms datetime, duration, and reason with user, orchestrator POSTs."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-termin-create>", "<uuid:skill-tomedo>"],
    "label":   "Load termin-create leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM converts human-readable date/time to epoch ms and confirms with user"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-termin-create>"],
    "label":   "Pre-load ts-tomedo-termin-create binding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-termin-create>"],
    "label":   "Execute: POST /termin → {ident: N}"
  }
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
  {"input": "new termin datum uhrzeit",                       "class": 3},
  {"input": "termin morgen 10 uhr patient 776",               "class": 3},
  {"input": "folgetermin buchen nach behandlung",             "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-termin-update` (class 21) — Tier 1

```
name:              "tomedo-termin-update"
description:       "Update or cancel an existing Termin — LLM confirms the action, orchestrator PUTs. Use {removed:true} to cancel."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-termin-update>", "<uuid:skill-tomedo>"],
    "label":   "Load termin-update leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM confirms the cancellation or rescheduling with user"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-termin-update>"],
    "label":   "Pre-load ts-tomedo-termin-update binding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-termin-update>"],
    "label":   "Execute: PUT /termin/{ident} with body_json"
  }
]
intent_examples: [
  {"input": "termin absagen",                                 "class": 3},
  {"input": "termin stornieren",                              "class": 3},
  {"input": "cancel appointment tomedo",                      "class": 2},
  {"input": "termin patient absagen",                         "class": 3},
  {"input": "termin verschieben",                             "class": 3},
  {"input": "reschedule appointment",                         "class": 2},
  {"input": "termin entfernen removed true",                  "class": 3},
  {"input": "appointment cancellation",                       "class": 2},
  {"input": "cancel follow-up termin",                        "class": 3},
  {"input": "termin 192572 stornieren",                       "class": 3},
  {"input": "nächsten termin absagen",                        "class": 3},
  {"input": "termin für patient abbrechen",                   "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-ebmleistung-create` (class 21) — Tier 1

```
name:              "tomedo-ebmleistung-create"
description:       "Add an EBM billing code (Ziffer) to a patient's KV-Schein — two-step write: POST /ebmleistung, PUT /kvschein link. LLM confirms Ziffer and Schein with user before dispatch."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-ebmleistung-create>", "<uuid:skill-tomedo>"],
    "label":   "Load ebmleistung-create leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM resolves EBM Ziffer string → catalog ident, confirms Schein ident and Nutzer ident with user before dispatch"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-ebmleistung-create>"],
    "label":   "Pre-load ts-tomedo-ebmleistung-create binding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-ebmleistung-create>"],
    "label":   "Step 1: POST /ebmleistung with all mandatory fields → {new_leistung_ident}"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-kvschein-link-leistung>"],
    "label":   "Pre-load ts-tomedo-kvschein-link-leistung binding"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-kvschein-link-leistung>"],
    "label":   "Step 2: PUT /kvschein/{schein_ident} → links leistung, Mac client sees Ziffer immediately"
  }
]
intent_examples: [
  {"input": "EBM Ziffer eintragen",                              "class": 3},
  {"input": "Leistung auf Schein buchen",                        "class": 3},
  {"input": "add EBM code to schein",                            "class": 2},
  {"input": "EBM 01100 für patient erfassen",                    "class": 3},
  {"input": "Ziffer 03220 auf KV-Schein",                        "class": 3},
  {"input": "billing code to schein tomedo",                     "class": 2},
  {"input": "EBM abrechnen patient",                             "class": 3},
  {"input": "leistung hinzufügen zum schein",                    "class": 3},
  {"input": "post ebm leistung",                                 "class": 2},
  {"input": "neue EBM Ziffer auf aktuellem Schein",              "class": 3},
  {"input": "Unvorhergesehene Inanspruchnahme eintragen",        "class": 3},
  {"input": "add billing code 03003 to patient schein",          "class": 3}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-patient-summary` (class 21) — Tier 0

```
name:              "tomedo-patient-summary"
description:       "Fetch a complete patient summary in one automated sequence: basic record, diagnoses, medications, and next appointment. Useful for automated context-gathering before a consultation or agent action."
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
    "label":   "Fetch: GET /patient/{id} → name, DOB, phones"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-relations>"],
    "label":   "Pre-load ts-tomedo-patient-relations binding"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-relations>"],
    "label":   "Fetch: GET /patient/{id}/patientenDetailsRelationen → diagnosen[]"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-parse-diagnosen>"],
    "label":   "Parse diagnoses → comma-separated text"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-medications>"],
    "label":   "Pre-load ts-tomedo-patient-medications binding"
  },
  {
    "step_id": "step-7",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-medications>"],
    "label":   "Fetch: GET /patient/{id}/.../medikamentenPlan"
  },
  {
    "step_id": "step-8",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-parse-medications>"],
    "label":   "Format medications with dosing notation"
  },
  {
    "step_id": "step-9",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-appointments>"],
    "label":   "Pre-load ts-tomedo-patient-appointments binding"
  },
  {
    "step_id": "step-10",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-appointments>"],
    "label":   "Fetch: GET /patient/{id}/termine?flach=true"
  },
  {
    "step_id": "step-11",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-parse-next-appointment>"],
    "label":   "Find and format next future appointment"
  }
]
intent_examples: [
  {"input": "patient zusammenfassung",                        "class": 2},
  {"input": "patient overview",                               "class": 2},
  {"input": "alle infos zu patient 13550",                    "class": 3},
  {"input": "full patient summary",                           "class": 2},
  {"input": "patientenkontext für consultation",              "class": 3},
  {"input": "bereite patientenakte vor",                      "class": 3},
  {"input": "prepare patient context for consultation",       "class": 2},
  {"input": "diagnosen medikamente termin patient abrufen",   "class": 3},
  {"input": "komplettübersicht patient",                      "class": 2},
  {"input": "patient info vor termin laden",                  "class": 3},
  {"input": "summarize patient data",                         "class": 2},
  {"input": "patient daten zusammentragen",                   "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Recipe: `tomedo-karteieintrag-anmerkung` (class 21) — Tier 1

```
name:              "tomedo-karteieintrag-anmerkung"
description:       "Create a plain-text Anmerkung (ANM type) KarteiEintrag for a patient — optimized for scripted/automated note creation. LLM provides the text only; all structural params (typ=ANM, mediaTyp=Text) are pre-fixed."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-karteieintrag-create>", "<uuid:skill-tomedo>"],
    "label":   "Load karteieintrag-create leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM generates/confirms the note text only — typ=ANM(6), mediaTyp=1 are pre-set"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-karteieintrag-create>"],
    "label":   "Pre-load ts-tomedo-karteieintrag-create binding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-karteieintrag-create>"],
    "label":   "Step 1: POST /karteieintrag — karteiEintragTyp=6(ANM), mediaTyp=1(Text)"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-link-karteieintrag>"],
    "label":   "Pre-load ts-tomedo-patient-link-karteieintrag binding"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-karteieintrag-link>"],
    "label":   "Step 2: PUT /patient/{id} → writes DB join row"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patientendetailsrelationen-link>"],
    "label":   "Pre-load ts-tomedo-patientendetailsrelationen-link binding"
  },
  {
    "step_id": "step-7",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patientendetailsrelationen-link>"],
    "label":   "Step 3: PUT /patientendetailsrelationen/{id} → Mac client shows entry immediately"
  }
]
intent_examples: [
  {"input": "anmerkung für patient anlegen",                  "class": 3},
  {"input": "notiz in kartei schreiben",                      "class": 3},
  {"input": "quick note for patient",                         "class": 2},
  {"input": "telefonnotiz patient",                           "class": 3},
  {"input": "schnellnotiz kartei",                            "class": 2},
  {"input": "add simple note to patient record",              "class": 2},
  {"input": "textnotiz für patient 13550",                    "class": 3},
  {"input": "kurze notiz in die kartei",                      "class": 3},
  {"input": "ANM eintrag patient",                            "class": 3},
  {"input": "plain text karteieintrag",                       "class": 2},
  {"input": "consultation note plain text",                   "class": 2},
  {"input": "automatische notiz in kartei",                   "class": 3}
]
source: "system"
validation_status: "validated"
```


## Step 6 — ExtensionCatalogues (class 23)

Two catalogues: the tomedo REST API catalogue and the cert-fetch setup extension.

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
  │ GET  /besuch/{patient_id}/besucheForPatient → visit records (patient ident!)│
  │ GET  /kvschein/{ident}                → KV-Schein + ebmLeistungen[]     │
  │ GET  /ebmkatalogeintrag/{ident}       → EBM catalog ident → Ziffer str  │
  │ POST /karteieintrag                   → step 1: create entry            │
  │ PUT  /patient/{id}  (link body)       → step 2: link entry to patient   │
  │ POST /termin                          → create appointment; returns {ident}│
  │ PUT  /karteieintrag/{id}              → update; visible:false=soft-del  │
  │ PUT  /termin/{id}                     → update; removed:true=cancel     │
  │ PUT  /patient/{id}  (gesperrt body)   → update; gesperrt:1(int)=block   │
  └─────────────────────────────────────────────────────────────────────────┘
  BROKEN:  GET /patient/searchByAttributes → returns {} for all queries.
  CRASH:   GET /leistung?patient=X, /patient/{id}/leistungen — unbounded queries.
  Leistungen safe path: patientenDetailsRelationen?limitScheine=true → GET /kvschein/{ident}.
  Briefkommandos ($[l %nr 0d ,]$) reveal the data model.

  WRITE TYPE FACTS: gesperrt=Integer(1), removed=Boolean, visible=Boolean.
  Error responses use HTTP 460 with Java stack trace.
  3-STEP KARTEIEINTRAG CRASH RULE: POST creates entry → POST links (patientenDetailsRelationen
  join row) → PUT /patientendetailsrelationen/{id} syncs Mac client.
  Without step 3, Mac client crashes: JSON2CoreData.m:349 ident != NULL.
  letzterNutzer field required in step 1 POST body or same crash occurs.

  TASK GROUPS:
  1. Health checks:    tomedo-serverstatus
  2. Patient reads:    tomedo-patient-detail, tomedo-patient-diagnoses,
                       tomedo-patient-medications, tomedo-patient-next-appointment,
                       tomedo-patient-visits
  3. Leistungen read:  tomedo-leistungen-read (Tier 0, two-step safe path)
  4. Patient search:   tomedo-patient-search-by-name (Tier 1)
  5. Writes (Tier 1):  tomedo-karteieintrag-create, tomedo-karteieintrag-update,
                       tomedo-termin-create
  6. Abenddokumentation-Audit (all Tier 0):
       tomedo-karteieintragtyp-list (setup), tomedo-tagesliste-get,
       tomedo-abend-audit-fetch-patient, tomedo-abend-audit-check-patient,
       tomedo-abend-audit

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
      "tomedo-patient-visits"
    ]
  },
  {
    "group_name": "patient-summary",
    "summary": "Automated full patient context: name/DOB/phones + diagnoses + medications + next appointment (Tier 0 multi-step)",
    "recipe_ids": ["tomedo-patient-summary"]
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
    "group_name": "karteieintrag-writes",
    "summary": "Create/update KarteiEinträge via direct mTLS REST (Tier 1)",
    "recipe_ids": [
      "tomedo-karteieintrag-create",
      "tomedo-karteieintrag-anmerkung",
      "tomedo-karteieintrag-update"
    ]
  },
  {
    "group_name": "termin-writes",
    "summary": "Create/cancel/reschedule Termine via direct mTLS REST (Tier 1)",
    "recipe_ids": [
      "tomedo-termin-create",
      "tomedo-termin-update"
    ]
  },
  {
    "group_name": "abend-audit-setup",
    "summary": "One-time setup for the Abenddokumentation-Audit: resolve ANA KarteiEintragTyp ident (Tier 0)",
    "recipe_ids": ["tomedo-karteieintragtyp-list"]
  },
  {
    "group_name": "abend-audit",
    "summary": "Automated evening documentation completeness audit for all patients seen today — Tier 0, no LLM",
    "recipe_ids": [
      "tomedo-tagesliste-get",
      "tomedo-abend-audit-fetch-patient",
      "tomedo-abend-audit-check-patient",
      "tomedo-abend-audit"
    ]
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
description:       "One-time setup: fetch mTLS certs from the tomedo server via SSH and configure BrassClaw for HTTPS API access."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-cert-fetch>"],
    "label":   "Load cert-fetch leaf skill"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM presents SSH host/user/key to user and asks for confirmation before running"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-cert-fetch>"],
    "label":   "Pre-load ts-tomedo-cert-fetch binding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-cert-fetch>"],
    "label":   "Execute: SSH to server, fetch 3 PEM files, chmod 0600 private key"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-serverstatus>"],
    "label":   "Pre-load ts-tomedo-serverstatus binding for verification"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-serverstatus>"],
    "label":   "Verify: GET /serverstatus with new certs — confirms mTLS works"
  }
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

### Complete Component Count (tomedo v3 REST-only stack)

| Class | Count | Names |
|-------|-------|-------|
| 0 — Tool | 2 | `tomedo-api`, `tomedo-cert-fetch-tool` |
| 1 — Leaf Skill | 23 | `skill-tomedo-serverstatus` … `skill-tomedo-abend-audit-auto-add-01100` |
| 2 — Domain Skill | 1 | `skill-tomedo` |
| 13 — ToolSkill | 21 | `ts-tomedo-serverstatus` … `ts-tomedo-kvschein-link-leistung` |
| 21 — Recipe | 21 | `tomedo-serverstatus` … `tomedo-ebmleistung-create` |
| 22 — PythonCode | 41 | `pc-tomedo-serverstatus` … `pc-tomedo-check-01100-erforderlich` |
| 23 — ExtensionCatalogue | 2 | `ext-tomedo`, `ext-tomedo-cert-fetch` |
| **Total** | **111** | |

---

### Tier Classification Summary

| Tier | Recipes | Reason |
|------|---------|--------|
| **Tier 0** | 13 | Direct REST reads + Leistungen two-step + patient-summary + all audit recipes (tagesliste, per-patient fetch/check, full audit, karteieintragtyp-list) |
| **Tier 1** | 8 | 1 name-search + 6 writes (karteieintrag create/anmerkung/update, termin create/update, ebmleistung create) + 1 cert-fetch |

---

### Seeding Order (bootstrapped in this order per group)

```
Group 1 — Tools (class 0):
  1. tomedo-api
  2. tomedo-cert-fetch-tool

Group 2 — ToolSkills (class 13):
  3. ts-tomedo-serverstatus
  4. ts-tomedo-patient-detail
  5. ts-tomedo-patient-relations
  6. ts-tomedo-patient-medications
  7. ts-tomedo-patient-appointments
  8. ts-tomedo-patient-visits
  9. ts-tomedo-patient-search
  10. ts-tomedo-karteieintrag-create
  11. ts-tomedo-patient-link-karteieintrag
  12. ts-tomedo-patientendetailsrelationen-link
  13. ts-tomedo-karteieintrag-update
  14. ts-tomedo-termin-create
  15. ts-tomedo-termin-update
  16. ts-tomedo-kvschein-get
  17. ts-tomedo-ebmkatalogeintrag-get
  18. ts-tomedo-cert-fetch
  19. ts-tomedo-ebmleistung-create                  ← EBM Ziffer write (step 1)
  20. ts-tomedo-kvschein-link-leistung              ← EBM Ziffer write (step 2)
  21. ts-tomedo-tagesliste-get                      ← Abenddokumentation-Audit (pending)
  22. ts-tomedo-besuch-tagesliste-get               ← fallback day-list (pending)
  23. ts-tomedo-karteieintragtyp-list               ← ANA ident discovery (pending)

Group 3 — PythonCode executors (class 22, with __execute_action__):
  24. pc-tomedo-serverstatus
  25. pc-tomedo-patient-detail
  26. pc-tomedo-patient-relations
  27. pc-tomedo-patient-medications
  28. pc-tomedo-patient-appointments
  29. pc-tomedo-patient-visits
  30. pc-tomedo-patient-search
  31. pc-tomedo-karteieintrag-create
  32. pc-tomedo-karteieintrag-link
  33. pc-tomedo-patientendetailsrelationen-link
  34. pc-tomedo-karteieintrag-update
  35. pc-tomedo-termin-create
  36. pc-tomedo-termin-update
  37. pc-tomedo-kvschein-get
  38. pc-tomedo-ebmkatalogeintrag-get
  39. pc-tomedo-cert-fetch
  40. pc-tomedo-ebmleistung-create                  ← EBM Ziffer write (step 1)
  41. pc-tomedo-kvschein-link-leistung              ← EBM Ziffer write (step 2)
  42. pc-tomedo-tagesliste-get                      ← Abenddokumentation-Audit (pending)
  43. pc-tomedo-besuch-tagesliste-get               ← fallback (pending)
  44. pc-tomedo-karteieintragtyp-list               ← ANA discovery (pending)
  45. pc-tomedo-patient-relations-audit             ← per-patient audit fetch
  46. pc-tomedo-kvschein-audit                      ← per-patient schein fetch

Group 4 — PythonCode pure-logic helpers (class 22, no __execute_action__):
  47. pc-tomedo-parse-diagnosen
  48. pc-tomedo-parse-medications
  49. pc-tomedo-parse-next-appointment
  50. pc-tomedo-epoch-to-date
  51. pc-tomedo-extract-phone-fields
  52. pc-tomedo-build-today-date                    ← Abenddokumentation-Audit
  53. pc-tomedo-parse-tagesliste                    ← extract unique patient IDs
  54. pc-tomedo-classify-insurance                  ← Privat | GKV | HZV
  55. pc-tomedo-extract-diagnosen-from-relations    ← extract diagnosen[]
  56. pc-tomedo-extract-karteieintraege-from-relations ← extract karteiEintraege[]
  57. pc-tomedo-extract-kvscheine-from-relations    ← extract kvScheine[] + first_ident
  58. pc-tomedo-extract-scheinart                   ← extract scheinart string
  59. pc-tomedo-extract-leistungen-from-schein      ← extract ebm/goae leistungen[]
  60. pc-tomedo-check-kartei-vollstaendigkeit        ← ANA/BEF/BES presence check
  61. pc-tomedo-check-privat-vollstaendigkeit        ← Privat completeness
  62. pc-tomedo-check-gkv-vollstaendigkeit           ← GKV completeness
  63. pc-tomedo-check-hzv-vollstaendigkeit           ← HZV completeness
  64. pc-tomedo-format-audit-bericht                ← format final report

Group 5 — Leaf Skills (class 1):
  65. skill-tomedo-serverstatus
  66. skill-tomedo-patient-detail
  67. skill-tomedo-patient-diagnoses
  68. skill-tomedo-patient-medications
  69. skill-tomedo-patient-appointments
  70. skill-tomedo-patient-visits
  71. skill-tomedo-patient-search-by-name
  72. skill-tomedo-karteieintrag-create
  73. skill-tomedo-karteieintrag-update
  74. skill-tomedo-termin-create
  75. skill-tomedo-termin-update
  76. skill-tomedo-leistungen-read
  77. skill-tomedo-ebmleistung-create               ← EBM Ziffer write
  78. skill-tomedo-abend-audit-auto-add-01100       ← auto-add 01100 late-arrival
  79. skill-tomedo-cert-fetch
  80. skill-tomedo-karteieintragtyp-list            ← Abenddokumentation-Audit setup
  81. skill-tomedo-tagesliste-get                   ← day schedule fetch
  82. skill-tomedo-abend-audit-fetch-patient        ← per-patient data fetch
  83. skill-tomedo-abend-audit-check-privat         ← Privat completeness leaf
  84. skill-tomedo-abend-audit-check-gkv            ← GKV completeness leaf
  85. skill-tomedo-abend-audit-check-hzv            ← HZV completeness leaf

Group 6 — Domain Skills (class 2):
  86. skill-tomedo

Group 7 — Recipes (class 21):
  86. tomedo-serverstatus                            (Tier 0)
  87. tomedo-patient-detail                          (Tier 0)
  88. tomedo-patient-diagnoses                       (Tier 0)
  89. tomedo-patient-medications                     (Tier 0)
  90. tomedo-patient-next-appointment                (Tier 0)
  91. tomedo-patient-visits                          (Tier 0)
  92. tomedo-leistungen-read                         (Tier 0 — kvschein two-step, now with ebmkatalogeintrag executor)
  93. tomedo-patient-summary                         (Tier 0 — automated multi-step context fetch)
  94. tomedo-patient-search-by-name                  (Tier 1 — LLM URL-encodes name)
  95. tomedo-karteieintrag-create                    (Tier 1 — LLM confirms, orchestrator POSTs 3-step)
  96. tomedo-karteieintrag-anmerkung                 (Tier 1 — ANM optimised, LLM provides text only)
  97. tomedo-karteieintrag-update                    (Tier 1 — LLM confirms, orchestrator PUTs)
  98. tomedo-termin-create                           (Tier 1 — LLM confirms date/time, orchestrator POSTs)
  99. tomedo-termin-update                           (Tier 1 — LLM confirms cancel/reschedule, orchestrator PUTs)
  100. tomedo-ebmleistung-create                     (Tier 1 — LLM confirms Ziffer + Schein, orchestrator POSTs 2-step)
  101. tomedo-cert-fetch                             (Tier 1 — one-time setup, LLM confirms SSH)
  102. tomedo-karteieintragtyp-list                  (Tier 0 — setup: resolve ANA ident, pending)
  103. tomedo-tagesliste-get                         (Tier 0 — day schedule, pending)
  104. tomedo-abend-audit-fetch-patient              (Tier 0 — per-patient data fetch)
  105. tomedo-abend-audit-check-patient              (Tier 0 — per-patient completeness check)
  106. tomedo-abend-audit                            (Tier 0 — full nightly audit, pending tagesliste)

Group 8 — ExtensionCatalogues (class 23):
  107. ext-tomedo
  108. ext-tomedo-cert-fetch
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
| Two tool surfaces via `builtin.http` + `builtin.shell` | http handles all mTLS GET/POST/PUT calls; shell handles SSH cert-fetch setup |
| 23 ToolSkills | One per distinct URL/method/operation — reads, writes, Leistungen, termin-update, ebmleistung create/link, cert-fetch, + 3 audit ToolSkills |
| 23 PythonCode executors + 17 pure-logic helpers | Executors call `__execute_action__` exactly once; helpers transform data without I/O; strict 1:1 ratio per tool binding |
| 22 leaf skills | One per distinct approach — each skill covers one sub-task, not one "feature"; audit has 6 dedicated leaf skills |
| 13 Tier-0 recipes | All known-ID/date reads + Leistungen two-step + patient-summary + full audit suite (orchestrator-only) |
| 8 Tier-1 recipes | 1 name-search + 6 write variants (incl. ebmleistung create) + 1 cert-fetch |
| Orchestrator-first mandatory | Every rust step MUST be paired with an orchestrator PythonCode executor — rust alone is a Q1 error |
| One-tool-call-per-skill | Three similar skills > one monolithic skill; each PythonCode calls `__execute_action__` exactly once |
| `tomedo-leistungen-read` fix | Added missing `pc-tomedo-ebmkatalogeintrag-get` orchestrator step — was a Q1 Rule 2 violation |
| `tomedo-patient-summary` Tier-0 | Automated context-gathering — orchestrator runs 4 reads with no LLM |
| `tomedo-karteieintrag-anmerkung` | Optimised for scripted notes — LLM provides text only, structural params (ANM/Text) pre-fixed |
| `tomedo-termin-update` added | Cancel/reschedule was missing — `removed:true` to cancel, `beginn`/`ende` to reschedule |
| All recipes use `step_descriptions` | Consistent format; old `rust_steps`/`orchestrator_steps` fields removed |
| German + English intent examples (12+ each) | Praxis staff speak German; orchestrator must handle both |
| Direct mTLS REST API supports GET + POST + PUT | Confirmed live 2026-08-22 — no partner agreement needed for writes |
| Leistungen via kvschein two-step only | `GET /leistung?patient=X` crashes server — must go patientenDetailsRelationen → kvschein |
| EBM/GOÄ mode via Schein presence | Detect EBM vs GOÄ by checking kvScheine vs goaeScheine in relations |
| Cert-fetch extension | One-time SSH setup fetches mTLS PEM bundle from server; required before any tomedo REST call |
| 3-step karteieintrag write (JSON2CoreData crash rule) | POST creates entry → PUT /patient links DB join row → PUT /patientendetailsrelationen syncs Mac client; without step 3 the Mac client crashes with `JSON2CoreData.m:349 ident != NULL` |
| `letzterNutzer` required in POST body, forbidden in PUT | Missing from POST → crash; present in PUT → corrupts sync record → crash |
| visits recipe single-step | /besuch/{patient_id}/besucheForPatient takes patient ident directly — confirmed live 2026-08-22 |
| Abenddokumentation-Audit fully Tier-0 | Date computation (pc-tomedo-build-today-date), insurance classification, and completeness checks are all deterministic — no LLM needed |
| Audit has 3 separate completeness-check leaf skills | `check-privat`, `check-gkv`, `check-hzv` are distinct skills (not one monolithic skill) — each covers one insurance type's rules |
| Tagesliste pending validation | Two endpoint candidates (`termin?datum` + `besuch/tagesliste`) need live probing; recipe pre-loads both with fallback logic |
| ANA ident requires one-time setup | `GET /karteieintragtyp` once to discover ANA ident; fallback is kürzel-string matching in check logic |
| Insurance type from besuch flags | `privatFall=true` → Privat; `kvFall=true` + `scheinart` containing "hzv" → HZV; `kvFall=true` otherwise → GKV |
| `POST /ebmleistung` NOT `/leistung` | `/leistung` stores `dtype='Leistung'` and drops `ebmKatalogEintrag` — confirmed broken 2026-08-22; only `/ebmleistung` creates correct `EBMLeistung` rows |
| EBMLeistung 2-step write | POST `/ebmleistung` → `{new_ident}`; then PUT `/kvschein/{id}` with `{ebmLeistungen:[{ident:N}]}` sets `invkvschein_ident` and writes KVSchein change record for Mac client sync |
| EBM Ziffer → catalog ident lookup | Ziffer strings (e.g. `01100`) must be resolved to internal ints via `ebmkatalogeintrag.code`; known: `1=01100`, `270=03003`, `298=03230` (this server) |
| 01100 late-arrival rule | GKV only (not Privat, not HZV): Monday `ankunft` > 20:00 local / Tue–Sun > 19:00 local → EBM 01100 required on Schein; `ankunft` epoch ms from API is UTC — convert via `Europe/Berlin` tz |
| `ankunft` field confirmed via API | `GET /besuch/{patient_id}/besucheForPatient` returns `ankunft` as epoch ms UTC, `kvFall` bool, `privatFall` bool — confirmed live 2026-08-22 |



---

## Step 8 — Abenddokumentation-Audit

> **Purpose:** Every evening, automatically fetch all patients seen that day,
> sort them by insurance type (Privat / GKV / HZV), and check each patient for
> documentation completeness. Patients with missing documentation are reported
> to the chat with their ID, name, and the specific missing item(s).
>
> **Tier:** Tier 0 throughout — the entire audit runs without LLM involvement.
> The orchestrator fetches data, classifies insurance, checks completeness rules,
> and formats the report. The LLM is never called.
>
> **Completeness rules per insurance type:**
>
> | Type | Required |
> |------|---------|
> | Privat | Diagnose · KarteiEintrag (ANA + BEF + Besuch) · Rechnung (GOÄ-Leistungen vorhanden) |
> | GKV | Diagnose · KarteiEintrag (ANA + BEF + Besuch) · Schein (KV-Schein) · EBM-Ziffern auf dem Schein |
> | HZV | Diagnose · KarteiEintrag (ANA + BEF + Besuch) · Schein (HZV-Schein) · HZV-Ziffern auf dem Schein |
>
> **One-tool-call-per-skill principle (mandatory):**
> Every ToolSkill binds exactly one URL pattern with one HTTP method.
> Every PythonCode executor calls `__execute_action__` exactly once.
> Pure-logic PythonCode helpers do zero I/O — they transform already-fetched data.
> This gives maximum orchestrator visibility and makes every step individually retryable.
>
> **Insurance classification:**
> - `besuch.privatFall == true` → Privat
> - `besuch.kvFall == true` AND Schein has HZV-Ziffern (see §hzv-ziffern) → HZV
> - `besuch.kvFall == true` AND no HZV-Ziffern → GKV
>
> **§hzv-ziffern:** HZV-Ziffern are practice-specific contracted codes. Detection:
> the schein field `scheinart` contains `"HZV"` or `"hzv"` (case-insensitive),
> OR ebmLeistungen contain at least one Ziffer in the known HZV code range
> (practice-specific; fallback: any Ziffer starting with `"0"` that is NOT
> a standard EBM Ziffer is HZV). The definitive reference is the practice's HZV
> contract (https://www.haevbw.de/HZV-Gegenueberstellung.pdf).
>
> **KarteiEintragTyp idents needed for audit:**
> | ident | kürzel | Beschreibung |
> |-------|--------|-------------|
> | ⚠️ TBD | ANA | Anamnese — ident not yet confirmed on this server; probe with GET /karteieintragtyp |
> | 2 | BEF | Befund |
> | 18 | BES | Besuch (auto-created on patient check-in) |
>
> **Note on ANA ident:** The Anamnese KarteiEintragTyp ident is not yet confirmed
> on this server. The audit logic uses kürzel-string matching as fallback when ident
> is unknown. Probe once with `GET /{db}/karteieintragtyp` and record the ident.
>
> **Tagesliste endpoint (pending validation):**
> `GET /{db}/termin?datum={YYYY-MM-DD}&flach=true` — expected to return today's
> Termine with patient.ident, beginn, ende fields. Marked `validation_status: "pending"`.
> If this endpoint is not available, use `GET /{db}/besuch/tagesliste?datum={date}`
> as fallback (also pending).

---

### Step 8.1 — ToolSkills (class 13) for Abenddokumentation-Audit

---

#### ToolSkill: `ts-tomedo-tagesliste-get` (class 13)

```
name:          "ts-tomedo-tagesliste-get"
tool_name:     "tomedo-api"
description:   "GET /{db}/termin?datum={YYYY-MM-DD}&flach=true — fetch the day schedule
                as a flat array of Termin objects for a specific date.
                Each entry includes patient.ident, beginn (epoch ms), ende (epoch ms), info.
                Use datum in YYYY-MM-DD format (e.g. '2026-08-22').
                This is the entry point for the Abenddokumentation-Audit.
                ⚠️ validation_status: pending — endpoint shape not yet confirmed live.
                Fallback: GET /{db}/besuch/tagesliste?datum={YYYY-MM-DD} if termin?datum fails.
                Use timeout_ms: 20000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/termin?datum={YYYY-MM-DD}&flach=true"},
  {name: "method",     param_type: "string", required: true, description: "GET"},
  {name: "timeout_ms", param_type: "number", required: false,
   description: "Timeout in ms, default 20000"}
]
param_template: {
  "url":        "{{vars.tomedo_base_url}}/termin?datum={{vars.datum_ymd}}&flach=true",
  "method":     "GET",
  "timeout_ms": 20000
}
preconditions:  "datum_ymd must be YYYY-MM-DD format. tomedo_cert_pem must be set."
error_handling: "HTTP 404 or empty array → no termine for this date. HTTP 460 → endpoint may not exist — try fallback besuch/tagesliste."
category:       "tomedo"
source:         "system"
validation_status: "pending"
```

---

#### ToolSkill: `ts-tomedo-besuch-tagesliste-get` (class 13)

```
name:          "ts-tomedo-besuch-tagesliste-get"
tool_name:     "tomedo-api"
description:   "GET /{db}/besuch/tagesliste?datum={YYYY-MM-DD} — fallback day-schedule
                endpoint using the besuch (visit) table instead of the termin table.
                Returns all Besuch records for a given day including:
                  patient.ident, ankunft (epoch ms, arrival), abgang (epoch ms, departure),
                  kvFall (bool), privatFall (bool).
                Use when ts-tomedo-tagesliste-get (termin?datum) returns 404 or is unavailable.
                ⚠️ validation_status: pending — endpoint shape not yet confirmed live.
                Use timeout_ms: 20000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/besuch/tagesliste?datum={YYYY-MM-DD}"},
  {name: "method",     param_type: "string", required: true, description: "GET"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url":        "{{vars.tomedo_base_url}}/besuch/tagesliste?datum={{vars.datum_ymd}}",
  "method":     "GET",
  "timeout_ms": 20000
}
preconditions:  "datum_ymd must be YYYY-MM-DD format. tomedo_cert_pem must be set."
error_handling: "HTTP 404 → endpoint does not exist on this server. Empty array → no visits for date."
category:       "tomedo"
source:         "system"
validation_status: "pending"
```

---

#### ToolSkill: `ts-tomedo-karteieintragtyp-list` (class 13)

```
name:          "ts-tomedo-karteieintragtyp-list"
tool_name:     "tomedo-api"
description:   "GET /{db}/karteieintragtyp — fetch the complete list of KarteiEintragTyp
                objects on this server. Used once to resolve unknown idents such as ANA
                (Anamnese). Each entry: ident (int), kuerzel (e.g. 'ANA'), bezeichnung.
                Run this once and record the ANA ident before deploying the audit.
                Use timeout_ms: 10000."
param_schema:  [
  {name: "url",        param_type: "string", required: true,
   description: "https://{host}:{port}/{db}/karteieintragtyp"},
  {name: "method",     param_type: "string", required: true, description: "GET"},
  {name: "timeout_ms", param_type: "number", required: false}
]
param_template: {
  "url":        "{{vars.tomedo_base_url}}/karteieintragtyp",
  "method":     "GET",
  "timeout_ms": 10000
}
preconditions:  "tomedo_cert_pem must be set. Run once to discover ANA ident."
error_handling: "HTTP 404 → endpoint not available on this server version."
category:       "tomedo"
source:         "system"
validation_status: "pending"
```

---

### Step 8.2 — PythonCode Executors (class 22) for Abenddokumentation-Audit

One `__execute_action__` call per executor. No imports beyond stdlib.
Each executor does exactly one thing: fetches one resource.

---

#### PythonCode: `pc-tomedo-tagesliste-get` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 0
# Fetches today's Termin list by date (YYYY-MM-DD) via ts-tomedo-tagesliste-get.
# IBS bakes in {{vars.tomedo_base_url}} and {{vars.datum_ymd}} before execution.
# datum_ymd format: YYYY-MM-DD (e.g. "2026-08-22").
# Returns flat array of Termin objects; each has patient.ident, beginn, ende.
# validation_status: pending — endpoint shape not yet confirmed on this server.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/termin?datum={{vars.datum_ymd}}&flach=true",
    "method": "GET",
    "timeout_ms": 20000
})
```

---

#### PythonCode: `pc-tomedo-besuch-tagesliste-get` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 0
# Fallback: fetches today's Besuch (visit) list by date via ts-tomedo-besuch-tagesliste-get.
# Use when pc-tomedo-tagesliste-get returns 404 or empty.
# Returns array of Besuch objects; each has patient.ident, ankunft, abgang,
# kvFall (bool), privatFall (bool).
# validation_status: pending — endpoint shape not yet confirmed on this server.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/besuch/tagesliste?datum={{vars.datum_ymd}}",
    "method": "GET",
    "timeout_ms": 20000
})
```

---

#### PythonCode: `pc-tomedo-karteieintragtyp-list` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 0
# Fetches the complete KarteiEintragTyp list to resolve unknown idents (e.g. ANA).
# Run once during setup; record the ANA ident for use in the audit logic.
# validation_status: pending — endpoint shape not yet confirmed on this server.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/karteieintragtyp",
    "method": "GET",
    "timeout_ms": 10000
})
```

---

#### PythonCode: `pc-tomedo-patient-relations-audit` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 0
# Fetches patientenDetailsRelationen for one patient_id for audit purposes.
# Uses limitScheine=true to include kvScheine[].ident, diagnosen[], and
# karteiEintraege[] — all fields needed by the completeness checks.
# One call per patient. IBS bakes in vars before execution.
# NOTE: strips embedded control characters from response body (confirmed live 2026-08-22).
import re as _re
_raw = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/patient/{{vars.patient_id}}/patientenDetailsRelationen?limitScheine=true&limitKartei=100&limitVerordnungen=0&limitZeiterfassungen=false&limitBehandlungsfaelle=false",
    "method": "GET",
    "timeout_ms": 15000
})
if isinstance(_raw, dict) and "body" in _raw:
    _raw["body"] = _re.sub(r'[\x00-\x08\x0b\x0c\x0e-\x1f]', '', _raw["body"])
result = _raw
```

---

#### PythonCode: `pc-tomedo-kvschein-audit` (class 22)

```python
# Channel: orchestrator | Class: 22 | Tier 0
# Fetches one KV-Schein by ident for the audit.
# Returns ebmLeistungen[], goaeLeistungen[], and scheinart (for HZV detection).
# schein_ident must come from patientenDetailsRelationen kvScheine[].ident.
# One call per schein. IBS bakes in vars before execution.
result = __execute_action__("tomedo-api", {
    "url": "{{vars.tomedo_base_url}}/kvschein/{{vars.schein_ident}}",
    "method": "GET",
    "timeout_ms": 15000
})
```

---

### Step 8.3 — Pure-Logic PythonCode Helpers (class 22) for Abenddokumentation-Audit

No `__execute_action__` calls. All inputs come from baked-in `{{vars.*}}` slots
holding previously-fetched JSON strings. Each helper does exactly one transform.

---

#### PythonCode: `pc-tomedo-parse-tagesliste` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts the unique patient IDs seen today from a Tagesliste response body.
# Accepts either a termin-array (each entry has patient.ident, beginn, ende)
# or a besuch-array (each entry has patient.ident, ankunft, abgang, kvFall, privatFall).
# Returns a list of dicts: [{patient_id, ankunft_ms, abgang_ms, kv_fall, privat_fall}]
# deduped by patient_id (keeps the entry with the latest ankunft).
import json as _j
try:
    data = _j.loads("{{vars.body}}")
    if not isinstance(data, list):
        data = list(data.values()) if isinstance(data, dict) else []
    seen = {}
    for entry in data:
        pid = None
        pat = entry.get("patient") or {}
        pid = pat.get("ident") if isinstance(pat, dict) else None
        if pid is None:
            continue
        ankunft = entry.get("ankunft") or entry.get("beginn") or 0
        abgang  = entry.get("abgang")  or entry.get("ende")   or 0
        kv      = bool(entry.get("kvFall",      False))
        privat  = bool(entry.get("privatFall",  False))
        if pid not in seen or ankunft > seen[pid]["ankunft_ms"]:
            seen[pid] = {
                "patient_id":  pid,
                "ankunft_ms":  ankunft,
                "abgang_ms":   abgang,
                "kv_fall":     kv,
                "privat_fall": privat
            }
    result = list(seen.values())
except Exception as e:
    result = {"error": str(e)}
```

---

#### PythonCode: `pc-tomedo-classify-insurance` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Classifies a single patient's insurance type from:
#   {{vars.privat_fall}}  — "true"/"false" string from tagesliste entry
#   {{vars.kv_fall}}      — "true"/"false" string from tagesliste entry
#   {{vars.scheinart}}    — scheinart string from kvschein (may be empty)
# Returns: "Privat" | "HZV" | "GKV" | "Unbekannt"
# HZV detection: scheinart contains "hzv" (case-insensitive).
_privat  = "{{vars.privat_fall}}".lower() == "true"
_kv      = "{{vars.kv_fall}}".lower()     == "true"
_sa      = "{{vars.scheinart}}".lower()

if _privat:
    result = "Privat"
elif _kv and "hzv" in _sa:
    result = "HZV"
elif _kv:
    result = "GKV"
else:
    result = "Unbekannt"
```

---

#### PythonCode: `pc-tomedo-check-kartei-vollstaendigkeit` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Checks karteiEintraege[] for the presence of required entry types:
#   ANA (Anamnese), BEF (Befund, ident=2), BES (Besuch, ident=18).
# {{vars.kartei_eintraege_json}} — JSON array of karteiEintrag objects from
#   patientenDetailsRelationen. Each has karteiEintragTyp.ident and/or
#   karteiEintragTyp.kuerzel.
# {{vars.ana_typ_ident}} — confirmed ANA ident (set to "0" if unknown → uses kuerzel fallback).
# Returns a dict: {"ANA": bool, "BEF": bool, "BES": bool}
import json as _j
try:
    eintraege = _j.loads("{{vars.kartei_eintraege_json}}")
    if not isinstance(eintraege, list):
        eintraege = []
    ana_ident = int("{{vars.ana_typ_ident}}") if "{{vars.ana_typ_ident}}".isdigit() else 0
    has_ana = has_bef = has_bes = False
    for e in eintraege:
        typ = e.get("karteiEintragTyp") or {}
        ident  = typ.get("ident",  0)
        kuerzel = (typ.get("kuerzel") or typ.get("kürzel") or "").upper()
        if ident == 2  or kuerzel == "BEF": has_bef = True
        if ident == 18 or kuerzel == "BES": has_bes = True
        if (ana_ident and ident == ana_ident) or kuerzel == "ANA": has_ana = True
    result = {"ANA": has_ana, "BEF": has_bef, "BES": has_bes}
except Exception as e:
    result = {"error": str(e), "ANA": False, "BEF": False, "BES": False}
```

---

#### PythonCode: `pc-tomedo-check-privat-vollstaendigkeit` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Checks completeness for a Privatpatient.
# Required: Diagnose · ANA + BEF + BES in Kartei · GOÄ-Leistungen (Rechnung) vorhanden.
# Inputs (all JSON strings baked in by IBS):
#   {{vars.diagnosen_json}}        — diagnosen[] array from patientenDetailsRelationen
#   {{vars.kartei_check_json}}     — output of pc-tomedo-check-kartei-vollstaendigkeit
#   {{vars.goae_leistungen_json}}  — goaeLeistungen[] array from kvschein
# Returns list of missing item strings (empty list = vollständig).
import json as _j
try:
    missing = []
    diagnosen = _j.loads("{{vars.diagnosen_json}}")
    if not isinstance(diagnosen, list) or len(diagnosen) == 0:
        missing.append("Diagnose fehlt")
    kartei = _j.loads("{{vars.kartei_check_json}}")
    if not kartei.get("ANA"): missing.append("Karteieintrag ANA (Anamnese) fehlt")
    if not kartei.get("BEF"): missing.append("Karteieintrag BEF (Befund) fehlt")
    if not kartei.get("BES"): missing.append("Karteieintrag BES (Besuch) fehlt")
    goae = _j.loads("{{vars.goae_leistungen_json}}")
    if not isinstance(goae, list) or len(goae) == 0:
        missing.append("Rechnung fehlt (keine GOÄ-Leistungen auf dem Schein)")
    result = missing
except Exception as e:
    result = ["Fehler bei der Prüfung: " + str(e)]
```

---

#### PythonCode: `pc-tomedo-check-gkv-vollstaendigkeit` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Checks completeness for a GKV-Patient.
# Required: Diagnose · ANA + BEF + BES in Kartei · KV-Schein vorhanden · EBM-Ziffern auf Schein.
# Also checks whether 01100 is required (late-arrival rule) and present.
# Inputs (all JSON strings baked in by IBS):
#   {{vars.diagnosen_json}}       — diagnosen[] array from patientenDetailsRelationen
#   {{vars.kartei_check_json}}    — output of pc-tomedo-check-kartei-vollstaendigkeit
#   {{vars.kv_scheine_json}}      — kvScheine[] array from patientenDetailsRelationen
#   {{vars.ebm_leistungen_json}}  — ebmLeistungen[] array from kvschein
#   {{vars.check_01100_json}}     — output of pc-tomedo-check-01100-erforderlich
#                                    {"erforderlich": bool, "reason": str}
# Returns list of missing item strings (empty list = vollständig).
import json as _j
try:
    missing = []
    diagnosen = _j.loads("{{vars.diagnosen_json}}")
    if not isinstance(diagnosen, list) or len(diagnosen) == 0:
        missing.append("Diagnose fehlt")
    kartei = _j.loads("{{vars.kartei_check_json}}")
    if not kartei.get("ANA"): missing.append("Karteieintrag ANA (Anamnese) fehlt")
    if not kartei.get("BEF"): missing.append("Karteieintrag BEF (Befund) fehlt")
    if not kartei.get("BES"): missing.append("Karteieintrag BES (Besuch) fehlt")
    scheine = _j.loads("{{vars.kv_scheine_json}}")
    if not isinstance(scheine, list) or len(scheine) == 0:
        missing.append("Schein fehlt (kein KV-Schein vorhanden)")
    ebm = _j.loads("{{vars.ebm_leistungen_json}}")
    if not isinstance(ebm, list) or len(ebm) == 0:
        missing.append("EBM-Ziffern fehlen (keine Leistungen auf dem Schein)")
    # 01100 late-arrival check
    check_01100 = _j.loads("{{vars.check_01100_json}}")
    if check_01100.get("erforderlich"):
        # check whether 01100 (ebmKatalogEintrag.ident == 1) is already on the schein
        ebm_list = ebm if isinstance(ebm, list) else []
        has_01100 = any(
            (e.get("ebmKatalogEintrag") or {}).get("ident") == 1
            for e in ebm_list
        )
        if not has_01100:
            missing.append("EBM 01100 fehlt (" + check_01100.get("reason", "") + ")")
    result = missing
except Exception as e:
    result = ["Fehler bei der Prüfung: " + str(e)]
```

---

#### PythonCode: `pc-tomedo-check-hzv-vollstaendigkeit` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Checks completeness for an HZV-Patient.
# Required: Diagnose · ANA + BEF + BES in Kartei · HZV-Schein vorhanden · HZV-Ziffern auf Schein.
# HZV-Ziffern detection: scheinart contains "hzv" OR ebmLeistungen non-empty on an HZV-Schein.
# Inputs (all JSON strings baked in by IBS):
#   {{vars.diagnosen_json}}       — diagnosen[] array from patientenDetailsRelationen
#   {{vars.kartei_check_json}}    — output of pc-tomedo-check-kartei-vollstaendigkeit
#   {{vars.kv_scheine_json}}      — kvScheine[] array from patientenDetailsRelationen
#   {{vars.ebm_leistungen_json}}  — ebmLeistungen[] array from kvschein
#   {{vars.scheinart}}            — scheinart string from kvschein (for HZV confirmation)
# Returns list of missing item strings (empty list = vollständig).
import json as _j
try:
    missing = []
    diagnosen = _j.loads("{{vars.diagnosen_json}}")
    if not isinstance(diagnosen, list) or len(diagnosen) == 0:
        missing.append("Diagnose fehlt")
    kartei = _j.loads("{{vars.kartei_check_json}}")
    if not kartei.get("ANA"): missing.append("Karteieintrag ANA (Anamnese) fehlt")
    if not kartei.get("BEF"): missing.append("Karteieintrag BEF (Befund) fehlt")
    if not kartei.get("BES"): missing.append("Karteieintrag BES (Besuch) fehlt")
    scheine = _j.loads("{{vars.kv_scheine_json}}")
    scheinart = "{{vars.scheinart}}".lower()
    if not isinstance(scheine, list) or len(scheine) == 0:
        missing.append("HZV-Schein fehlt (kein Schein vorhanden)")
    elif "hzv" not in scheinart:
        missing.append("HZV-Schein fehlt (vorhandener Schein ist kein HZV-Schein)")
    ebm = _j.loads("{{vars.ebm_leistungen_json}}")
    if not isinstance(ebm, list) or len(ebm) == 0:
        missing.append("HZV-Ziffern fehlen (keine Leistungen auf dem HZV-Schein)")
    result = missing
except Exception as e:
    result = ["Fehler bei der Prüfung: " + str(e)]
```

---

#### PythonCode: `pc-tomedo-extract-scheinart` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts the scheinart string from a kvschein response body.
# {{vars.body}} — raw JSON string from GET /kvschein/{ident}.
# Returns the scheinart string in lowercase, or "" if not present.
import json as _j
try:
    data = _j.loads("{{vars.body}}")
    scheinart = (data.get("scheinart") or data.get("scheinArt") or "")
    result = scheinart.lower() if isinstance(scheinart, str) else str(scheinart).lower()
except Exception as e:
    result = ""
```

---

#### PythonCode: `pc-tomedo-extract-leistungen-from-schein` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts ebmLeistungen[] and goaeLeistungen[] arrays from a kvschein body.
# {{vars.body}}        — raw JSON string from GET /kvschein/{ident}
# {{vars.leistung_typ}} — "ebm" or "goae" (which array to return)
# Returns the requested array (list of Leistung objects), or [] if absent.
import json as _j
try:
    data = _j.loads("{{vars.body}}")
    typ = "{{vars.leistung_typ}}".lower()
    if typ == "goae":
        arr = data.get("goaeLeistungen") or data.get("goaeleistungen") or []
    else:
        arr = data.get("ebmLeistungen")  or data.get("ebmleistungen")  or []
    result = arr if isinstance(arr, list) else []
except Exception as e:
    result = []
```

---

#### PythonCode: `pc-tomedo-extract-diagnosen-from-relations` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts diagnosen[] array from a patientenDetailsRelationen response body.
# {{vars.body}} — raw JSON string from GET /patient/{id}/patientenDetailsRelationen
# Returns the diagnosen[] array (list), or [] if absent.
import json as _j
try:
    data = _j.loads("{{vars.body}}")
    diagnosen = data.get("diagnosen") or []
    result = diagnosen if isinstance(diagnosen, list) else []
except Exception as e:
    result = []
```

---

#### PythonCode: `pc-tomedo-extract-karteieintraege-from-relations` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts karteiEintraege[] array from a patientenDetailsRelationen response body.
# {{vars.body}} — raw JSON string from GET /patient/{id}/patientenDetailsRelationen
# Returns the karteiEintraege[] array (list), or [] if absent.
import json as _j
try:
    data = _j.loads("{{vars.body}}")
    eintraege = data.get("karteiEintraege") or data.get("karteieintraege") or []
    result = eintraege if isinstance(eintraege, list) else []
except Exception as e:
    result = []
```

---

#### PythonCode: `pc-tomedo-extract-kvscheine-from-relations` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Extracts kvScheine[] array (and the first schein ident) from a
# patientenDetailsRelationen response body.
# {{vars.body}} — raw JSON string from GET /patient/{id}/patientenDetailsRelationen
# Returns a dict: {"scheine": [...], "first_ident": N_or_null}
import json as _j
try:
    data   = _j.loads("{{vars.body}}")
    scheine = data.get("kvScheine") or data.get("kvscheine") or []
    if not isinstance(scheine, list):
        scheine = []
    first = scheine[0].get("ident") if scheine else None
    result = {"scheine": scheine, "first_ident": first}
except Exception as e:
    result = {"scheine": [], "first_ident": None, "error": str(e)}
```

---

#### PythonCode: `pc-tomedo-format-audit-bericht` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Compiles the per-patient audit results into a single formatted chat report.
# {{vars.audit_results_json}} — JSON array of per-patient result dicts:
#   [{patient_id, name, dob, insurance_type, ankunft, abgang, missing: [strings]}]
# Patients with empty missing[] are omitted (vollständig).
# Returns a formatted German-language report string ready to send to chat,
# or "✅ Alle Patienten vollständig dokumentiert." if no issues found.
import json as _j
try:
    patients = _j.loads("{{vars.audit_results_json}}")
    if not isinstance(patients, list):
        result = "Fehler: audit_results_json ist keine Liste"
    else:
        lines = []
        for p in patients:
            missing = p.get("missing") or []
            if not missing:
                continue
            pid   = p.get("patient_id", "?")
            name  = p.get("name", "Unbekannt")
            dob   = p.get("dob", "")
            ins   = p.get("insurance_type", "?")
            ankunft = p.get("ankunft", "")
            abgang  = p.get("abgang", "")
            header = f"⚠️ [{ins}] {name} (ID {pid}, *{dob})"
            if ankunft:
                header += f"  Ankunft: {ankunft}"
            if abgang:
                header += f"  Abgang: {abgang}"
            for m in missing:
                lines.append(f"  • {m}")
            lines.insert(-len(missing), header)
        if not lines:
            result = "✅ Alle Patienten vollständig dokumentiert."
        else:
            result = "📋 Abenddokumentation-Audit\n\n" + "\n".join(lines)
except Exception as e:
    result = "Fehler beim Formatieren des Berichts: " + str(e)
```

---

#### PythonCode: `pc-tomedo-build-today-date` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Returns today's date as a YYYY-MM-DD string (local time on the BrassClaw host).
# Used by the audit recipe to compute datum_ymd without LLM involvement.
# No inputs needed — uses the system clock at execution time.
import datetime as _dt
result = _dt.date.today().strftime("%Y-%m-%d")
```

---

#### PythonCode: `pc-tomedo-check-01100-erforderlich` (class 22)

```python
# Channel: orchestrator | Class: 22 — pure logic, no __execute_action__
# Checks whether EBM 01100 (Unvorhergesehene Inanspruchnahme) must be added
# to the KV-Schein based on the patient's arrival time (ankunft).
#
# Rule (GKV-only — do NOT apply to Privat or HZV):
#   Monday (weekday 0):  ankunft local time > 20:00 → 01100 required
#   Tue–Sun (weekday 1–6): ankunft local time > 19:00 → 01100 required
#
# Inputs (IBS bakes in before execution):
#   {{vars.ankunft_ms}}   — ankunft epoch ms (UTC) from besuch API response
#   {{vars.kv_fall}}      — "true"/"false" string
#   {{vars.privat_fall}}  — "true"/"false" string
#   {{vars.scheinart}}    — scheinart string from kvschein (for HZV exclusion)
#
# Returns dict:
#   {"erforderlich": bool, "reason": str}
#   erforderlich=True  → 01100 must be added
#   erforderlich=False → not required (with reason string)
#
# Timezone: epoch ms from tomedo API is UTC. Server runs CEST (UTC+2 summer,
# UTC+1 winter). Use Europe/Berlin for correct local-time comparison.
import datetime as _dt
try:
    import zoneinfo as _zi
    _tz = _zi.ZoneInfo("Europe/Berlin")
except ImportError:
    # fallback: assume UTC+2 (CEST) — valid for summer months
    _tz = _dt.timezone(_dt.timedelta(hours=2))

_kv     = "{{vars.kv_fall}}".lower()    == "true"
_privat = "{{vars.privat_fall}}".lower() == "true"
_sa     = "{{vars.scheinart}}".lower()
_ms     = int("{{vars.ankunft_ms}}" or "0")

if not _kv or _privat:
    result = {"erforderlich": False, "reason": "Nicht GKV (Privat oder kein KV-Fall)"}
elif "hzv" in _sa:
    result = {"erforderlich": False, "reason": "HZV-Patient — 01100 nicht anwendbar"}
elif _ms == 0:
    result = {"erforderlich": False, "reason": "ankunft_ms fehlt oder null"}
else:
    _local = _dt.datetime.fromtimestamp(_ms / 1000, tz=_tz)
    _wday  = _local.weekday()       # 0=Montag, 6=Sonntag
    _h     = _local.hour
    _m     = _local.minute
    _hm    = _h * 60 + _m           # minutes since midnight (local)
    _threshold = 20 * 60 if _wday == 0 else 19 * 60  # 20:00 Mon, 19:00 Tue-Sun
    if _hm > _threshold:
        _tag = ["Mo","Di","Mi","Do","Fr","Sa","So"][_wday]
        _limit = "20:00" if _wday == 0 else "19:00"
        result = {
            "erforderlich": True,
            "reason": f"GKV-Ankunft {_tag} {_local.strftime('%H:%M')} > {_limit} → 01100 erforderlich"
        }
    else:
        _tag = ["Mo","Di","Mi","Do","Fr","Sa","So"][_wday]
        _limit = "20:00" if _wday == 0 else "19:00"
        result = {
            "erforderlich": False,
            "reason": f"Ankunft {_tag} {_local.strftime('%H:%M')} ≤ {_limit} — kein Notfalleinsatz"
        }
except Exception as e:
    result = {"erforderlich": False, "reason": "Fehler: " + str(e)}
```

---

### Step 8.4 — Leaf Skills (class 1) for Abenddokumentation-Audit

One leaf skill per distinct approach. Each references exactly the components needed
for its specific sub-task. No LLM involvement in any of these skills.

---

#### Leaf Skill: `skill-tomedo-tagesliste-get` (class 1)

```
name:        "skill-tomedo-tagesliste-get"
class_code:  1
description: "Leaf skill: fetch the day's Termin/Besuch list for a given date — Tier 0."
body: |
  Fetch today's patient list (Tagesliste) for the given date (YYYY-MM-DD).

  PRIMARY: Use pc-tomedo-tagesliste-get:
    GET {{vars.tomedo_base_url}}/termin?datum={{vars.datum_ymd}}&flach=true
    Returns flat array of Termin objects with patient.ident, beginn, ende.

  FALLBACK (if primary returns 404 or empty): Use pc-tomedo-besuch-tagesliste-get:
    GET {{vars.tomedo_base_url}}/besuch/tagesliste?datum={{vars.datum_ymd}}
    Returns Besuch objects with patient.ident, ankunft, abgang, kvFall, privatFall.

  After fetching: use pc-tomedo-parse-tagesliste to extract unique patient IDs
  with their arrival/departure times and insurance flags.

  datum_ymd is computed by pc-tomedo-build-today-date — no LLM needed.
  ⚠️ Both endpoints are pending live validation on this server.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "pending"
```

---

#### Leaf Skill: `skill-tomedo-karteieintragtyp-list` (class 1)

```
name:        "skill-tomedo-karteieintragtyp-list"
class_code:  1
description: "Leaf skill: fetch all KarteiEintragTyp records to resolve unknown idents (e.g. ANA) — Tier 0, run once during setup."
body: |
  Fetch the complete KarteiEintragTyp list from GET {{vars.tomedo_base_url}}/karteieintragtyp.
  Use pc-tomedo-karteieintragtyp-list.
  Each entry contains ident (int) and kuerzel (e.g. "ANA", "BEF", "BES").
  Record the ANA ident and set it as config key `tomedo_ana_typ_ident` before
  running the Abenddokumentation-Audit.
  ⚠️ Endpoint pending validation on this server.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "pending"
```

---

#### Leaf Skill: `skill-tomedo-abend-audit-fetch-patient` (class 1)

```
name:        "skill-tomedo-abend-audit-fetch-patient"
class_code:  1
description: "Leaf skill: fetch all audit-relevant data for one patient — Tier 0. One patientenDetailsRelationen call + one kvschein call."
body: |
  For patient {{vars.patient_id}}, fetch all data needed for the completeness audit:

  STEP 1 — patientenDetailsRelationen (pc-tomedo-patient-relations-audit):
    GET /patient/{{vars.patient_id}}/patientenDetailsRelationen
      ?limitScheine=true&limitKartei=100&limitVerordnungen=0
      &limitZeiterfassungen=false&limitBehandlungsfaelle=false
    Strips control characters from response body.
    Provides: diagnosen[], karteiEintraege[], kvScheine[].

  STEP 2 — Extract arrays using pure-logic helpers:
    pc-tomedo-extract-diagnosen-from-relations     → diagnosen[]
    pc-tomedo-extract-karteieintraege-from-relations → karteiEintraege[]
    pc-tomedo-extract-kvscheine-from-relations     → kvScheine[] + first_ident

  STEP 3 — KV-Schein fetch (pc-tomedo-kvschein-audit), only if first_ident is non-null:
    GET /kvschein/{{vars.schein_ident}}
    Provides: ebmLeistungen[], goaeLeistungen[], scheinart.

  STEP 4 — Extract from schein (pure-logic helpers):
    pc-tomedo-extract-scheinart                    → scheinart string
    pc-tomedo-extract-leistungen-from-schein (ebm) → ebmLeistungen[]
    pc-tomedo-extract-leistungen-from-schein (goae)→ goaeLeistungen[]

  ⚠️ Do NOT call /leistung?patient=X or /patient/{id}/leistungen — server crash.
  The kvschein path is the ONLY safe Leistungen read path.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

#### Leaf Skill: `skill-tomedo-abend-audit-check-privat` (class 1)

```
name:        "skill-tomedo-abend-audit-check-privat"
class_code:  1
description: "Leaf skill: check documentation completeness for one Privatpatient — Tier 0."
body: |
  Check completeness for a Privatpatient using pc-tomedo-check-privat-vollstaendigkeit.

  Required for Privatpatienten:
    ✓ Diagnose (diagnosen[] non-empty)
    ✓ Karteieintrag ANA (Anamnese)
    ✓ Karteieintrag BEF (Befund, ident=2)
    ✓ Karteieintrag BES (Besuch, ident=18)
    ✓ Rechnung vorhanden (goaeLeistungen[] non-empty on the Schein)

  Inputs needed (from skill-tomedo-abend-audit-fetch-patient output):
    diagnosen_json       — JSON array string of diagnosen[]
    kartei_check_json    — JSON dict from pc-tomedo-check-kartei-vollstaendigkeit
    goae_leistungen_json — JSON array string of goaeLeistungen[]

  Kartei check must be run first via pc-tomedo-check-kartei-vollstaendigkeit
  (pass ana_typ_ident from config key `tomedo_ana_typ_ident`, or "0" to use kuerzel fallback).

  Returns list of missing item strings. Empty list = vollständig.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

#### Leaf Skill: `skill-tomedo-abend-audit-check-gkv` (class 1)

```
name:        "skill-tomedo-abend-audit-check-gkv"
class_code:  1
description: "Leaf skill: check documentation completeness for one GKV-Patient — Tier 0."
body: |
  Check completeness for a GKV-Patient using pc-tomedo-check-gkv-vollstaendigkeit.

  Required for GKV-Patienten:
    ✓ Diagnose (diagnosen[] non-empty)
    ✓ Karteieintrag ANA (Anamnese)
    ✓ Karteieintrag BEF (Befund, ident=2)
    ✓ Karteieintrag BES (Besuch, ident=18)
    ✓ Schein vorhanden (kvScheine[] non-empty)
    ✓ EBM-Ziffern auf dem Schein (ebmLeistungen[] non-empty)

  Inputs needed (from skill-tomedo-abend-audit-fetch-patient output):
    diagnosen_json      — JSON array string of diagnosen[]
    kartei_check_json   — JSON dict from pc-tomedo-check-kartei-vollstaendigkeit
    kv_scheine_json     — JSON array string of kvScheine[]
    ebm_leistungen_json — JSON array string of ebmLeistungen[]

  Returns list of missing item strings. Empty list = vollständig.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

#### Leaf Skill: `skill-tomedo-abend-audit-check-hzv` (class 1)

```
name:        "skill-tomedo-abend-audit-check-hzv"
class_code:  1
description: "Leaf skill: check documentation completeness for one HZV-Patient — Tier 0."
body: |
  Check completeness for an HZV-Patient using pc-tomedo-check-hzv-vollstaendigkeit.

  Required for HZV-Patienten:
    ✓ Diagnose (diagnosen[] non-empty)
    ✓ Karteieintrag ANA (Anamnese)
    ✓ Karteieintrag BEF (Befund, ident=2)
    ✓ Karteieintrag BES (Besuch, ident=18)
    ✓ HZV-Schein vorhanden (kvScheine[] non-empty AND scheinart contains "hzv")
    ✓ HZV-Ziffern auf dem Schein (ebmLeistungen[] non-empty on the HZV-Schein)

  HZV detection: scheinart field from kvschein contains "hzv" (case-insensitive).
  Reference: https://www.haevbw.de/HZV-Gegenueberstellung.pdf

  Inputs needed (from skill-tomedo-abend-audit-fetch-patient output):
    diagnosen_json      — JSON array string of diagnosen[]
    kartei_check_json   — JSON dict from pc-tomedo-check-kartei-vollstaendigkeit
    kv_scheine_json     — JSON array string of kvScheine[]
    ebm_leistungen_json — JSON array string of ebmLeistungen[]
    scheinart           — scheinart string from kvschein

  Returns list of missing item strings. Empty list = vollständig.
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

#### Leaf Skill: `skill-tomedo-abend-audit-auto-add-01100` (class 1)

```
name:        "skill-tomedo-abend-audit-auto-add-01100"
class_code:  1
description: "Leaf skill: auto-add EBM 01100 to a GKV patient's KV-Schein during the evening audit when the late-arrival rule is triggered and 01100 is not yet present — Tier 0 (no LLM, all inputs deterministic)."
body: |
  Automatically add EBM 01100 (Unvorhergesehene Inanspruchnahme, ebmKatalogEintrag.ident=1)
  to a GKV patient's KV-Schein when all of these are true:
    1. insurance_type == "GKV" (not Privat, not HZV)
    2. pc-tomedo-check-01100-erforderlich returned erforderlich=true
    3. EBM 01100 (ebmKatalogEintrag.ident==1) is NOT already in ebmLeistungen[]

  Only run when the above conditions are confirmed. Do not run for Privat or HZV patients.

  STEP 1 — POST /ebmleistung (pc-tomedo-ebmleistung-create):
    Body vars required:
      tomedo_base_url         — from config
      body_json               — JSON string with all mandatory fields:
        {
          "datum":                  <ankunft_ms>,          ← use patient's ankunft_ms as datum
          "visible":                true,
          "anzahl":                 1,
          "ebmKatalogEintrag":      {"ident": 1},          ← 01100, always ident=1 on this server
          "leistungserbringer":     {"ident": <nutzer>},   ← from config key tomedo_nutzer_ident
          "betriebsstaette":        {"ident": 1},
          "dokumentierenderNutzer": {"ident": <nutzer>},
          "letzterNutzer":          {"ident": <nutzer>},
          "abrechnenderArzt":       {"ident": <nutzer>}
        }
    Returns: {new_leistung_ident}

  STEP 2 — PUT /kvschein/{schein_ident} (pc-tomedo-kvschein-link-leistung):
    Vars: schein_ident (first kvSchein ident from patientenDetailsRelationen),
          leistung_ident (from step 1 result)
    Returns: HTTP 204 — leistung linked, Mac client sees 01100 immediately.

  After successful write: remove "EBM 01100 fehlt" from this patient's missing[] list
  in the audit report — it has been resolved automatically.

  Config keys required:
    tomedo_nutzer_ident   — default Arzt ident for auto-written Leistungen
    tomedo_base_url       — https://{host}:8443/{db}

  ⚠️ ENDPOINT: POST to /ebmleistung (NOT /leistung) — confirmed live 2026-08-22.
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

---

### Step 8.5 — Recipes (class 21) for Abenddokumentation-Audit

All Tier 0. The orchestrator runs the full audit without LLM involvement.
Intent examples cover both manual trigger ("check documentation now") and
the automated nightly invocation ("run evening audit").

---

#### Recipe: `tomedo-karteieintragtyp-list` (class 21) — Tier 0

> Run this once during initial setup to resolve the ANA KarteiEintragTyp ident.

```
name:              "tomedo-karteieintragtyp-list"
description:       "One-time setup: fetch all KarteiEintragTyp records to resolve the ANA ident. Record the result in config key tomedo_ana_typ_ident before running the Abenddokumentation-Audit."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-karteieintragtyp-list>", "<uuid:skill-tomedo>"],
    "label":   "Load karteieintragtyp-list leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-karteieintragtyp-list>"],
    "label":   "Pre-load ts-tomedo-karteieintragtyp-list binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-karteieintragtyp-list>"],
    "label":   "Execute: GET /karteieintragtyp → full list of entry types with idents and kürzel"
  }
]
intent_examples: [
  {"input": "welche karteieintrag typen gibt es",              "class": 2},
  {"input": "karteieintragtyp liste abrufen",                  "class": 2},
  {"input": "ANA ident herausfinden",                          "class": 3},
  {"input": "list all kartei entry types",                     "class": 2},
  {"input": "fetch karteieintragtyp catalog",                  "class": 2},
  {"input": "welchen ident hat ANA anamnese",                  "class": 3},
  {"input": "karteieintrag typ idents nachschlagen",           "class": 2},
  {"input": "all entry type idents tomedo",                    "class": 2},
  {"input": "karteitypen auflisten",                           "class": 2},
  {"input": "setup audit karteieintragtyp",                    "class": 3}
]
source: "system"
validation_status: "pending"
```

---

#### Recipe: `tomedo-tagesliste-get` (class 21) — Tier 0

```
name:              "tomedo-tagesliste-get"
description:       "Fetch the Tagesliste (day schedule) for a given date — returns unique patient IDs, names, arrival/departure times, and insurance flags. Uses termin?datum endpoint with besuch/tagesliste as fallback. date is computed automatically from today if not supplied."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-tagesliste-get>", "<uuid:skill-tomedo>"],
    "label":   "Load tagesliste leaf + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-build-today-date>"],
    "label":   "Compute today's date as YYYY-MM-DD string — no LLM needed"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-tagesliste-get>"],
    "label":   "Pre-load ts-tomedo-tagesliste-get binding (primary: termin?datum)"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-tagesliste-get>"],
    "label":   "Execute: GET /termin?datum={today}&flach=true → day Termin list"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-besuch-tagesliste-get>"],
    "label":   "Pre-load ts-tomedo-besuch-tagesliste-get binding (fallback: besuch/tagesliste)"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-besuch-tagesliste-get>"],
    "label":   "Fallback execute: GET /besuch/tagesliste?datum={today} if primary returned 404/empty"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-parse-tagesliste>"],
    "label":   "Parse: deduplicate patients, extract IDs, arrival/departure, kvFall/privatFall flags"
  }
]
intent_examples: [
  {"input": "tagesliste heute",                                "class": 2},
  {"input": "welche patienten waren heute da",                 "class": 3},
  {"input": "heutige patientenliste",                          "class": 2},
  {"input": "day schedule today",                              "class": 2},
  {"input": "patienten von heute abrufen",                     "class": 2},
  {"input": "liste der heutigen patienten",                    "class": 2},
  {"input": "today's patient list tomedo",                     "class": 2},
  {"input": "alle patienten die heute behandelt wurden",       "class": 3},
  {"input": "wer war heute in der praxis",                     "class": 3},
  {"input": "tagesliste abrufen",                              "class": 2},
  {"input": "fetch today's schedule",                          "class": 2},
  {"input": "heutige termine patienten",                       "class": 2}
]
source: "system"
validation_status: "pending"
```

---

#### Recipe: `tomedo-abend-audit-fetch-patient` (class 21) — Tier 0

> Per-patient data fetch sub-recipe. Called once per patient during the audit.
> Fetches patientenDetailsRelationen + first kvSchein.

```
name:              "tomedo-abend-audit-fetch-patient"
description:       "Fetch all audit-relevant data for one patient: patientenDetailsRelationen (diagnosen, karteiEintraege, kvScheine) and the first kvSchein (ebmLeistungen, goaeLeistungen, scheinart). One patientenDetailsRelationen call + one kvSchein call. No LLM."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-tomedo-abend-audit-fetch-patient>", "<uuid:skill-tomedo>"],
    "label":   "Load audit-fetch-patient leaf + domain skill"
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
    "include": ["<uuid:pc-tomedo-patient-relations-audit>"],
    "label":   "Execute: GET /patient/{id}/patientenDetailsRelationen → diagnosen, karteiEintraege, kvScheine"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-diagnosen-from-relations>"],
    "label":   "Extract diagnosen[] array from relations body"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-karteieintraege-from-relations>"],
    "label":   "Extract karteiEintraege[] array from relations body"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-kvscheine-from-relations>"],
    "label":   "Extract kvScheine[] array and first schein ident from relations body"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-kvschein-get>"],
    "label":   "Pre-load ts-tomedo-kvschein-get binding"
  },
  {
    "step_id": "step-7",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-kvschein-audit>"],
    "label":   "Execute: GET /kvschein/{first_ident} → ebmLeistungen, goaeLeistungen, scheinart (skipped if no schein)"
  },
  {
    "step_id": "step-8",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-scheinart>"],
    "label":   "Extract scheinart string from kvschein body"
  },
  {
    "step_id": "step-9",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-leistungen-from-schein>"],
    "label":   "Extract ebmLeistungen[] from kvschein body (leistung_typ=ebm)"
  },
  {
    "step_id": "step-10",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-leistungen-from-schein>"],
    "label":   "Extract goaeLeistungen[] from kvschein body (leistung_typ=goae)"
  }
]
intent_examples: [
  {"input": "audit daten für patient laden",                   "class": 3},
  {"input": "fetch patient audit data",                        "class": 2},
  {"input": "patientendaten für dokumentationsprüfung",        "class": 3},
  {"input": "load all data for patient completeness check",    "class": 2},
  {"input": "patient daten für abenddokumentation",            "class": 3},
  {"input": "diagnosen kartei schein für patient abrufen",     "class": 3},
  {"input": "fetch relations and schein for patient",          "class": 2},
  {"input": "patient audit datenabruf",                        "class": 2},
  {"input": "vollständige patientendaten für prüfung",         "class": 3},
  {"input": "audit fetch single patient",                      "class": 2}
]
source: "system"
validation_status: "validated"
```

---

#### Recipe: `tomedo-abend-audit-check-patient` (class 21) — Tier 0

> Per-patient completeness check. One recipe per insurance type is intentionally
> avoided — instead this single recipe classifies and then dispatches to the
> correct check helper, keeping the call surface minimal.

```
name:              "tomedo-abend-audit-check-patient"
description:       "Run the documentation completeness check for one patient. Classifies insurance type (Privat/GKV/HZV), checks kartei entries, evaluates 01100 late-arrival rule for GKV, then applies insurance-specific completeness rule. Returns a list of missing items. No LLM."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": [
      "<uuid:skill-tomedo-abend-audit-check-privat>",
      "<uuid:skill-tomedo-abend-audit-check-gkv>",
      "<uuid:skill-tomedo-abend-audit-check-hzv>",
      "<uuid:skill-tomedo>"
    ],
    "label":   "Load all three completeness-check leaf skills + domain skill"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-kartei-vollstaendigkeit>"],
    "label":   "Check karteiEintraege for ANA, BEF, BES presence (pure logic)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-classify-insurance>"],
    "label":   "Classify insurance type: Privat | GKV | HZV from privat_fall, kv_fall, scheinart"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-01100-erforderlich>"],
    "label":   "GKV: evaluate 01100 late-arrival rule from ankunft_ms + kv_fall + scheinart (pure logic)"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-privat-vollstaendigkeit>"],
    "label":   "Check Privat completeness (diagnose + kartei ANA/BEF/BES + GOÄ-Leistungen)"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-gkv-vollstaendigkeit>"],
    "label":   "Check GKV completeness (diagnose + kartei + schein + EBM-Ziffern + 01100 if erforderlich)"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-hzv-vollstaendigkeit>"],
    "label":   "Check HZV completeness (diagnose + kartei ANA/BEF/BES + HZV-Schein + HZV-Ziffern)"
  }
]
intent_examples: [
  {"input": "dokumentation prüfen für patient",               "class": 3},
  {"input": "check patient documentation completeness",       "class": 2},
  {"input": "vollständigkeitsprüfung patient",                "class": 3},
  {"input": "is patient documentation complete",              "class": 2},
  {"input": "fehlende dokumentation patient prüfen",          "class": 3},
  {"input": "run completeness check for patient",             "class": 2},
  {"input": "kartei schein diagnose vorhanden prüfen",        "class": 3},
  {"input": "patient dokumentation audit einzeln",            "class": 3},
  {"input": "check single patient for missing docs",          "class": 2},
  {"input": "einzelpatient dokumentationsprüfung",            "class": 3}
]
source: "system"
validation_status: "validated"
```

---

#### Recipe: `tomedo-abend-audit` (class 21) — Tier 0

> Top-level evening audit. Runs fully automated — no LLM, no user interaction.
> Designed for a scheduled nightly trigger (e.g. cron at 20:00).
> Reports only patients with missing documentation.

```
name:              "tomedo-abend-audit"
description:       "Automated evening documentation audit: fetch today's patient list, classify by insurance type (Privat/GKV/HZV), check each patient for documentation completeness (diagnose, karteiEintraege ANA/BEF/BES, schein, leistungen, 01100 late-arrival rule), auto-add EBM 01100 to GKV Schein if required and missing, then send a report to chat. Fully orchestrator-driven — no LLM."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": [
      "<uuid:skill-tomedo-tagesliste-get>",
      "<uuid:skill-tomedo-abend-audit-fetch-patient>",
      "<uuid:skill-tomedo-abend-audit-check-privat>",
      "<uuid:skill-tomedo-abend-audit-check-gkv>",
      "<uuid:skill-tomedo-abend-audit-check-hzv>",
      "<uuid:skill-tomedo-abend-audit-auto-add-01100>",
      "<uuid:skill-tomedo>"
    ],
    "label":   "Load all audit leaf skills (incl. auto-add-01100) + domain skill into orchestrator context"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-build-today-date>"],
    "label":   "Compute today's date as YYYY-MM-DD (no LLM — pure Python datetime)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-tagesliste-get>"],
    "label":   "Pre-load ts-tomedo-tagesliste-get binding (primary day-list endpoint)"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-tagesliste-get>"],
    "label":   "Execute: GET /termin?datum={today}&flach=true → day Termin list"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-besuch-tagesliste-get>"],
    "label":   "Pre-load ts-tomedo-besuch-tagesliste-get binding (fallback day-list endpoint)"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-besuch-tagesliste-get>"],
    "label":   "Fallback execute: GET /besuch/tagesliste?datum={today} if primary returned 404/empty"
  },
  {
    "step_id": "step-6",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-parse-tagesliste>"],
    "label":   "Parse day list → unique patient IDs with arrival/departure and insurance flags"
  },
  {
    "step_id": "step-7",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-patient-relations>"],
    "label":   "Pre-load ts-tomedo-patient-relations binding (used per-patient in loop)"
  },
  {
    "step_id": "step-8",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-patient-relations-audit>"],
    "label":   "Per-patient loop: fetch /patientenDetailsRelationen → diagnosen, karteiEintraege, kvScheine"
  },
  {
    "step_id": "step-9",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-diagnosen-from-relations>"],
    "label":   "Per-patient: extract diagnosen[] array"
  },
  {
    "step_id": "step-10",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-karteieintraege-from-relations>"],
    "label":   "Per-patient: extract karteiEintraege[] array"
  },
  {
    "step_id": "step-11",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-kvscheine-from-relations>"],
    "label":   "Per-patient: extract kvScheine[] and first schein ident"
  },
  {
    "step_id": "step-12",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-kvschein-get>"],
    "label":   "Pre-load ts-tomedo-kvschein-get binding (used per-patient in loop)"
  },
  {
    "step_id": "step-13",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-kvschein-audit>"],
    "label":   "Per-patient: fetch /kvschein/{ident} → ebmLeistungen, goaeLeistungen, scheinart"
  },
  {
    "step_id": "step-14",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-scheinart>"],
    "label":   "Per-patient: extract scheinart string"
  },
  {
    "step_id": "step-15",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-leistungen-from-schein>"],
    "label":   "Per-patient: extract ebmLeistungen[] (leistung_typ=ebm)"
  },
  {
    "step_id": "step-16",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-extract-leistungen-from-schein>"],
    "label":   "Per-patient: extract goaeLeistungen[] (leistung_typ=goae)"
  },
  {
    "step_id": "step-17",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-kartei-vollstaendigkeit>"],
    "label":   "Per-patient: check karteiEintraege for ANA/BEF/BES presence"
  },
  {
    "step_id": "step-18",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-classify-insurance>"],
    "label":   "Per-patient: classify insurance type → Privat | GKV | HZV"
  },
  {
    "step_id": "step-19",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-privat-vollstaendigkeit>"],
    "label":   "Per-patient (Privat only): check diagnose + kartei + GOÄ-Leistungen"
  },
  {
    "step_id": "step-19b",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-01100-erforderlich>"],
    "label":   "Per-patient (GKV only): evaluate 01100 late-arrival rule from ankunft_ms + kv_fall + scheinart"
  },
  {
    "step_id": "step-20",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-gkv-vollstaendigkeit>"],
    "label":   "Per-patient (GKV only): check diagnose + kartei + schein + EBM-Ziffern + 01100 if erforderlich"
  },
  {
    "step_id": "step-21",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-check-hzv-vollstaendigkeit>"],
    "label":   "Per-patient (HZV only): check diagnose + kartei + HZV-Schein + HZV-Ziffern"
  },
  {
    "step_id": "step-21b",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-ebmleistung-create>"],
    "label":   "Pre-load ts-tomedo-ebmleistung-create binding (used for auto-add 01100)"
  },
  {
    "step_id": "step-21c",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-ebmleistung-create>"],
    "label":   "Per-patient (GKV, 01100 erforderlich + fehlt): POST /ebmleistung → {new_leistung_ident}"
  },
  {
    "step_id": "step-21d",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-tomedo-kvschein-link-leistung>"],
    "label":   "Pre-load ts-tomedo-kvschein-link-leistung binding (used for auto-add 01100)"
  },
  {
    "step_id": "step-21e",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-kvschein-link-leistung>"],
    "label":   "Per-patient (GKV, 01100 erforderlich + fehlt): PUT /kvschein/{id} → links leistung, Mac client notified"
  },
  {
    "step_id": "step-22",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-tomedo-format-audit-bericht>"],
    "label":   "Format audit report: list patients with remaining missing items → send to chat"
  }
]
intent_examples: [
  {"input": "abenddokumentation prüfen",                       "class": 2},
  {"input": "abend audit starten",                             "class": 2},
  {"input": "run evening documentation audit",                 "class": 2},
  {"input": "dokumentation heute prüfen",                      "class": 2},
  {"input": "check today's documentation completeness",        "class": 2},
  {"input": "wer hat heute fehlende dokumentation",            "class": 3},
  {"input": "which patients have incomplete documentation",    "class": 2},
  {"input": "vollständigkeitsprüfung alle patienten heute",    "class": 3},
  {"input": "nightly documentation check",                     "class": 2},
  {"input": "evening audit tomedo",                            "class": 2},
  {"input": "fehlende scheine diagnosen karteieinträge",       "class": 3},
  {"input": "abendprüfung dokumentation alle patienten",       "class": 3},
  {"input": "automated audit tonight",                         "class": 2},
  {"input": "privat gkv hzv dokumentation kontrolle",         "class": 3}
]
source: "system"
validation_status: "pending"
```

