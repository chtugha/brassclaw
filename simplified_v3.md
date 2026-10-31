# Simplified v3: Plan zur Vereinfachung von Sicherheitsmodell, Runtime und WebUI

> **Current status — 2026-10-23:** use [the combined implementation and ownership
> index](docs/implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


## Vorrang: Konformität des bereits implementierten Stands herstellen

**Abgleich vom 24.10.2026; vor der nächsten regulären Planfortsetzung ausführen.**
Dieser Abschnitt gleicht die geänderten Verträge der fünf Pläne gegen die bereits
implementierten Teilpfade ab. Er ersetzt weder die bisherigen Implementierungen
noch ihre unveränderlichen historischen Belege. Ausgangspunkt sind der aktuelle
[Implementierungsindex](docs/implementation-status.md), die dort verlinkten
Abnahmen und der geprüfte gemeinsame Arbeitsbaum, einschließlich der privaten
neun Schritte/drei Regeln aus dem [Graph-Prüfschritt](docs/validation/2026-10-24-python-graph-review.md).
Die ursprüngliche Prüfung ist Quell-/Vertragsanalyse; die K1-/K2-Reparaturen
haben inzwischen ihre unten verlinkten begrenzten Abnahmen. K3–K7 sind weiterhin
offen. Ein im Code fehlender Übergang
ist von einem noch nicht vorliegenden Nachweis zu unterscheiden.
K2/K3 betreffen bereits teilweise implementierte Pfade mit zuvor offenen
Integrationen; die präzisierte Reihenfolge macht sie für vorhandene Consumer
vorrangig. Das erklärt ihre früheren begrenzten Abnahmen nicht nachträglich
für falsch und behauptet keine allein durch die Planänderung entstandene Regression.

**Reihenfolge:** K0 festhalten, K1–K6 im angegebenen Owner bearbeiten, K7 schließen;
erst danach die reguläre Remaining-Liste und die nächsten offenen Schritte der
fünf Pläne fortsetzen. Abhängige Änderungen dürfen zu einem kohärenten Paket
gebündelt werden. Eine bloße Zuständigkeitsverschiebung verlangt keine erneute
Implementierung. Bei einem Nachweisabgleich ist eine Codeänderung nur nötig, wenn
der betroffene Pfad den Vertrag tatsächlich verletzt; sonst genügt ein exakt
passender vorhandener oder ergänzender Caller-Nachweis. Bereits laufende Tasks
werden dadurch weder rematcht noch erneut ausgeführt oder automatisch abgebrochen.

### K0 — Ausgangsumfang und Abschlussbelege verbindlich festhalten

Die folgenden Feststellungen in den vorhandenen Implementierungs-/Validierungs-
record übernehmen, mit tatsächlichen Source-/Paket-/Artefaktidentitäten und den
zugehörigen bisherigen Receipts. Keine zweite Statusdatenbank oder neue Queue.
Laufende parallele Änderungen beim Beginn jedes betroffenen Pakets abgleichen;
keine älteren Quellen über den gemeinsamen Arbeitsbaum kopieren.

| Plan / bereits implementierter Teil | Ergebnis des Abgleichs / vorrangige Folge |
| --- | --- |
| Simplified: globaler Dienst, typisierte Auswahl, Live-Settings, native Custody und atomare PG-Aufnahme | Grundlage bleibt erhalten. Neue Produktübergaben betreffen K1–K3; Child/MCP und Artefakt-/Upgradegrenzen benötigen den begrenzten Nachweisabgleich K4/K6. Kein Neubau der Runtime. |
| Validator: unveränderliche Subjects, Source-/Definition-/Contract-/Graphregeln und sealed Observations; exakte menschliche Usage-Q2-Route | Keine durch die Ownership-Änderung belegte Notwendigkeit, diese Bausteine neu zu schreiben. K5 sichert ihre tatsächliche Grenze und die geänderten Paketinputs. Acht produktive Klassenrouten, allgemeine Pure-Approval und deren Writer waren bereits offen. |
| Monty composition: qualifizierter Reply-/History-Pfad, vorhandene authored Aktivierung und begrenzte öffentliche Commands/MCP | Aktivierungsübergang in K2 ergänzen, aktuelle Caller-/Lifecycle-Belege in K4 abgleichen. Bereits qualifizierte Adapter wiederverwenden; keine neue MCP-Ausführungsschicht. |
| Monty implementation: vorhandene Preload-/Binding-/Werteadapter und explizites natives Child-Profil | K4 prüft die präzisierten Isolations-/Parent-/Resultgrenzen. R09-Parentaktivierung, fünf Subagentmigrationen und R10/Doc-Consumer bleiben offene Facharbeit. |
| Prefix: Source-only-Jobdefinitionen, Profile/Originale/Generationen, Messungen und revisionierte Modellgrenzen | Konkrete Identitätslücke K1, bestehende Provider-/Aktivierungsübergaben K2/K3 und Paket-/Retentionsabgleich K5/K6. Quarantäne ist weiterhin keine Serving-Aktivierung. |

### K1 — Bestehende Prefix-Aufnahme auf vollständige Requestidentität korrigieren

> **Stand 24.10.2026, nach dem obigen Abgleich:** K0 und K1 sind für ihren
> beschriebenen Umfang abgeschlossen. [K1-Abnahme](docs/validation/2026-10-24-k1-prefix-request-identity.md)
> belegt 33 tatsächliche Tests, strikte betroffene Lints und exakte Artefakte.
> Die folgenden K1-Befunde/Reparaturschritte erklären den behobenen Ausgangspunkt.
> K2 ist inzwischen separat abgenommen, siehe dessen Abschnitt; K3–K7 bleiben
> vor der regulären Fortsetzung offen.

**Belegte Codelücke; Owner:** gemeinsamer Admissionvertrag in Phase 3a, mit dem
Prefix-Owner für den fachlichen Requestvergleich (§7 des Prefix-Plans).
[submit_prefix_source_snapshot / admit_prefix_definition](crates/brassclaw_reborn_composition/src/prefix_job_submission.rs)
liest bei vorhandener Job-ID die Definition, vergleicht aber im Wiederholungszweig
nicht den erneut übergebenen `snapshot_bytes` mit ihrem gespeicherten Input.
Auch [Store::reserve](crates/brassclaw_reborn_composition/src/prefix_job_definitions.rs)
vergleicht in beiden Konfliktpfaden Request-/Scope-Metadaten, ohne den angebotenen
Input vollständig einzubeziehen. Damit kann gleiche ID mit abweichenden Quellen
statt eines Konflikts den alten Auftrag zurückgeben.

1. Explizit übergebene fachliche Daten unter der ursprünglichen ID vollständig
   vergleichen, einschließlich Snapshot/Input und etwaiger expliziter Auswahl.
   Identische Wiederholung liest Originaldefinition und Originalrun; geänderter
   Inhalt wird vor Aufnahme/Effekten als Konflikt abgewiesen. Dieselbe Prüfung
   vorgefundenen Datensätzen und dem Gewinner eines konkurrierenden Inserts geben.
2. Automatisch erfasste Quellen oder neu verfügbare Compilerrevisionen bei einem
   Transportretry nicht erneut auswählen: die ursprüngliche Auswahl bleibt gültig.
   Reconnect ohne neue Nutzlast ist ein eigener Original-Lookup, kein geänderter
   Submit. Bewusst neue Quellen bekommen eine neue Request-/Jobidentität.
3. Den vorhandenen Definition-/Thread-/internen ResultOnly-/Runpfad erhalten.
   Verlorenes Reserve-/Ensure-/Admission-COMMIT zuerst über Originalidentitäten
   nachlesen; solange das nicht möglich ist, ungeklärt bleiben statt Ersatzjob.
   Interne Transportnachrichten sind keine öffentliche Chat-/Matcheraufnahme.

**Abnahme:** gleiche ID mit gleichen und unterschiedlichen Snapshots, zwei
konkurrierende Inserts mit unterschiedlichen Inputs, Compilerwechsel nach erster
Aufnahme und verlorene Bestätigung. Konflikt verursacht null zusätzliche Runs/
Dispatches; echte Wiederholung liefert ursprüngliche Inputs/Artefakte. Den
Messjobpfad auf dieselbe Vertragslücke prüfen und nur betroffene Vergleiche ändern.

### K2 — Vorhandene Katalogaktivierung mit dauerhafter Invalidierung verbinden

> **Abgeschlossen für vorhandene Consumer, 24.10.2026:**
> [K2-Abnahme](docs/validation/2026-10-24-k2-catalogue-publication.md) belegt die
> atomare Invalidierung V177, Original-Readback, tatsächliche Fehler vor/nach
> COMMIT, sichtbaren Runtime-/Discovery-Uptake, Restart/Overrides und betroffene
> Source-/Mess-/Child-/WebUI-Consumer. 39 frische plus neun genau weiterverwendete
> Rust-Ergebnisse und 21 UI-Fälle bestehen, ebenso strikte Lints und Integrität.
> Die folgenden Befunde erklären den reparierten Ausgangspunkt. Generiertes
> Prefix-Serving, K3–K7 und die Gesamtpläne sind damit nicht abgeschlossen.

**Belegte Übergabelücke; Owner:** Phase 0a/§8.2, Prefix liefert Abhängigkeiten und
seinen Neubau-Consumer, Composition die bestehenden Aktivierungs-/MCP-Consumer.
[activate_authored](crates/brassclaw_reborn_composition/src/authored_monty_catalogue.rs)
publiziert Katalog/Intentdaten/aktiven Pointer und Aktivierungsbeleg transaktional.
Ein dauerhafter betroffener Prefix-Invalidierungsauftrag ist dort noch nicht Teil
dieser Transaktion; Discovery/In-Memory-Publikation folgen nach COMMIT.

1. Betroffene Source-/Prefix-/Consumerreferenzen mit der neuen Kataloggeneration
   über denselben bestehenden Publikationsowner dauerhaft invalidieren bzw. zur
   Verarbeitung markieren. Aktiven Pointer, vollständigen genehmigten Katalog
   und Invalidierungsreferenz gemeinsam committen. Vorhandene Records erweitern,
   falls nötig additiv migrieren; kein zweiter Aktivator oder Jobrunner.
2. Nach ungewissem COMMIT oder fehlgeschlagener Discovery-/Runtimepublikation
   ursprünglichen Aktivierungsbeleg lesen und genau diese Generation abgleichen.
   Sichtbare Ziel-/Effektiv-/Pendingzustände dürfen keinen alten In-Memory-Stand
   als neuen Erfolg ausgeben. Keine neue Aktivierungsidentität zum Reparieren.
3. Prefix aus dem neuen aktiven Snapshot bauen, niemals vor dessen Publikation.
   Neue abhängige Modellcalls bleiben bis zum passenden qualifizierten Prefix
   blockiert; providerfreie Builds/Verwaltung/geeignete Tier-0-Arbeit bleiben frei.
   Vorhandene qualifizierte Inline-/Recipe-Kontexte als solche exakt ausweisen;
   nicht als vollständigen neuen Prefix-Servingpfad deklarieren oder pauschal
   verwerfen. Alte Tasks behalten ihre kompatible Auswahl unter Live-Policy.

**Abnahme:** Aktivierung ohne Provider/Prefix, Crash-/Ackverlust vor/nach COMMIT,
Discoveryfehler nach erfolgreichem DB-Commit, verlorene/doppelte Invalidierung,
Neustart und fehlgeschlagener Build. Keine unpublizierte Generation im Compiler,
kein stiller Rollback, kein stale Modellkontext und kein Tier-2-Replay. Bestehende
[authored Caller-Fälle](crates/brassclaw_reborn_composition/src/runtime/tests/authored_catalogue_tests.rs)
gezielt erweitern statt einen zweiten Aktivierungstestpfad zu bauen.

### K3 — Bestehende Provideränderungen bis zu ihren Verbrauchern quittieren

**Begrenzte zusätzliche Abnahme, 31.10.2026:**
[BUILD045–051](docs/validation/2026-10-31-settings-catalogue-facades.md)
belegt aktuelle Recipe-/ToolSkill-Leseports, atomare Legacy-Recipe-Ablehnung
einschließlich Queue-Konkurrenz und wiederverwendbare Auth-Refresh-Publikation
in einer Aufrufertransaktion. Die bisherigen Original-/Refresh-Zustände bleiben
erhalten; dies ist noch kein nativer Codex-Credential-/Login-/Dispatch-Cutover.
Dieser bleibt in K3/Schritt6 beim bestehenden Provider-/Auth-Owner. Eigenständige
OAuth-Default-/Maximallaufzeiten sind im Quellstand über V197 und den bestehenden
quittierten Settings-/API-/Browserpfad angeschlossen: positive u32, Defaults
600/600, Default höchstens Maximum. Bestehende Originale und native Exact-/Gate-
Fristen bleiben unverändert; neue Browserinputs binden Target und erste Expiry
vor ihren weiteren Aufrufen. Quellstand und tatsächliche begrenzte Abnahme werden
im Implementierungsindex getrennt geführt. Kein neuer Settings-, Credential-,
Workflow- oder Queue-Owner und kein K3-Abschluss.

**Belegte Übergabelücke; Owner:** §10.1/Phase 3a; Prefix und Composition liefern
Verbraucherfälle. [PgProviderRepo](crates/brassclaw_reborn_composition/src/pg_provider_repo.rs)
hat revisioniertes CAS und atomare Deaktivierung/Auswahlbereinigung.
[LlmConfigService](crates/brassclaw_reborn_composition/src/llm_config_service.rs)
committet Definition/ggf. Key/Auswahl und lädt danach laufende Clients neu.
Ungewisser COMMIT und fehlgeschlagenes Reload liefern Unavailable; der vollständige
revisionsbezogene Readback-/Invalidierungs-/Effektivvertrag ist damit nicht erfüllt.

1. Den ursprünglichen Änderungsauftrag und seine bestätigte oder ungeklärte
   Providerrevision über die vorhandene Transaktion rekonstruierbar machen.
   Wiederholtes Nachlesen erzeugt keine neue Revision, Secretmutation oder
   Reaktivierung. Ein späterer fremder Edit darf nicht als eigene Bestätigung
   gelten; Geheimnisse verbleiben im verschlüsselten Hostbereich.
2. Betroffene Modell-/Prefix-/Kohai-/Sempai-Verbraucher über den vorhandenen
   Revisions-/Publikationsmechanismus zuverlässig aktualisieren/invalidieren.
   Verlorene Notification bei Wiederaufnahme nachlesen; kein zweiter Provider-
   katalog und keine neue allgemeine Event-Infrastruktur. Persistierte Revision,
   effektive Revision und fehlgeschlagene/pending Übernahme getrennt zeigen.
3. Vor dem nächsten betroffenen Dispatch aktuelle Deaktivierung, Auth-/Target-
   Kompatibilität und technische Verschärfung prüfen. Alte Clients nicht als
   erfolgreichen Uptake melden. Kein stiller Endpoint-/Modell-/Prefixfallback;
   letzter deaktivierter Provider lässt Verwaltung und providerfreie Arbeit zu.

**Abnahme:** echtes Edit/Deactivate/Re-Add mit verlorenem COMMIT-Reply,
Reloadfehler, parallelem Edit, externem DB-Writer/Neustart und Änderung im Wait.
Die bestehenden [Provider-Caller-Fälle](crates/brassclaw_reborn_composition/src/runtime/tests/provider_request_policy_tests.rs)
und Modelllimit-Fälle gezielt ergänzen. Der noch offene vollständige finale
Render-/Größenguard bleibt beim Prefix-Owner; seine komplette neue Servingpipeline
ist keine Voraussetzung, diesen bestehenden DB-/Dispatchübergang zu reparieren.

**Backend-Wartezeit, Quellpfad 31.10.2026:** Der vorhandene quittierte
`MontyExecutionLimits`-Owner trägt `auth_backend_timeout_millis` für jeden
geschützten ProductAuth-Backend-Aufruf. Die gemeinsame Factory-Anbindung umfasst
Custom-, PostgreSQL- und Dateisystem-Ports; neue Aufrufe übernehmen den wirksamen
Wert, laufende behalten ihre erfasste Frist. V194 erhält explizite Werte und
unabhängige INSERT-Defaults. WebUI-Steuerung und EN/DE-Hilfe gehören zu demselben
Pfad. Ein Timeout bestätigt keinen fehlgeschlagenen Effekt und erlaubt keine
Wiederholung. Quellcode allein ist keine Abnahme: aktuelle native DB-/Aufruf-/
Browser-/Worker-/Lint-Belege stehen im Implementierungsindex. Setup-Laufzeiten,
deren Browser-Expiry-Produzenten und unabhängige Browser-Wartezeiten bleiben
offene Arbeit im bestehenden K3-Schritt6; damit ist K3 nicht geschlossen. Keine
zweite Settings-Verwaltung oder neue Auth-/Workflow-Engine einführen.

**Manuelle Setup-Laufzeiten, Quellpfad 31.10.2026:**
`manual_token_setup_ttl_secs` (600 s) und
`extension_manual_token_setup_ttl_secs` (300 s) ersetzen die festen Vorgaben
für neue manuelle Token-Einrichtungen im vorhandenen quittierten Settings-Owner.
V195 ergänzt fehlende Werte, erhält explizite Vorgaben und verändert keine
Auth-Originale. Die nativen Original-Owner behalten die erste Ablaufzeit bei
Wiederholung und konkurrierender Anlage; der Extension-Adapter liest bestehende
Originale vor neuen Vorgaben. Direkte native Aufrufer mit expliziter Ablaufzeit
behalten ihren Vertrag. WebUI und EN/DE-Hilfe sind angeschlossen; Abnahmebelege
werden im Implementierungsindex geführt. OAuth-Maximum/Browser-Produzent,
Popup- und Browser-Anfragefristen sowie Ingress-Grenzen bleiben offen. Dieser
Quellstand schließt weder K3 noch die Gesamtpläne.

**OAuth-Challenges wartender Tasks, Quellpfad 31.10.2026:**
`turn_gate_oauth_setup_ttl_secs` (600 s, positive u32) ersetzt ausschließlich
den festen Default neuer Google-/DCR-Gate-Originale. Beide Provider verwenden
denselben internen Exact-/GateDefault-Vertrag und den vorhandenen quittierten
Settings-Owner. Das Original wird vor einer neuen Vorgabe nachgelesen; parallele
Erstellungen übernehmen die Ablaufzeit des authentifizierten Gewinners.
GateDefault verlangt weiterhin TurnGateResume; explizite native und Browser-
Setup-Eingaben bleiben exakt. V196 ergänzt nur fehlende Settings, erhält größere
oder kleinere gültige Betreiberwerte und alle vorhandenen Auth-Originale.
WebUI-Steuerung und EN/DE-Hilfe gehören dazu. Diese Quelländerung benötigt noch
die tatsächliche native/Browser/Worker-Abnahme im bestehenden K3-Schritt6;
eigenständige OAuth-Defaults/-Maxima und Browser-/Ingress-Grenzen bleiben offen.

### K4 — Bereits qualifizierte Loader-, Child- und MCP-Grenzen abgleichen

**Nachweisabgleich; kein pauschal festgestellter Neubau. Owner:** Phase 0a/3a,
Composition liefert die tatsächlichen Verbraucher nach seinem Ownershipvertrag.
[RetainedPreload](crates/brassclaw_engine/src/executor/retained_preload.rs),
[RetainedRecipeExecution](crates/brassclaw_engine/src/executor/retained_recipe.rs),
[native Child-Provenienz](crates/brassclaw_reborn_composition/src/pg_monty_child.rs),
[Child-Reconciliation](crates/brassclaw_reborn_composition/src/monty_child_admission.rs)
und [MCP-Chatbridge](crates/brassclaw_reborn_composition/src/mcp_chat_bridge.rs)
enthalten bereits die tragenden Mechanismen. Bestehende einschlägige Belege zuerst
auf exakte Source-/Graph-/Artefaktidentität und folgende neue Präzisierungen prüfen:

1. Gleichnamige alte/neue Exporte behalten nach Wait/Child ihre Bindings; mutable
   Inputs/Defaults/Ergebnisse sind isoliert. Unzulässige rohe Monty-Werte werden
   vor verlustbehafteter Konversion abgewiesen; echter Markertext bleibt Daten.
2. Parentclaim und ursprüngliche Invocation/Childauswahl bleiben erhalten;
   Aufnahmebeleg ist kein terminales Ergebnis. Bereits unterstützte readable
   Ackverluste lösen keinen Ersatzchild aus. Nicht unterstützte/unlesbare Outcomes
   bleiben sichtbar ungeklärt; Catch/Timeout erzeugt keinen leeren Erfolg.
3. Ein wartender MCP-Parent blockiert seinen gewöhnlichen Chat nicht. Ungewisser
   Send, Result-Delivery und nicht destruktiver Close werden am Originalchat
   abgeglichen; Kohai-Disconnect betrifft nur den eigenen Request. Kein zweiter
   Reply/History-Effekt und kein direkter MCP-Executor.

**Arbeit/Abnahme:** Nur fehlende Fälle durch die realen bestehenden Caller
qualifizieren und konkret gefundene Fehler in diesen Ownern korrigieren.
Vorhandene passende Receipts wiederverwenden. Unsupported Retry-/Ketten-/Occurrence-
profile nicht neu freischalten: aktuell `stop`/`max_attempts: 1` und ein gebundener
Toolcall pro gewöhnlichem Ausführungsschritt. R09-Public-Parentbootstrap und fünf
Subagentrecipes, volle Crash-Recovery und V35-13/14 bleiben reguläre offene Arbeit.

#### Offene Definitions- und Consumerpräzisierungen nach K3

Der [Exact-Abgleich](docs/plans/Exact_V3_definition_update_at_K3.md) wird mit dem
jetzigen Auftrag als begrenzte Dokument-/Contractlieferung ab K4 umgesetzt. K0–K3
und bereits abgeschlossene Teilnachweise bleiben unverändert; keine zusätzliche
Voraussetzung für deren Abschluss. Die frühere Phase-3a-Pin-/Singleton-/Tracker-
Beschreibung ist historischer Ausgangspunkt, keine Anweisung zum erneuten Upgrade.
Aktuelle Baseline sind der tatsächlich vendorte Monty-1.0-Controlstand, vorhandener
retained Loader und ein globaler Dienst; verbleibende Lifecycle-/Recoverygates
bleiben beim bisherigen Owner. Später abgeschlossene K4-Teile ebenfalls nicht öffnen.

Für neue Consumer gelten die vier Komponentenguides und folgende genaue Zuordnung:

- K4 gleicht vorhandene Caller für Definition versus Invocation, echte typed
  Resultrefs, Source-Daten und MCP-Advertisement versus tatsächliches Taskcapture
  ab. Die [Composition-Nachfolger](docs/plans/monty.composition.md#open-contract-and-reference-successors-from-k4)
  und [vollständige Referenz](docs/plans/monty.composition.implementation.md#open-definition-and-example-successors-from-k4)
  ersetzen für neue Arbeit die dort benannten alten pauschalen Beispielregeln,
  ohne deren Originaltext/Receipts zu ändern. Bereits qualifizierte Mechanik nutzen.
- K5 prüft D01–D12 nur soweit tatsächlich betroffene Dokument-/Teaching-/Paketinputs
  vorliegen. Quellen-/Beispielprüfung ist keine Runtimeaktivierung. Neue eingebettete
  Bytes erhalten ihre eigene Identität; unabhängige alte Belege behalten ihren Scope.
- K6 übernimmt geänderte Paketbytes über bestehende immutable Revisionen/Bootstrap/
  Upgradeowner, unter Erhalt alter Tasks und Overrides. `ON CONFLICT DO NOTHING`
  oder ein neuer Rust-Text allein beweist keine installierte Übernahme.
- K7 dokumentiert Textlieferung, tatsächlich betroffene aktuelle Ausführung und
  zukünftige nicht aktivierte Entwürfe getrennt. Aktive Fehler können nicht als
  bloß zukünftige Arbeit vertagt werden; unbetroffene aktive Pfade nicht abschalten.

Keine neue VM, Approvalroute, MCP-Registry oder K8. Neue allgemeine Pure-Approval,
acht Validatorrouten, Bibliotheks-/Subagentmigration und dynamische Occurrences
bleiben bei ihren bestehenden offenen Gates. Eine Dokumentkorrektur zieht diese
nicht vor. Bei einer konkreten neu belegten Callerabweichung folgt nur das echte
Delta beim zuständigen Owner mit dessen Nachweis; kein pauschaler neuer Testauftrag.

### K5 — Geänderte eingebettete Dokumente und Validatorgrenzen nachziehen

**Paket-/Nachweisabgleich; Owner:** Installation/Phase 3a/6, mit Prefix- und
Validator-Owner. [Release-Dokumente](crates/brassclaw_reborn_composition/src/prefix_release_documents.rs),
[Release-Inventar](scripts/prefix/release-reference-inventory.json) und
[Source-Capture](crates/brassclaw_reborn_composition/src/prefix_source_capture.rs)
verwenden vollständige Paketquellen und unveränderliche gespeicherte Revisionen.
Die aktuellen Guide-/Planänderungen sind keine nachträgliche Änderung alter
Snapshots; der [jüngste Validatorbeleg](docs/validation/2026-10-24-python-graph-review.md)
schließt parallele Dokumentänderungen und die reguläre App ausdrücklich aus.

1. Tatsächlich paketierte geänderte Texte/Includelisten ermitteln, neue exakte
   Dokument-/Paketrevisionen erfassen und über vorhandenes Seeding/Repair anbieten.
   Keine alten Quellbytes, Hashes, Approvals oder Overrides überschreiben und keine
   alte Source-only-Generation durch bloßes Umbenennen als aktuell deklarieren.
2. Nur dadurch betroffene Paket-/Bootstrap-/Source-Capture-Consumer erneut
   qualifizieren. Nicht betroffene Interpreter-/Loaderbelege behalten ihren Scope.
   Qualifizierte shipped Nachfolger nutzen system_seed ohne zusätzliche Instanz-Q2;
   Operatoränderungen behalten authored Q1/Behavior/human Q2.
3. Aktuelle private Definition-/Contract-/Graphregeln und sealed Inputs weiterhin
   als begrenzte Vorprüfungen behandeln. Bei geänderten Paket-/Regelinputs die
   ursprüngliche Subject-/Rule-/Validatorauswahl und negative Substitutionsfälle
   nachweisen. Kein Runabschluss/Parsererfolg/stdout erzeugt Q1 oder Pure-Approval.
   Die bestehende menschliche Usage-Q2-Route bleibt exakt und idempotent; keine
   Rollen-/Human-Flag-Abkürzung und keine pauschale neue Q2-Runde für alte Tasks.

**Abnahme:** neue Dokumentrevision bei erhaltenem alten Snapshot/Override,
wiederholter Boot ohne künstliche Passbelege und korrekte neue Source-Capture-
Identität. Fehlender neuer Klassengraph ist weiterhin fehlender Support, keine
Regression der neun bereits geprüften privaten Schritte. Allgemeine Q1-/Pure-
Writer und acht produktive Routen nicht in diese Paketkorrektur hineinziehen.

### K6 — Alte Artefakte und Daten vor dem gemeinsamen Upgrade absichern

**Nachweis-/Retentionslücke; Owner:** Phase 6/8, fachliche Inventare aus allen vier
anderen Plänen. Der aktuelle [Supportindex](docs/implementation-status.md) belegt
retained Workerhosting, aber keinen allgemeinen ABI-Loader, vollständigen
Review-/Prefix-Restore oder beliebige Fortsetzung nach Prozesscrash.

1. Für den bereits unterstützten Umfang laufende/suspendierte Originalruns,
   Katalog-/Export-/native Implementierungsreferenzen, ursprüngliche Counts,
   confirmed/unresolved Effekte, Overrides und ggf. advertisierten Command-/Chat-
   Zustand im vorhandenen Upgrade-Inventar festhalten. Dazu bestehende Prefix-
   Originale/Generationen/Jobdefinitionen sowie vorhandene sealed Review-/Q2-Daten
   und Source-/Derivedreferenzen nehmen; keine künftigen Datenfelder erfinden.
2. Beim einen Backup-/Restoreowner tatsächliche benötigte Bytes/Checksums und
   unterstützte Reader/Adapter sichern/prüfen. Eine eingefrorene Workerdatei
   beweist keine Wiederladbarkeit jeder alten nativen Toolimplementierung.
   Wenn alte Tasks nicht sicher weiterlaufen können: Upgrade vor Cutover blockieren
   oder vorher kontrolliert drainen/reconciliieren; niemals latest substituieren,
   ungewisse Effekte wiederholen oder eine alte RAM-Fortsetzung neu erfinden.
3. Fehlende Retentions-/Fencingprüfungen für diese vorhandenen Artefakte ergänzen.
   Neue fachliche Restore-/Migrationsteilnehmer später im selben Owner aufnehmen;
   keine separaten Prefix-/Validator-Backupbefehle und keine angewandten Migrationen
   umschreiben. Die allgemeine Recoveryimplementierung bleibt offen, bis ihr
   vollständiger eigener Vertrag abgenommen ist.

**Abnahme:** bestehender Task/Override über kompatibles Paketupgrade bzw. sichere
Blockade bei fehlendem alten Artefakt; Restore der bereits unterstützten Records
mit bestätigten und ungeklärten Effekten. Fehlende Wiederaufnahme bleibt explizit
blockiert und ist kein „bestandener Restore“. Weiterentwicklung ohne unsicheren
Upgrade benötigt keinen vorgezogenen universellen Cross-Version-Loader.

### K7 — Konformitäts-Gate schließen und am bisherigen Fortschritt fortsetzen

Im bestehenden Validierungsrecord pro K0-Zeile und K1–K6 festhalten: konkrete
Änderung oder begründetes „keine Codeänderung“, exakte betroffene Identitäten,
wiederverwendete/neu beobachtete Callerfälle und verbleibende Supportgrenzen.
Ein Quellbefund allein ist kein bestandener Verhaltenstest. Gemeinsame Fälle
nur einmal am Owner ausführen; passende Belege in den anderen Plänen referenzieren.
Kein pauschaler Full-Build/Full-Suite-Lauf allein wegen dieser Markdownergänzung.
Bei späteren Ruständerungen die notwendigen echten gezielten Builds/Tests/Lints
nach der Entwicklungspolicy durchführen.

**Voraussetzung der regulären Fortsetzung:** K1–K3 sind für vorhandene Consumer
repariert und abgenommen; K4–K6 sind für den bereits unterstützten Umfang durch
passende Belege oder sichere ausdrückliche Blockaden geschlossen. Eine Blockade
gilt nur, wenn der fehlende Pfad im Produkt wirklich nicht als unterstützt
freigegeben ist und seine Records/Effects erhalten bleiben; kein Markdown-Label
und kein stilles Entfernen einer aktiven Recipe. Unveränderte funktionierende
Pfade nicht abschalten, um das Gate rechnerisch zu erfüllen. Offene Befunde in
bereits aktiven Pfaden verhindern den Abschluss dieses Vorrangblocks.

Danach an den bereits erreichten offenen Schritten weiterarbeiten. Allgemeine
Pure-Approval/acht Validatorrouten, fünf Subagentmigrationen, vollständige
Bibliothekskonvertierung, akzeptierte Prefixdesigns/Generate/Serving/finaler Guard,
Scope-/Produktcutover, dynamische Occurrences und allgemeine Crash-Recovery bleiben
bei ihren bisherigen Ownern und offenen Gates. Sie werden weder durch dieses
Gate als erledigt markiert noch als angebliche Wiederholung abgeschlossener
Schritte vorgezogen. Historische akzeptierte Belege bleiben unverändert erhalten.

## Remaining implementation — consolidated 2026-10-22

This is the current remaining-work disposition for the existing phases, not a
restart of completed implementation. Use the [shared status/owner index](docs/implementation-status.md)
and [dated evidence journal](docs/plans/simplified-v3-implementation.md). The
contracts and acceptance matrices below remain binding; old descriptions of a
per-chat VM, absent immutable storage or absent typed bindings are historical.

The 2026-10-23 successors additionally provide complete native raw-file capture,
sealed pure-review source/subject prerequisites, independent pure-execution
coverage rejection and per-step observation limits. Their scoped receipts are
linked from the status index. None completes general pure approval or class Q1.
Consume [validator's exact remaining contract](docs/reborn/pure-component-review.md#exact-remaining-cutover-contract)
for those gaps; do not add a runtime approval exception or duplicate writer here.

1. **Finish authority and product-surface cutover (Phases 0–3/4–5, §§8–10).**
   Complete the consumer-by-consumer scope/role/profile/grant inventory and the
   authenticated route/function matrix. Replace remaining per-operation approval
   and role/tenant permission decisions with current instance-wide Tool rules;
   preserve token/origin/CSRF, sandbox/network/secret checks and data identity/AAD.
   Remove a legacy check only with its replacement wired through the real caller.
   Finish Intents CRUD/matching explanation, operator configuration and unsupported
   product surfaces. Acceptance: real authenticated read/write/dispatch cases,
   invalid token/external-sender rejection, live policy changes during retained
   tasks, no hidden role/edition path and no lost function disguised as cleanup.
2. **Finish live resource/cap coverage (Phase 3a, §§5/9).** Start from the existing
   acknowledged Rust/Monty settings publication and UI explanations. Inventory
   remaining actual consumers/defaults/ranges, including transport depth 64,
   PG snapshot conflict attempts 12 and filesystem attempts 8. Reuse the existing
   settings owner for adjustable policies and show desired/effective revision,
   pending/failure state and short help. Before lifting depth/parser limits,
   complete safe owned decoding and late-error cleanup. Conflict retry only pure
   snapshot transitions, never external effects. Keep startup sizing default,
   512 MiB startup/manual floor, optional slow automatic mode and 600 executing
   task seconds. Disabled discretionary token budgets must reach all remaining
   consumers; provider context/output and per-model hard input caps remain active.
   Acceptance: real WebUI edits, failed/slow DB writes, concurrent edits, active
   tasks/waits, unsafe reductions, deep/large hostile values and platform checks.
3. **Finish durable lifecycle/child recovery (Phase 3a).** Reuse the single global
   service, actual parent claims, captured catalogue, native invocation custody,
   dependent admission and the atomic transcript/turn participant. Complete
   unreadable/delayed ensure/input/run outcomes, filesystem pending receipts,
   legacy unkeyed reservation retirement, bounded ancestor cancellation and
   saturated/multilevel child waits. Persist/reconcile original identities,
   dispatch counts and known/unknown effects across shutdown/restart; expose
   operator diagnosis/settlement without resubmission or synthetic completion.
   Fence the old generation before supervised replacement; no parallel VM,
   per-chat VM, Rust workflow fallback or whole-Recipe replay. Acceptance includes
   real lost/rejected COMMITs, stale attempts, policy edits, no duplicate effects,
   singleton ownership, service readiness and unrelated-work progress.
4. **Consume shared component/review work once (Phase 0a).** Exact review subjects,
   eight validator workflows, trusted evidence and general pure approval are
   implemented by [validator Steps 1–12](validator_v3.md#remaining-implementation--consolidated-2026-10-22).
   Canonical library migration, usage exports/layouts and MCP population are
   implemented by [composition](docs/plans/monty.composition.md#remaining-implementation--consolidated-2026-10-22)
   and its component/Recipe appendices. Runtime consumes their qualified selections
   and approval/artifact references; it does not build a second loader, validator,
   manifest store or component catalogue. Full native artifact retention/loading
   across incompatible upgrades remains a runtime gate, not metadata-only proof.
5. **Consume prefix/provider and future v3.5 work (Phase 0a/3a, §10).** PostgreSQL
   provider management is the canonical mutable owner. Prefix/profile/model
   budgets, Generate/design/Sources/serving and unconditional final request sizing
   are implemented by [the prefix remaining sequence](docs/plans/prefix_v3_upgrade.md#remaining-implementation-sequence--full-completion).
   This plan wires/accepts those results in ordinary dispatch; it does not add a
   second budget store or compiler. Recipe-owned Sempai semantics, dynamic
   occurrences and connection/facility consolidation use the existing
   [v3.5 work IDs](path_to_v3.5.md#ongoing-plan-handoffs-not-parallel-implementations).
   Preserve native accounting/custody when moving business sequencing to Python.
   The concrete shared handoffs are specified below in Phase 3a's
   [admission/recovery contract](#gemeinsamer-admission--und-recoveryvertrag-für-produktaufträge),
   Phase 6 item 10, Phase 8 item 7, §8.2's activation ordering and §10.1 items 7–8.
   Prefix consumes these mechanisms and supplies its own payloads/adapters and
   integration cases; it does not implement another scheduler or upgrade owner.
   The [shared review-runtime contract](#gemeinsame-runtime-anbindung-der-komponentenprüfung)
   additionally covers validator-specific custody/admission/boot/upgrade needs.
   [validator_v3.md](validator_v3.md#concurrent-implementation-with-simplified-v3)
   implements review subjects, all class checks and trusted approval semantics;
   this plan integrates them without creating another review/evidence writer.
   The [library integration contract](#gemeinsame-runtime-verträge-für-die-komponentenmigration)
   covers composition export/child/MCP/deployment handoffs. Composition owns the
   component families and protocol/command wiring; this plan implements only the
   missing shared mechanics in their existing owners.
6. **Complete data and compatibility migration (Phases 6/8).** Finish dry-run
   inventory, override-aware immutable seed upgrades and retained old artifact/
   approval references. Audit remaining config.toml/provider-file and scoped
   store fallbacks; retire them only after their current PostgreSQL consumers
   and migration/repair paths pass. Reuse the existing PG connection/TLS builder;
   remaining connection lifetime/TLS acceptance belongs to V35-21. Preserve secret
   AAD, keys, aliases and transcripts. Test fresh/existing/repeated boot, V176
   healthy/corrupt/ambiguous legacy receipt upgrade, interrupted writes, backup
   and actual restore. Never edit applied migrations or choose ambiguous owners.
7. **Run combined production/cutover acceptance (Phases 7–8, §5).** Exercise the
   actual operator UI/API, all retained Recipe variants/classes and external
   adapters on the combined source, with original IDs, no-replay effects, old
   task selections, live policy/budgets and complete eligible history. Include
   Linux/macOS/Windows build/behavior boundaries and measured startup/latency/
   idle/memory evidence before performance claims. Release only after those
   gates and a fresh supported installation/provider/prefix/function audit.
   A normal commit/push can record this incomplete checkpoint; it does not close
   the implementation or authorize claiming full release acceptance.

## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
A Skill needs no dedicated Recipe or matching command to exist or be complete.
Many Recipes may reuse the same approved usage. MCP exposure requires a separately
qualified MCP-call Recipe/variant; reference/test workflows do not imply ownership.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](skills.md#preloadable-function-interface-binding-v3-target).
Selected retained loader/store/runner support is implemented; this contract
does not certify every workflow or the complete migration. See
[the current implementation status](docs/implementation-status.md).
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


## Binding Recipe architecture (v3)

Read all four ground-truth guides before authoring or changing components:
[recipe.md](recipe.md) for workflows and persisted IBS syntax,
[skills.md](skills.md) for complete usages, recursive schemas and association
approval, [tools.md](tools.md) for primitives, retained implementations, live
policy and recovery, and [toolskills.md](toolskills.md) for IBS binding descriptors.
Their component contracts govern the implementation steps below; historical
examples do not override them. This plan specifies targets, not completed
runtime or database functionality.

- Rust Tools supply primitives; many ToolSkills describe their IBS bindings;
  many Skills expose one Tool usage through declared preloaded Python functions;
  many small PythonCode components provide reusable executable building blocks.
  Recipes tell the orchestrator how to use them to fulfill task goals. Prefer
  explicit reusable steps, not fewer steps or specialized Rust workflow Tools.
- Each Recipe component step references exactly one stable component UUID.
  PythonCode may internally compose smaller PythonCode components; this is not
  a multi-component Recipe step. Keep all independent Tool calls in separate
  execution steps; the existing direct dependent-chain exception still applies.
- IBS/composition reads the newest activated, approved versions at task start
  from one consistent catalogue snapshot and pins exact UUID/version/checksum
  references in BuildInstruction, including Recipe/variant/step_link/input layout,
  nested dependencies, actual Tool implementations and exact association approval.
  Incompatible or unapproved newest active combinations fail before effects;
  never silently downgrade or enter Tier 2. Recipes carry
  no version numbers. Execution, child steps, waits and resumption retain that
  selection; do not look up latest again during the task.
- Approved versions are immutable. Changes create new versions; authored
  versions pass Q1 and human Q2 before activation. Replacement neither deletes
  nor invalidates originals used by running/suspended tasks. Current global
  Tool policy is checked independently before every dispatch.
- Inputs and results are typed data. Use the exact input-reference grammar and
  step-local binding convention in recipe.md. Runtime values never become
  Python source. Monty owns each task's intermediate results; unrelated tasks
  and attempts stay isolated, including across child execution and waits.
- Rust-channel ToolSkill binding executes nothing and grants no permission.
  Orchestrator-channel PythonCode calls host.<tool>(...). Only an actual
  No-Match enters Tier 2; errors or begun Recipe failures never replay there.
- The Skill tells the Orchestrator how to use the Tool; its associated PythonCode
  implements that usage. The ToolSkill tells IBS how to prepare the binding.
  Neither descriptive metadata nor a code example is an executable association.
- Exact-combination approval is separate from task selection and Tool permission.
  Authored combinations require trusted Q1, human Q2 and behavioral evidence;
  only verified system_seed bootstrap provenance permits null q2_ref with the
  required integrity/Q1 and behavioral evidence. Source/status labels do not qualify.
- Every retained Tool version/dispatch alias receives the same Tool's current
  global policy. Persist invocation counts and effect status before dispatch
  through the supported recovery contract; waits/reclaims/crashes do not reset
  counts or permit replay of completed effects.

The legacy composer still contains source-substitution and historical step-body
paths. Ordinary PostgreSQL/skills-db execution uses immutable retained selections,
typed inputs, checked single-component steps and preloaded exports on the global
Monty service. Full-library migration, general pure approval, dynamic occurrences
and durable recovery remain open; see the current implementation status index.

## Verbindliche Skill-Architektur

Ein **Skill** ist genau ein wiederverwendbares Nutzungsmuster eines Tools für
den Orchestrator: **Prosa-Anleitung plus ausdrücklich zugeordnete vorladbare
Python-Funktionsschnittstelle**. Sie deklariert Exportnamen/Signaturen, private
Hilfsfunktionen/Konstanten, Abhängigkeiten und typisierte Verträge. Abhängigkeiten
werden deterministisch zuerst geladen; Laden führt keinen Toolaufruf aus.
Aufrufe verwenden die am Vorgangsstart fixierten Revisionen und Exportbindungen.
Gemeinsamer Code/unveränderliche Konstanten sind zulässig; veränderliche Arrays,
Defaults und Ergebnisse bleiben pro Vorgang/Versuch/Aufruf isoliert. Ein
passender ToolSkill und aktuelle Kernelprüfungen bleiben vor jedem Dispatch
erforderlich. Ein vollständiger Skill benötigt kein eigenes Recipe und keine
Befehlsschablone; mehrere Recipes können dieselbe freigegebene Nutzung verwenden.
Nur eine beabsichtigte MCP-Veröffentlichung benötigt ein gesondert qualifiziertes
MCP-Aufruf-Recipe oder eine geeignete Variante. MCP übermittelt dessen Befehl,
keinen Pythoncode. „Leaf Skill“ meint
dieselbe Einheit. Größere fachliche Zusammenhänge gehören zur **Extension** und
ihrem ExtensionCatalogue; das **Recipe** bestimmt Reihenfolge und Datenübergabe.
Tool und ToolSkill gehören zur Rust-Seite. Binden führt nichts aus; PythonCode
ruft `host.<tool>(...)` auf, der Kernel prüft die aktuelle globale Toolregel.

Heute sind Prosa (`reborn_skills`, Klassen 1–3) und Code (`reborn_python_code`,
Klasse 22) getrennt gespeichert. Diese Trennung ist zulässig; sie macht einen
Skill fachlich nicht zu reiner Prosa. Die vorhandenen Klassenbezeichnungen sind
Consumer-Klassifikationen, keine Hierarchie von kleinen und großen Skills.

**Umsetzung und Abnahme:** die vollständigen Zielverträge
`skill-association/1` und `skill-association-approval/1` aus skills.md über Store,
Validatoren, IBS/Composer und WebUI umsetzen. Die Zuordnung verbindet stabile
Skill-/PythonCode-/ToolSkill-/Tool-UUIDs und Nutzungsverträge; der getrennte
vertrauenswürdige Approval-Datensatz belegt die exakt geprüften Revisionen,
Checksums und transitiven Abhängigkeiten. Beide Skill-Teile gemeinsam sichtbar
und durch neue unveränderliche Versionen einzeln bearbeitbar machen; betroffene
Kombinationen erst mit passender neuer Approval-Evidenz gemeinsam aktivieren.
Unveränderte Skill-Prosa darf ihre Revision behalten. Einzelne validierte Zeilen
oder ein Taskmanifest sind kein Kombinationsnachweis und keine Toolfreigabe.
Autorreview, unterstützte Q1-Audits, Verhaltensnachweise und menschliches Q2
stellen bei authored-Versionen die fachliche Übereinstimmung vor Aktivierung
her; trusted system_seed folgt seinem gesonderten Evidenzvertrag. IBS prüft
beim Taskstart strukturierte Verweise/Verträge und bestätigte Evidenz, nicht
Prosa-Bedeutung oder Python-Geschäftslogik mittels LLM. Laufende ausgewählte
Revisionen und ihre ursprünglichen Approval-Datensätze bleiben erhalten.
Vorhandene Prosa-Zeilen nicht blind als vollständig migrierte Skills markieren;
Zuordnungen prüfen und fehlende als Migrationsbedarf anzeigen. Fachliche
Mehrtool-Übersichten in Extension-Kontext überführen und bestehende UUID-Verweise
kontrolliert migrieren. Tier 0 führt zugeordneten PythonCode ohne LLM aus;
Tier 1 darf die Prosa in expliziten LLM-Schritten nutzen. Beispiele in Prosa
werden niemals automatisch ausgeführt. Abnahme prüft Wiederverwendung, gültige
Referenzen, Fortsetzung mit fixierten Revisionen und die getrennten Kanäle.
Diese Anforderungen sind Planinhalt, keine Behauptung fertiger Implementierung.


**Status:** Implementierung begonnen; Voraussetzungen teilweise umgesetzt, kein Produkt-/Daten-Cutover. Fortschritt und verbleibende Nachweise: `docs/plans/simplified-v3-implementation.md`.
**Stand:** 2026-10-06 (Plan nach Einzelprüfung überarbeitet: Recipe-Vertrag, globale Toolregeln, Live-Limits, Intents, Migration und Leistungsabnahme)
**Geltungsbereich:** gesamte BrassClaw-Codebase; Schwerpunkt Reborn, globaler Orchestrator, Authentifizierung, Autorisierung, Runtime-Auswahl, WebUI und Persistenz.

**Ground-truth-Abgleich 2026-10-06:** Phase 0a ergänzt verbindliche Arbeitspakete
für Assoziation/Approval, vollständige Versionierung, typisierte Bindings,
ToolSkill-Vorbereitung und den exakten Retryvertrag. Phasen 3/3a/5/6/7/8 und die
globale Abnahme müssen diese Verträge nachweisen. Die Ergänzung ist Planarbeit;
sie meldet keinen neuen Implementierungs- oder Testfortschritt.

## 1. Zielbild

BrassClaw wird als **ein einziges vollständiges Produkt** gebaut und ausgeliefert. Es gibt keine Editionen, deploymentabhängigen Produktvarianten oder vom Nutzer auswählbaren Runtime-Sicherheitsprofile. Eine zentrale, instanzweite Sicherheitsrichtlinie gilt für alle Interaktionen und wird mit sicheren Standardwerten ausgeliefert.

Die WebUI hat einen einzigen Betreiberzugang: Wer sich mit dem gültigen Instanz-Token anmeldet, ist Betreiber und kann sämtliche Instanzdaten einsehen und alle angebotenen Einstellungen verwalten. Es gibt keine WebUI-Rollen, Benutzerkonten, Projektberechtigungen oder per Benutzer abweichenden Profile. Projekt-, Thread- und ähnliche Kennungen dürfen als Datenbeziehungen bestehen bleiben; sie sind keine Berechtigungsgrenzen.

Das ist ein **Single-Operator-/Single-Instance-Modell**, kein Multi-Tenant-SaaS-Modell. Netzwerkanbindung, Sandboxen, Secret-Schutz, Eingabevalidierung, Audit und Schutz vor CSRF/Token-Diebstahl bleiben eigenständige technische Sicherheitsmaßnahmen. Ein erfolgreicher WebUI-Login darf nicht automatisch externe Kanal-Absender oder agentengenerierte Capability-Aufrufe zu Betreibern machen.

**Monty startet beim Systemstart als genau ein globaler Orchestrator und bleibt für die gesamte Instanzlaufzeit im Hintergrund aktiv.** Nach Datenbankmigration, Komponenten-Seeding und Integritätsprüfung wird die globale Monty-VM mit dem verifizierten Python-Orchestrator gestartet, bevor Turn-Worker, Trigger-/Kanal-Produzenten und Ingress Arbeit annehmen. Idle bedeutet blockierendes Warten auf Arbeit, keine CPU-Polling-Schleife und keine LLM-Aufrufe. Eine Chatnachricht ist ein Vorgang des bereits laufenden Orchestrators; es entsteht keine neue globale VM pro Chat oder Eingabe. Task-Ende, Chat-Löschen, Browser-Trennung und Run-Abbruch beenden nicht den Orchestrator. Nur Instanz-Shutdown oder beaufsichtigte Wiederherstellung nach einem fatalen VM-Fehler ersetzen ihn.

Der globale Orchestrator verarbeitet explizite, getrennte Vorgangskontexte: Conversation-/Thread-/Run-/Message-IDs bleiben Datenbeziehungen für History, Fortsetzungen, Antworten, Audit und Idempotenz. Sie bilden keinen gemeinsamen Chatverlauf. Recipe-/PythonCode-Ausführung bleibt die Ablaufsteuerung; Rust stellt VM-Hosting, Eingabe-/Ereignistransport, persistente Turn-Koordination und Kernel-Gates bereit. Ein globaler Rust-Dispatcher darf keinen zweiten Agenten-/Recipe-Loop implementieren. Kurzlebige Ausführungen von `host.run_program` sind keine weiteren globalen Orchestratoren.

### 1.1 Verbindlicher Recipe-Vertrag

```text
Eingabe
  → bereits laufender globaler Monty-Orchestrator
  → bestehendes Intent-Matching
      │
      ├─ Match
      │   → ausgewählte Recipe-Variante
      │   → IBS erstellt BuildInstruction
      │       mit fixierten Komponentenreferenzen und Eingabezuordnung
      │   → Composition löst die Referenzen auf:
      │       Skills → Prosa + zugeordneter ausführbarer PythonCode
      │       ToolSkills → Metadaten für die Rust-seitigen Tool-Bindings
      │       weitere PythonCode-Komponenten → ausführbare Logik
      │   → Tool-Bindings werden anhand der ToolSkills vorbereitet
      │       und über die Rust-seitige Host-Integration bereitgestellt
      │   → Monty führt die komponierten Schritte in Recipe-Reihenfolge aus:
      │       • PythonCode verarbeitet separat übergebene typisierte Eingaben
      │       • host.<tool>(...) ruft das jeweilige Rust-Tool auf
      │         → Kernel prüft aktuelle globale Toolregel und technische Grenzen
      │         → Ergebnis zurück an Monty
      │       • vorgesehene LLM-Schritte bei Tier 1
      │       • Ergebnisse werden innerhalb des Vorgangs weitergegeben
      │   → Antwort → History → Vorgang abgeschlossen
      │
      └─ No Match
          → Monty stellt den Tier-2-Prompt zusammen:
              Nutzereingabe + Vorgangs-History
              (aus der Komponentenbibliothek vorkompiliert)
          → Prompt wird an Kohai geschickt
          → Kohai formatiert nur den dynamischen Prompt anhand des
            gespeicherten akzeptierten Designs des Provider-/Modell-Zielpaars
          → Falls Sempai verbunden/aktiviert ist: diesen formatierten Prompt
            optimieren/korrigieren und als finalen dynamischen Prompt zurückgeben;
            Sempais eigener Aufruf nutzt sein eigenes für sein Zielpaar generiertes Prefix
          → Kohai setzt das vorgesehene fertige, unveränderlich gespeicherte Prefix
            zeichengetreu genau einmal davor; keine erneute Formatierung
          → Vollständigen Request prüfen und zum LLM schicken
          → LLM bearbeitet die Aufgabe
          → LLM sendet die Antwort zurück an Kohai
          → Wenn ein Sempai an Kohai angeschlossen ist sendet Kohai die Antwort an Monty und an Sempai voneinander unabhängig weiter, sonst nur an Monty.
          → Wenn ein Sempai verbunden ist dann wertet Sempai den Vorgang aus und sendet ggf. einen erstellten Component Kandidaten oder einen neuen Intent zur Q1 validation Queue.
          → Antwort → History → Vorgang abgeschlossen
          → Q1 + Verhaltensvalidierung → menschliches Q2
          → kohärente Aktivierung
          → für künftige Vorgänge nutzbar
Der globale Monty-Orchestrator bleibt aktiv und wartet auf weitere Arbeit.
```

Die tatsächliche Reihenfolge folgt den `step_descriptions` der ausgewählten Variante. Ein `channel:"rust"`-Schritt bindet einen ToolSkill und führt nichts aus. Der folgende passende `channel:"orchestrator"`-Schritt führt PythonCode mit `host.<tool>(...)` aus. Tier 0 enthält keine LLM-Schritte; Tier 1 enthält die im Recipe vorgesehenen LLM-Schritte. Das bereits vorhandene Rust-Intent-Matching und IBS werden weiterverwendet. Eine zusätzliche Matching-VM ist keine Voraussetzung dieses Plans.

| Zuständigkeit | Verantwortung |
|---|---|
| Recipes / Python / Komponenten | Fachliche Schrittfolge, Toolaufrufe, LLM-Schritte, Antworten und Fortsetzung des fachlichen Ablaufs |
| Rust / Infrastruktur | VM-Hosting, Ereignistransport, persistente Aufnahme, Worker-Claims, Speicher-/Zeitmessung und Dienstlebenszyklus |
| Kernel | Aktuelle globale Toolregeln und technische Sandbox-, Netzwerk-, Secret- und Ressourcenregeln durchsetzen |
| WebUI / Produkte | Vollständige Betreiberverwaltung und Eingabe-/Control-Übergabe über dieselben Verträge |

**Globale Toolregeln:** Der Betreiber stellt Zulassung, Sperrung und unterstützte Toolparameter im WebUI instanzweit ein. Vor jedem tatsächlichen Dispatch gilt die aktuelle Einstellung. Es gibt keine zusätzlichen Toolfreigaben pro Operation, Run oder Versuch. Worker-Claims/Attempts bleiben Ausführungskennungen; Q1/Q2 bleiben Komponentenvalidierung; externe OAuth-/Auth-Waits bleiben Dienstauthentifizierung.

**Fehlervertrag:** No-Match, Mehrdeutigkeit, Matching-/DB-Fehler und Recipe-Ausführungsfehler sind verschiedene Ergebnisse. Nur echtes No-Match beginnt den normalen Tier-2-Pfad. Mehrdeutigkeit verlangt eine nachvollziehbare Auswahl; technische Fehler werden als Fehler angezeigt. Ein begonnenes Recipe darf nach einem Schrittfehler nicht stillschweigend als Tier-2-Aufgabe neu ausgeführt werden. Bereits erfolgte Effekte und ungeklärte Ergebnisse bleiben festgehalten. Eine Fehlermeldung darf keine neue fachliche Toolausführung auslösen. Für den fehlgeschlagenen No-Match-Instruction-Pfad ebenfalls keine verdeckte direkte LLM-Ersatzschleife einführen.

**Komponentenänderungen:** Neue Fähigkeiten zuerst durch Wiederverwendung vorhandener Recipes, Varianten, ToolSkills und PythonCode umsetzen. Neue Rust-Tools sind nur für fehlende Systemprimitive zulässig und benötigen den vollständigen Komponentenstack. Runtime-Lebenszyklus und Kernel-Durchsetzung sind Infrastrukturaufgaben. Ein laufender Vorgang behält seine ausgewählten Komponentenrevisionen; neue freigegebene Revisionen gelten für folgende Vorgänge.

## 2. Auditumfang und aktuelle Architektur

Gesichtet wurden die Repository-Orientierung (`README.md`, `CONTRIBUTING.md`, `AGENTS.md`, `CLAUDE.md`), Workspace- und Crate-Struktur, Reborn-Architektur- und Vertragsdokumente, relevante Konfigurationen, Einstiegs- und Kompositionspunkte, Scope-/Profil-/Trust-/Ingress-Implementierungen, WebUI-Module sowie Test- und CI-Inventare. Die ursprüngliche Untersuchung war statisch; die inzwischen ausgeführten gezielten Implementierungsprüfungen stehen in `docs/plans/simplified-v3-implementation.md`. Das aktuelle Workspace-/Dateiinventar ist aus Cargo und dem gemeinsamen Git-Baum abzuleiten; die früheren Größenangaben waren historische Momentaufnahmen.

### 2.1 Aktuelle Pfade

- **Produktstart:** `src/main.rs` delegiert an `brassclaw_reborn_cli`; CLI-Befehle liegen in `crates/brassclaw_reborn_cli/src/commands/`.
- **Komposition:** `brassclaw_reborn_composition` baut Runtime, Substrate, Loop-Driver, Provider und WebUI zusammen.
- **Requestpfad:** WebUI-Ingress authentifiziert Bearer-Token; WebUI-Handler erhalten `WebUiAuthenticatedCaller`; Komposition reicht in Turn-/Host-APIs weiter.
- **Ausführung:** Turn-Koordination und Runner rufen einen Agent-Loop auf. Capability-Aufrufe laufen über `CapabilityHost`, Autorisierung/Obligations-/Approval-Pfade und Dispatcher/Runtime-Lanes.
- **Monty-Istzustand (2026-10-22):** Der gewöhnliche PostgreSQL/skills-db-Pfad startet `GlobalMontyOwner` vor Workern/Produzenten. `GlobalMontyDriver` übergibt exakt adressierte Aufgaben an diesen bereits laufenden Dienst. `PersistentMontyDriver` und die lazy Per-Chat-VM sind entfernt. Verbleibend sind die im aktuellen Restplan benannten Recovery-, Bibliotheks-, Policy- und Abnahmegates, nicht ein erneuter Singleton-Umbau.
- **Sicherheitsdienste:** getrennte Crates für Trust, Authorization, Approvals, Secrets, Network, Filesystem, Resources, Processes, Safety und Audit/Eventing.
- **Persistenz:** PostgreSQL (eingebettet oder extern), Migrations in Root- und Crate-Verzeichnissen; einzelne Legacy-/Upgrade-Pfade bestehen fort.

### 2.2 Erkannte Komplexitätsquellen

1. **Deployment- und Runtime-Profilmatrix:** `brassclaw_host_api::runtime_policy` definiert `DeploymentMode` und zahlreiche `RuntimeProfile`-Varianten (u. a. local/hosted/enterprise, safe/dev/yolo, `Full`, `Sandboxed`, `Experiment`). `brassclaw_runtime_policy` kombiniert diese mit Obergrenzen und Organisationsregeln zu mehreren Backend-Feldern. Profile tauchen zusätzlich in Konfiguration, CLI, Komposition, Run-Profilen, Trust-Entscheidungen, Tests und Dokumenten auf.
2. **Scope-Identität als Autorisierung und Storage-Konvention:** `brassclaw_host_api::scope`, `resource.rs`, Actor-/Principal-Verträge sowie Authorization-, Approval-, Turns-, Filesystem-, Secrets-, Memory-, Events-, Thread- und Product-Adapter-Pfade transportieren Tenant/User/Project/Agent/Thread-Scope. Werte dienen teils zugleich zur Objektadressierung, Mandantentrennung, Freigabeprüfung, Lease-Bindung, Verschlüsselungs-AAD und Datenbankfilterung.
3. **Mehrere Trust-/Grant-/Lease-Schichten:** TrustClass, Capability-Grants, Capability-Profile, capability allowlists, scoped/fingerprinted approval leases und Obligation-Handling bilden überlappende Entscheidungsachsen. Ihre Grenzen sind sicherheitskritisch und müssen vor dem Zusammenlegen anhand tatsächlicher Aufrufpfade inventarisiert werden.
4. **Mehrere interne Ausführungslanes:** First-party, MCP, System und Prozess-/Sandbox-Ausführung haben unterschiedliche Mechaniken. Das sind nicht automatisch Produkteditionen oder Benutzerprofile. Ihre Isolation und Side-Effect-Gates dürfen nicht allein deshalb entfernt werden, weil die Auswahloberfläche vereinfacht wird.
5. **WebUI-Sicherheitskonfiguration:** Es gibt bereits Settings-Bausteine für Safety/Security, Tokens, Provider und weitere Funktionen, aber sie sind nicht als ein vollständiges, systematisches Panel pro Schutzebene organisiert. Backend-Konfiguration und UI-Abdeckung sind nicht durch einen einzigen vollständigen Parameterkatalog belegt.
6. **WebUI-Fläche:** Routen und Einstellungen spiegeln noch Profile, Security, Users und Scopes wider. Zudem gibt es statische API-Module mit expliziten Platzhaltern für fehlende v2-Endpunkte: Admin, Jobs, Missions/Projects, Workspace, Routines, Settings/Users sowie Gateway-Restart.
7. **Migration und Dokumentationsdrift:** Verträge dokumentieren bewusst LocalSingleUser, HostedMultiTenant und EnterpriseDedicated. Einige Tests/Kommentare sind als Migrations- und Feature-Gates markiert. Die E2E-Dokumentation beschreibt teilweise ältere Binary-/libSQL-Annahmen. Ein Umbau braucht daher einen expliziten Contract-/Schema-/Fixture-Cutover statt bloßer Typumbenennung.
8. **Historische Chatbindung und verbleibende Fortsetzungen:** Der alte `basic_mode.py`-/`LimitedTracker`-Befund beschreibt den Ausgangszustand. Der gewöhnliche globale Monty-1.0-Pfad hat exakte Eingaben, getrennte Task-Accounts, typisierte Ergebnisse, gemeinsame Live-Revisionen und Reply/History-Abschluss. Ungeklärte Effekte, allgemeine Crash-Recovery, verbleibende Legacy-Aufrufer und volle Plattform-/Cutover-Abnahme bleiben offen.

**Nachweisstand:** Der vorhandene A→B→A-Test belegt die Fortsetzungs-API der gepinnten Monty-Version, nicht den vollständigen Produktionspfad. Native PostgreSQL-Testrigs liegen inzwischen in Composition vor; erfolgreiche Boot-/Restore-/Cutover-Abnahme ist damit noch nicht behauptet. Ältere Angaben im Implementierungsbericht zu ausschließlich Docker-basierten Tests und konkreten Invocation-Approvals sind historischer Stand und beim nächsten Implementierungsnachweis zu aktualisieren. Tatsächliche Testergebnisse bleiben unverändert dokumentiert.

### 2.3 Dateien und Crates mit hoher Änderungswahrscheinlichkeit

- Verträge/Identität: `crates/brassclaw_host_api/src/{scope,resource,runtime_policy,trust,ingress,capability_profile}.rs`, `crates/brassclaw_common/src/identity.rs`, `crates/brassclaw_reborn_identity/`.
- Runtime-Auflösung: `crates/brassclaw_runtime_policy/`, `crates/brassclaw_reborn_config/`, `crates/brassclaw_reborn_composition/src/{local_runtime_profile.rs,production_runtime_policy.rs,runtime/,factory.rs}`, `crates/brassclaw_turns/src/run_profile/`, CLI-Befehl `runtime-profile` und `BRASSCLAW_RUNTIME_PROFILE`-Environmentpfad.
- Autorisierung und Seiteneffekte: `brassclaw_authorization`, `brassclaw_approvals`, `brassclaw_capabilities`, `brassclaw_trust`, `brassclaw_secrets`, `brassclaw_filesystem`, `brassclaw_network`, `brassclaw_processes`, `brassclaw_process_sandbox`, `brassclaw_resources`, `brassclaw_host_runtime`.
- WebUI: `brassclaw_reborn_webui_ingress`, `brassclaw_webui_v2`, `brassclaw_reborn_composition` WebUI-Routen/Services und `brassclaw_webui_v2_static/static/js/pages/`.
- Persistenz und Migrationen: Root `migrations/`, `brassclaw_pg`, `brassclaw_reborn_identity`, `brassclaw_secrets`, `brassclaw_authorization`, Turns-/Thread-/Event-Stores.
- Grenztests: `brassclaw_architecture/tests/reborn_dependency_boundaries.rs`, crate-contract-Tests, Root `tests/reborn_*`, `tests/e2e/`, `tests/playwright*`.
- Globaler Orchestrator: `brassclaw_reborn_composition/src/{runtime.rs,webui.rs,booted_db.rs,persistent_monty_driver.rs,session_registry.rs,pg_orchestrator_code_port.rs}`, `brassclaw_engine/src/executor/{orchestrator.rs,orchestrator_code_port.rs}`, `brassclaw_engine/orchestrator/basic_mode.py`, `brassclaw_reborn/src/{runtime.rs,turn_runner.rs}`, `brassclaw_turns/src/run_profile/{driver.rs,host.rs}`, Eingangs-/Gate-/Reply-Pfade in `brassclaw_product_workflow` und CLI `serve`/`repl`. Dateinamen beim Implementierungsbeginn erneut prüfen; neue Transportverträge bleiben neutral und DB-frei in den vorhandenen Port-Grenzen.

## 3. Sicherheitsentscheidungen vor Implementierung

Diese Fragen werden in Phase 0 als kurze, verbindliche Architekturentscheidung festgehalten:

1. **Betriebsmodell:** v3 unterstützt genau eine lokale/selbst betriebene Instanz mit einem Betreiber. Hosted multi-tenant und Enterprise-dedicated sind keine unterstützten Modi/Build-Varianten mehr.
2. **Betreiber-Token:** ein Bootstrap-Token authentifiziert den Betreiber. Speicherung/Rotation, konstante Laufzeitvergleiche, CSRF-/Origin-Schutz, sichere Sessionbehandlung und Secret-Redaktion bleiben erhalten. Ein widerrufbarer Browser-Session-Token kann intern nötig sein, ist aber kein zusätzliches Konto/Rollenmodell.
3. **Default-Policy:** feste Code-Defaults und eine zentrale, vom Betreiber verwaltete Instanzkonfiguration. Globale Tool-Zulassung/Sperrung ersetzt operationsbezogene Freigaben. WebUI-Edits gelten instanzweit; ein gültiger Betreiber-Token reicht zur Verwaltung aller angebotenen Einstellungen. Technische Host-/Netzwerkgrenzen bleiben konfiguriert und durchgesetzt.
4. **Isolationslinie:** Profile und deploymentabhängige Auflösung verschwinden als Wahlmechanik. Eine feste Standardausführungspolitik entscheidet intern weiterhin, welche Isolation für nicht vertrauenswürdigen Code notwendig ist. Sandbox-/Network-/Secret-Grenzen dürfen erst entfernt werden, wenn konkrete Bedrohungsanalyse und Tests zeigen, dass sie redundant sind.
5. **Begriffe:** „Scope entfernen“ betrifft Autorisierungsdimensionen. Projekt-, Thread-, Run-, Extension- und Kanal-IDs bleiben dort, wo sie fachliche Beziehungen, idempotente Operationen, Replay oder Audit benötigen.
6. **Externe Kanäle:** Telegram/MCP/OAuth-/sonstige Fremdidentitäten sind keine WebUI-Betreiberidentitäten. Kanalzulassung und Conversation-Bindings regeln die Aufnahme von Eingaben; angenommene Aufgaben verwenden dieselben globalen Toolregeln. Kein zusätzliches kanal-, user- oder projektbezogenes Toolrollenmodell einführen. Der Betreiber-Token darf nie als Identität externer Absender eingesetzt werden.
7. **Settings-Vollständigkeit:** Jede der fünf Sicherheitsdimensionen erhält einen eigenen WebUI-Settings-Bereich. Pro Parameter existiert eine dokumentierte wirksame Quelle und Übernahmeregel; keine versteckten DB-/Env-/Run-Overrides. DB-Verbindung, Listener und initialer Token können einen expliziten Bootstrap-Weg außerhalb der WebUI benötigen. Dieser darf keine parallele Toolpolicy erzeugen. Geheimniswerte bleiben auslesegeschützt; Metadaten und Schreiboperationen sind sichtbar.
8. **Orchestrator-Lebenszyklus (bereits festgelegt):** genau eine globale Monty-VM pro laufender Instanz; Start beim Boot, dauerhafte Hintergrundausführung, getrennte Vorgangskontexte und kontrollierter Shutdown. Kein Per-Chat-/Per-Turn-Fallback. Kurzlebige CLI-Verwaltungsbefehle wie `doctor`, `completion`, `repair` oder `migrate` starten keinen Agentendienst. Jeder tatsächlich lang laufende Produkteinstieg verwendet dieselbe globale Boot-Komposition. Die technische Umsetzung und ihre Abnahme sind Phase 3a; diese Zielentscheidung steht nicht erneut zur Auswahl.

## 4. Umsetzungsplan

Jedes Umsetzungspaket besteht aus Vertrag, Änderung und zugehörigem Nachweis. Zuerst die betroffenen Verbraucher und Datenmigration entwerfen, dann implementieren, danach alte Verträge entfernen. Die produktive Migration erfolgt in Phase 8. Das vollständige Gesamtinventar darf unabhängige, abgegrenzte Verbesserungen nicht blockieren; für den jeweiligen Eingriff müssen alle betroffenen Verbraucher geklärt sein.

Priorität: Komponentenverträge aus Phase 0a und globale Toolentscheidung festlegen → Monty-1.0.0-Migration und Ressourcenvertrag klären → Monty-Fortsetzungen, faire Ausführung und Live-Budgets technisch nachweisen → Komponenten-/Binding-/Retrypfade implementieren und Produktionsboot/Dispatch verdrahten → Intents und WebUI vervollständigen → Datenmigration und produktiven Wechsel abnehmen. Phasen 0a und 1–3 brauchen vor Phase 3a klare Zielverträge; ihre gesamte Datenbereinigung muss dafür noch nicht abgeschlossen sein. Unabhängige Infrastrukturproben dürfen vorangehen, aber gewöhnliche produktive Recipe-Ausführung und Cutover benötigen die tatsächlich durchgesetzten Komponentenverträge und ihre Caller-Level-Abnahme.

### Phase 0 — Inventar, Bedrohungsmodell und Baseline

1. Vollständige maschinenlesbare Referenzliste für `ResourceScope`, Actor/Principal, Tenant/User/Project/Agent/Thread-IDs, `RuntimeProfile`, `DeploymentMode`, Profile-Environmentvariablen, TrustClass, Grants, Leases und WebUI-Caller erstellen.
2. Für jeden Scope-Verbrauch klassifizieren: (a) Autorisierung, (b) Storage-/Foreign-Key-Beziehung, (c) Verschlüsselungs-AAD, (d) Audit/Replay/Idempotenz, (e) bloße UI-Filterung. Keine Entfernung auf Basis von Namenssuche allein.
3. Erreichbare WebUI-Routen und Seiten den tatsächlichen Backend-Handlern zuordnen. Dateinamen/TODOs sind zunächst Kandidaten. Eine verbindliche Funktionsliste für die Vollversion erstellen: gewünschte Fähigkeiten, Komponenten-/Recipe-Abdeckung, Produktadapter, UI/API und Abnahmeszenario. Fachliche Funktionen dürfen nicht allein durch Löschen eines Platzhalters als erledigt gelten.
4. Für jede der fünf Schutzebenen einen **kanonischen Settings-Katalog** erstellen. Pro Eintrag Quelle, Typ, Default, Wertebereich, Wirkort, Validator, Sensitivität, Übernahmezeitpunkt und Neustartbedarf dokumentieren. Gespeicherte und wirksame Revision sowie Fehler-/Rollback-Verhalten festlegen. Bootstrap-Parameter gesondert ausweisen; doppelte Policyquellen auflösen.
5. Referenztests und Datenpfade erfassen: PostgreSQL, native Embedded-Postgres-Testrigs, Migration/Restore, E2E, Architekturgrenzen, CI und Build-Features. Vorhandene Testumgebungen wiederverwenden; fehlendes Docker allein ist kein Grund, native DB-Nachweise auszulassen.
6. Bedrohungsmodell auf Single-Operator aktualisieren: Token-Diebstahl, CSRF, Browser-XSS, Fernzugriff, SSRF, Prompt Injection, nicht vertrauenswürdige Extensions/MCP-Server, Prozessausführung, Geheimnisse, Backup/Restore und Kanal-Absender.
7. Architektur- und Leistungsbaseline aufnehmen: Profil-/Scope-/Rollen-/Featureanzahl; Cold-Boot bis Ready, Tier-0-/Tier-1-Latenz p50/p95, Matching-/IBS-Zeit, SQL-Roundtrips, VM-/Programmerzeugung, LLM-Aufrufe/Tokens, Durchsatz bei wartenden Aufgaben, Control-Reaktionszeit, Idle-CPU und Speicher nach vielen Turns. Testhardware, Datenbestand, Warm-/Cold-Cache, Last und Wiederholungen festhalten; vor Optimierungen messbare Ziele gemäß §5 vereinbaren.
8. Monty-Lebenszyklus vollständig erfassen: alle VM-Erzeuger und Resume-Pfade, Worker-/Trigger-Startreihenfolge, AcceptedMessageRef-Auflösung, Signale, Gate-Fortsetzungen, Subagent-/MCP-Aufrufe, Reply-Persistenz, `LimitedTracker`, stdout/Token-Zähler, Shutdown und Wiederanlauf. Den getaggten Monty-Quellcode aus Cargo.lock prüfen: Welche Grenzen gelten kumulativ, welche beim Resume und welche während Idle? Keine erfundenen Reset-/Snapshot-APIs in den Implementierungsplan übernehmen.

**Abnahmekriterium:** Eine Review-fähige Entscheidungsmatrix benennt für jede entfernte Dimension Ersatz/Disposition und Datenmigration; kein sicherheitsrelevanter Scope-Verbrauch bleibt ungeklärt.

### Phase 0a — Ground-truth-Komponentenverträge implementieren

Diese Arbeitspakete sind verpflichtende Voraussetzungen der produktiven Recipe-
und Cutover-Abnahme, nicht nur Dokumentationsaufgaben. Verträge vor Änderungen
an Stores, Host-Adaptern und Runnern festlegen; Implementierung und relevante
Tests mit den betroffenen Verbrauchern koordinieren. Die Runtime-Arbeiten bleiben
Infrastruktur, keine spezialisierten Rust-Tools für fachliche Workflows.

1. **Komponentenrollen und aktuelle Lücken inventarisieren.** Tool (0) als
   registriertes Rust-Primitiv, ToolSkill (13) als nicht ausführende IBS-Metadaten,
   Skill (1–3) als Prosa plus ausdrücklich zugeordneter PythonCode (22), Recipe
   (21) als geordnete Komponenten-/Datenflussanleitung behandeln. Der Skill
   erklärt dem Orchestrator die Toolbenutzung; Code implementiert sie; ToolSkill
   bereitet das Binding vor. Tatsächliche Store-Konstruktoren, Validatoren,
   Recipe-Editoren, Seeder, Retrieval, Composer und Hostpfade den Anforderungen
   zuordnen. Fehlende strukturierte Felder/Verbraucher als Umsetzungslücke
   ausweisen; keine neuen Felder in bestehende APIs hineinbehaupten. Neues
   ToolSkill-Metadatum braucht keine Rust-Kompilation; eine neue Primitive
   benötigt ihren getrennten Build-/Validierungs-/Registrierungspfad.
2. **Assoziation, Approval und Provenienz über den Validator-Owner abschließen.**
   Diese Implementierung erfolgt in [validator_v3.md Steps 2–3/8–10](validator_v3.md#remaining-implementation--consolidated-2026-10-22).
   Vorhandene unveränderliche Revision-/Review-/Association-Stores wiederverwenden;
   keine zweite Schema-/Writer-/Q2-Implementierung in diesem Plan.
3. **Ergebnisse dieses Owners übernehmen.** Die Runtime prüft die exakten
   genehmigten Kombinationen und kontrollierten system_seed-Nachweise. Authored
   benötigt menschliches Q2; ausgelieferte qualifizierte Seeds kein zusätzliches
   Instanz-Q2. Labels sind keine Evidenz. Verbleibende Aktivierungs-/Consumer-Gates
   hier abnehmen, statt die Reviewimplementierung erneut auszuführen.

4. **Vollständige unveränderliche Auswahl erhalten.** Matching und IBS verwenden
   eine konsistente Kataloggeneration und wählen die neuesten aktivierten,
   genehmigten Versionen. Recipe-UUID/Revision/Checksum, Variante, exakter
   step_link, ausgewählte Schritte/Reihenfolge/Inputlayout, alle Tool-/ToolSkill-/
   Skill-/PythonCode- und transitiven Kontext-/Codeabhängigkeiten sowie Approval-
   IDs gemeinsam im ephemeral BuildInstruction auflösen und über den Task-
   Snapshot-/Fortsetzungsvertrag festhalten. Eingebettete Varianten/Layoutdaten
   innerhalb der ausgewählten Reciperevision identifizieren; keine unabhängige
   Variantenversions-API erfinden. Inkompatible neueste aktive Verträge oder
   fehlende Kombinationsapproval führen vor Effekten zum expliziten Fehler;
   kein stiller Rückgriff auf ältere Versionen, kein Tier-2-Fallback. Laufende
   Tasks behalten ihre ursprüngliche Auswahl, ohne erneutes Matching/latest-
   Lookup oder neue Q2-Prüfung nur wegen einer Ersatzversion.
5. **Tatsächliche Toolimplementierungen pinnen.** Für jeden ausgewählten Tool-
   Stand unveränderliche Definition und kompatibles Implementierungshandle/
   Artefakt inklusive tatsächlichem Adapter-/ABI-Vertrag auflösen und erhalten.
   Metadatenchecksum plus veränderliche Datei oder latest-Handler unter demselben
   Namen reicht nicht. Laden/Registrieren muss exakt die ausgewählte Implementierung
   bereitstellen; fehlende Artefakte, falsche Checksums oder inkompatible ABI vor
   Effekten ablehnen. Alte Definitionen, Implementierungen und Approval-Evidenz
   bleiben verfügbar, solange Tasks/Child-Ausführung/Checkpoints sie benötigen.
   Bei inkompatiblen Host-/Binary-Upgrades offene Arbeit drainen/reconciliieren,
   statt eine nicht nachgewiesene Weiterverwendung alter Ausführung zu behaupten.
6. **Exakte typisierte Bindings bis zum Runner umsetzen.** Die in recipe.md definierte
   whole-value-Referenzgrammatik `{{vars.name}}` mit Namen `[a-z][a-z0-9_]*`
   nur in deklarierter Referenzmetadatenposition parsen; keine Ausdrücke,
   eingebetteten Textfragmente oder Auswertung von Benutzerdaten. Captures
   explizit validieren/typisieren; fehlgeschlagene Regexverfeinerung darf nicht
   ungeprüft zum Rohslot werden. Schrittlokale Namen auf Taskinputs, typisierte
   Konstanten oder benannte frühere Ergebnisse abbilden und separat vom Python-
   Quelltext als `inputs["local_name"]` bis in den tatsächlichen Monty-Pfad
   transportieren. Plain-Text-Quellsubstitution aus dem Zielpfad entfernen.
   Die in skills.md definierten rekursiven ValueSchema-, Presence-/Null-/Default- und Kompatibilitäts-
   regeln implementieren: Listelemente, Objektfelder, typisierte Zusatzfelder,
   unabhängig materialisierte Defaults nur für fehlende Consumerinputs und keine
   erfundenen Outputs. Numerische Transport-/Toolgrenzen und tatsächlich berechnete
   Argumente vor Dispatch prüfen; keine Bool-als-Integer-Coercion, Rundung oder
   Clamping. Startup prüft deklarierte Datenkanten, nicht Ergebnisse noch nicht
   ausgeführter Schritte. Konkrete Inputs/Outputs prüfen, sobald sie verfügbar
   sind; optionale/nullfähige Felder und Listenindizes benötigen sichere Guards.
7. **Komponentenschritte und interne Assembly validieren.** Persistierte IBS-
   Syntax aus recipe.md verwenden: knowledge/stepnumber und vorhandene Typen;
   keine erfundenen channel/step_id/llm-Insertfelder. Jeder component-Schritt
   muss genau eine UUID enthalten; leere und mehrfache Includes ablehnen.
   Interne PythonCode-Komposition erlaubt geordnete kleine Bausteine mit
   vollständigen Verträgen; zyklische/fehlende Referenzen, Symbol-/Inputkonflikte
   und unvereinbare Verträge ablehnen und alle Versionen pinnen. Pure Logic
   macht null Toolcalls; eine Benutzung normalerweise einen. Unabhängige
   Dispatches brauchen separate Schritte. Nur die direkte abhängige Kette aus
   recipe.md erlaubt mehrere Calls als eine Einheit, mit nachgewiesener Binding-
   Abdeckung; kein mehrtooliger Leaf Skill und kein Umgehen der Paarregel.
8. **ToolSkill-Metadaten tatsächlich vorbereiten.** Die ausgewählte class-13-
   Referenz und genehmigte Assoziation deterministisch zum class-0-Tool, dessen
   tatsächlichem Dispatch-ID-/Callable-/Adaptervertrag und ausgewählter
   Implementierung auflösen. Die normale Rust-Bindingstufe direkt mit der
   passenden class-22-Ausführungsstufe paaren und Verfügbarkeit prüfen, bevor
   Code darauf zugreift. ToolSkill-Parametervertrag, registriertes Toolschema,
   fixe Selektoren und Skill-Argument-/Computed-Argument-Verträge müssen
   kompatibel sein. Legacy-param_template oder beschreibender content führt
   nichts aus und überschreibt keine Codeargumente. Aktuelle rust_directives
   bzw. leere tool_bindings aus Seedhelpers sind kein Beleg dynamischen Ladens;
   jeden konkreten Vorbereitungs-/Hostpfad durch den Caller nachweisen.
9. **Exakten Fehler-/Retryvertrag aus skills.md verdrahten.** failure mit
   action, max_attempts, idempotency, idempotency_evidence_ref und
   retryable_outcomes strukturiert prüfen und im Orchestrator-/Runnerpfad
   durchsetzen. max_attempts ist eine positive ganze Zahl ohne Boolwerte,
   einschließlich des ersten Dispatchs; stop verlangt 1, retry mindestens 2.
   Zähler an der logischen Schrittinvocation dauerhaft über Waits, Reclaim,
   Attemptwechsel und Recovery erhalten; Ausführungsattempt und Invocation
   unterscheiden. retry erlaubt nur ausdrücklich gelistete
   confirmed_no_effect_transient/unknown_completion mit vertrauenswürdiger
   read_only- oder getesteter dauerhafter deduplicated-Evidenz für diese Nutzung.
   not_assumed erlaubt keine Retries. Bei Deduplication denselben dauerhaften
   Key und dieselben Argumente über den gesamten Fortsetzungszeitraum erhalten.
   Timeout ist kein No-Effect-Nachweis; unbekannte Fertigstellung braucht vom
   Vertrag abgedeckte Evidenz oder explizite sichere Reconciliation. Legacy-
   ignore/retry/fallback-Labels allein erfüllen diesen Vertrag nicht. Aktuelle
   Policy, Authbedarf, Cancellation, Claims/Attempts und Ressourcen vor jeder
   zulässigen Wiederholung prüfen; Fehlerantworten nie zu Erfolg umdeuten.
10. **Durablen Dispatch-/Effektvertrag und Aktivierung abnehmen.** Vor Dispatch
    Invocation/Versuchszähler, Dispatchabsicht, ausgewählte Workflowreferenzen,
    nötigen Key/Argumente und Fortschritts-/Effektstatus über unterstützte
    Persistenz festhalten. Scheitert die notwendige persistente Aufnahme, kein
    unprotokollierter Dispatch. Unterbrochene Aufrufe ohne Ergebnis sind ungeklärt,
    nicht automatisch effektlos. Ergebnisse/Completion dauerhaft der Invocation
    zuordnen; bestätigte Effekte bei Output-, Reply- oder Folgeschrittfehlern
    niemals wiederholen. Geheimnisse bleiben im geschützten Hostbereich, nicht
    in model-sichtbaren Fortsetzungen. Tatsächliche Transaktionen/Unique-Keys/
    Claim-Fencing verwenden, keine atomare DB-plus-Fremdservice-Transaktion
    behaupten. Rust stellt Persistenz/Fencing bereit; Recipe/Python sequenziert
    fachliche Wiederholung/Reconciliation. Zusammenhängende aktive Generationen
    erst nach gültigen Verträgen, Approval, Artefaktverfügbarkeit und passenden
    Binding-/Runnernachweisen veröffentlichen. Altaufgaben behalten ihr Manifest.

**Abnahmekriterium:** Für jeden Vertrag sind Store/Migration, Validator,
IBS/Composer, Host/Runner, Aktivierung und relevante WebUI-Verbraucher benannt
und umgesetzt, soweit sie den Vertrag verwenden. Reale PostgreSQL-/Caller-Level-
Tests aus Phase 7 belegen die Zielpfade; kein bloßer Schemaentwurf, metadata-
Insert oder Helfertest gilt als vollständige Durchsetzung. Fehlende Nachweise
sperren gewöhnlichen produktiven Recipe-Cutover, nicht unabhängige sichere
Entwicklungsarbeit oder explizite eingeschränkte Draft-Validierung.

#### Gemeinsame Runtime-Verträge für die Komponentenmigration

Dieser Plan implementiert die folgenden gemeinsamen Mechanismen für
[monty.composition](docs/plans/monty.composition.md#concurrent-implementation-with-simplified-v3)
und dessen Anhänge. Die Komponenten-/Commandmigration samt engen fachlichen
Primitivprofilen bleibt dort; Validator behält Review/Evidenz, Prefix seine
Compiler-/Generierungs-/Servinglogik und V35-13/14 dynamische Kontrollstrukturen.
Vorhandene Store-, Interface-, Preload-, Child-, Chat- und Deployment-Unterstützung
ist der Ausgangspunkt. Nur nachgewiesene Lücken in ihren bestehenden Ownern
schließen; keine zweite Registry, Queue, VM, Approval- oder Komponentenablage.

**Skill, Prüfung, Deployment und MCP getrennt halten.** Ein Skill hat kein
verpflichtendes eigenes Recipe. Ein konsumierendes oder prüfendes Recipe erzeugt
keine Eigentumsbeziehung; Validator-Recipes dürfen die eigenständige Nutzung prüfen.
Der vorhandene Deployment-Pfad veröffentlicht vollständige Recipe-Graphen. Das
belegt keine unabhängige Aktivierung ungenutzter Bibliothekskomponenten und verlangt
kein zusätzliches MCP-Recipe. Exakte Usage-, Workflow- und MCP-Kommandoevidenz bleiben
getrennt. Optionales `mcp_call` gehört zur unveränderlichen Recipe-Revision; fehlende
Deklaration ist zulässig, eine vorhandene fehlerhafte Deklaration bleibt ein Fehler.
Entzug/Änderung nutzt den bestehenden Revisions-/Freigabe-/Publikationsweg. Bereits
erfasste Tasks behalten ihre Auswahl; alte Kommandos vor Taskaufnahme werden bei
geändertem/entzogenem Vertrag abgelehnt. Keine zweite Registry oder Ausführungslane.

1. **Exportauflösung und Aufrufzustand durchgängig erhalten.** Composition liefert
   kanonische UUIDs, checksummierte Interfaces/Implementierungssymbole, explizite
   private Helper-/Konstantenabhängigkeiten, fixe/berechnete Argumente und genaue
   Producer-/Consumerverträge. Phase 0a löst sie in einer Katalogauswahl auf und
   erhält Export-/Binding-/Artefaktauflösung auch nach verschachteltem Aufruf,
   Child und Wait. Mehrdeutige Export-/Symbol-/Aliaszuordnung, kollidierende Namen,
   Zyklen, fehlende Imports oder Initialisierungseffekte vor Dispatch ablehnen.
   Gleiche öffentliche Namen in alter/neuer Auswahl dürfen einander nicht
   überschreiben. Mutable Defaults/Arrays/Closures/Inputs/Ergebnisse bleiben
   invocationbezogen, auch beim Wiederverwenden reiner Helper. Unsupported rohe
   Monty-Werte vor verlustbehafteter Konversion ablehnen; ein echter String mit
   Markertext bleibt zulässig. Der bestehende gewöhnliche retained Pfad bleibt
   bei einem gebundenen Toolcall je Ausführungsschritt und `stop`/`max_attempts: 1`,
   bis sein Owner einen erweiterten Binding-/Retryvertrag wirklich qualifiziert.
   Keine dokumentierte Kettenausnahme oder Read-only-Evidenz schaltet ihn allein frei.
2. **R09 über die bestehende Child-Aufnahme vervollständigen.** Den vorhandenen
   expliziten Workflow-/Inputpfad mit Parentclaim, ursprünglicher nativer Invocation,
   Child-Run und Original-Katalog binden. Composition liefert endliche rekursive
   Input-/Result-/Fehlerprofile und die tatsächlichen Parent-/Subagentrecipes;
   diese Phase implementiert fehlende Admission-/Fortsetzungs-/Result-Custody.
   Parent wartet ohne Worker-/VM-Blockade; Child matcht nicht neu und liest nicht
   latest. Aufnahmebeleg und terminales Child-Ergebnis bleiben getrennt; echte
   Reply-/Run-/Fehlerdaten einschließlich optionalem nullable sanitisiertem Fehler
   nach dem deklarierten Profil liefern, nie `authorized:true` oder leeren Erfolg
   als Abschluss. Lost-Ack, Cancel, Sättigung, stale Parentattempt, unlesbares oder
   ungeklärtes Ergebnis bleiben am ursprünglichen Child; keinen Ersatzchild nach
   Timeout erzeugen. Andere Tasks/Control müssen während Parentwait fortschreiten.
   Composition migriert alle fünf Tier-1-Subagentworkflows und ihre Consumer erst
   nach qualifizierter Parent-Binding-/Export-/Bootstrap-Abnahme. Keine neue
   spezialisierte Child-Engine oder Wiederbelebung des Flavor-/Task-Legacypfads.
3. **MCP mit gewöhnlichem Chat-Lifecycle versorgen.** Bestehende authentifizierte
   Create-/Send-/Observe-/Close-Ports samt nicht destruktiver Schließung und
   bestätigten terminalen Reply-/Historyreferenzen verwenden. Composition besitzt
   Protokoll, Commandprüfung, Listing und MCP-Request-/Chatkorrelation; diese Phase
   ergänzt nur fehlende gewöhnliche Produkt-/Runtimeunterstützung. Exakte Message/
   Run/Attempt-Zuordnung statt neuester Nachricht verwenden. Unsicheres Submit
   idempotent nachlesen; Delivery-/Closefehler am Originalchat reparieren, ohne
   neue Nachricht/Chat oder Replay des Effects. Ein wartender MCP-Parent darf
   die Ausführung seines normalen Childchats nicht blockieren; vorhandene faire
   begrenzte Worker-Claims und denselben Monty-Account nutzen. Disconnect ist
   weder Cancelbeweis noch Löschung laufender Arbeit. Kein direkter MCP-Zugang zu
   Monty/IBS/Tools, kein zusätzlicher Reply-/Historywriter oder Chat-Ausführungsmodus.
4. **Listener und Kohai sauber in gemeinsame Lebenszyklen einhängen.** Die eine
   Startup-/Shutdown-Reihenfolge übernimmt bestehende Listener-Ready-/Cleanup-
   Hooks und baut das vollständige qualifizierte Listing vor Werbung auf. Keine
   frühe Producer-/Providerwerbung vor Bereitschaft; Fehler nicht als erfolgreiche
   leere Ersatzliste maskieren. Der Server bleibt bei Providerdisconnect und
   Idle aktiv. Composition implementiert das tatsächlich unterstützte Kohai-
   MCP-Protokoll und requestlokales Advertisement/Cleanup; §10.1 liefert Provider-
   Revision/Auth/Availability. Prefix liefert den finalen Prefix; danach unmittelbar vor Send verbinden/
   advertisen. Der eine finale Requestguard prüft den tatsächlich gesendeten
   Request einschließlich seiner Tool-/Advertisement-Metadaten; Intermediate Toolroundtrips
   sind kein fertiger Modellanswer. Complete-Answer/Cancel/Error räumt genau diese
   Verbindung auf, ohne andere Exchanges oder den Server zu stoppen. Fehlende
   Providerprotokolle bleiben explizit unsupported, ohne heimlichen Fallback.
   Listenerfehler folgen dem bestehenden readinessabhängigen Ingressvertrag;
   Diagnose und sichere unabhängige Aufgaben erhalten ihre eigenen Freigaben.
5. **Komponentenpakete unter einem Deployment-/Restorevertrag migrieren.**
   Composition liefert pro Familie die installierte UUID-/Revision-/Alias-/
   Consumer-/Override-Zuordnung und eindeutige Reuse-/Retire-/Blocked-Ziele, keine
   anhand von Namen erratenen Identitäten. Phase 2/6 implementiert fehlendes
   hostabgeleitetes Instanzrouting vorhandener Storageadapter; fachliche Objekt-
   und Source-/Derivedbeziehungen bleiben erhalten. R06–R08/N16/N20 definieren
   ihre Operationen im Composition-/Prefix-Owner, nicht in einem zweiten Store.
   Genehmigte ausführbare Revisionen und mutable Pending-Dokumente unterscheiden;
   ein Dokument-Upsert publiziert keinen Code. Vollständige Quellsnapshots, deren
   bestätigte Speicherreferenzen, Hash-/Darstellungsvertrag und abgeleitete Texte
   mit getrennten Effektreceipts sichern; nummerierte Vorschau ist kein Original.
   Fehlende fachliche Lineageoperationen bleiben explizite Primitive-/Usagearbeit,
   physische Sicherung/Reader-/Routingmigration gehört Phase 6/8. Ausgelieferte
   exakte Pakete durch vorhandenen noncircular Seed-/Integrity-/Behaviorpfad
   qualifizieren und Overrides erhalten; authored Änderungen nutzen Validator-Q2.
   Aktivierung über §8.2 veröffentlicht genehmigte Closure und Invalidierung;
   Prefix baut danach. Neue Legacy-Aufnahme familienweise erst nach erfolgreicher
   Consumerabnahme ersetzen. Alte Exporte, native Artefakte, Approvals und Commands
   erhalten, solange Tasks/Waits/advertisierte Requests sie benötigen; ein vor
   Chat-Submit nicht mehr kompatibler Command wird vor Effekten abgewiesen, statt
   eine alte Recipe heimlich auszuwählen. Runtime-/Binary-Rollback niemals aus
   einem bloßen Komponenten-Auswahlrollback ableiten.

Gemeinsame Abnahmefälle: alte/neue gleichnamige Exporte mit isolierten mutablen
Daten; Parent A wartet, unabhängiger Task B läuft, Child liefert genau sein Ergebnis;
Child-Aufnahme-/Replyverlust und Live-Toolblock ohne Ersatzdispatch; MCP-Roundtrip
bei begrenzten Claims und getrennten Send-/Delivery-/Closefehlern; Request-lokales
Kohai-Cleanup bei parallelen Exchanges; Familienupgrade/Restore mit Override,
altem Wait, fehlendem alten nativen Artefakt und Source-/Derivedreferenzen.
Dafür die echten PostgreSQL-/Monty-/Host-/Chatcaller verwenden. Gemeinsame Belege
am implementierenden Owner einmal führen; Composition ergänzt seine konkreten
Familien/Commands/Subagentvarianten. Unveränderte qualifizierte Mechanismen nicht
neu implementieren oder in einer zweiten identischen Testpipeline prüfen.

### Phase 1 — Einzige Instanz- und Betreiberidentität

1. Einen instanzweiten `Instance`-Kontext definieren. Der authentifizierte WebUI-Aufrufer ist stets `Owner`; keine vom Client gelieferte User-/Tenant-/Project-ID darf diese Identität verändern.
2. `WebUiAuthenticatedCaller` auf minimal notwendige Session-/Request-Metadaten reduzieren. Handler erhalten keine vom Browser frei wählbare Berechtigungsidentität.
3. Entfernen: WebUI-Userverwaltung, Rollen-/Benutzerwechsel, Login/OIDC-/DB-Auth-Varianten, tenant- oder userbasierte WebUI-Zugriffsregeln, sofern ausschließlich Bestandteil des aufgegebenen Multi-User-Betriebsmodells. Externe OAuth-Verbindungen, die Drittanbieterzugriff für Funktionen ermöglichen, sind fachlich zu unterscheiden und nicht pauschal zu löschen.
4. Tokenkonfiguration auf einen Betreiberzugang vereinheitlichen. Bootstrap, Rotation/Reset und Widerruf bestehender Sessions eindeutig definieren. Keine Klartext-Logs; konstante Tokenvergleiche und sichere Session-Erzeugung beibehalten. Wiederherstellung darf keine neue Benutzer-/Rollenverwaltung benötigen.
5. Listener sicher standardisieren: lokale Bindung als Default; expliziter Fernzugriff bleibt erreichbar, aber nur über dokumentierte Betreiberkonfiguration mit Auth, TLS-/Reverse-Proxy-Hinweisen, Origin-/CSRF-Schutz und Rate-Limits. Ungeschützter Remote-Bind darf nicht aus „ein Token = volle Rechte“ folgen.
6. Login-E2E-Tests umstellen: gültiger Token erhält die vollständige Verwaltungsoberfläche samt tatsächlichen Settings-Schreiboperationen; ungültiger/fehlender Token erhält keinen Zugriff. Rotation, Sessionwiderruf, CSRF/Origin und Sessionablauf bleiben getestet.

### Phase 2 — Autorisierungsscopes entfernen, Daten-IDs entkoppeln

1. `ExecutionContext` und `ResourceScope` für den unterstützten Einzelinstanzbetrieb in einen kompakten `InstanceContext` plus erforderliche fachliche Objekt-IDs überführen.
2. Capability-Entscheidung auf registriertes Tool + aktuelle instanzweite Zulassung/Sperrung + technische Instanzregeln reduzieren. Keine user-, projekt-, agent- oder tenantbezogenen Toolgrants und keine zusätzliche operationsbezogene Freigabe.
3. Alte Tool-Approval-Leases, Invocation-Freigaben und zusätzliche `Ask`-Entscheidungen aus allen produktiven Toolpfaden entfernen. Globale Einstellungen bestimmen den Dispatch. Worker-Claims/Lease-Tokens für Ausführungsownership, externe Auth-Waits und Q1/Q2-Komponentenvalidierung bleiben erhalten und bekommen eindeutig getrennte Begriffe.
4. Fachliche IDs für Datenbeziehungen, Eingabezuordnung und Ausführungskorrektheit erhalten. Verwaltungszugriff wird durch den Betreiber-Token geprüft. Externe Ingress-Verarbeitung behält ihre Provenienz und Conversation-Bindings ohne Betreiber-Verwaltungsidentität.
5. Nur redundante Autorisierungsfilter und Storage-Strukturen vereinfachen. Stabile Objekt-IDs und funktionierende Pfade bevorzugt erhalten. SQL-Keys, Filesystem-Präfixe und Indizes nur mit konkretem Nutzen und zuvor entworfener Migration ändern; Entfernen von Rollen erzwingt keinen vollständigen Dateisystemumbau.
6. Secrets: alle Betreibergeheimnisse instanzweit speichern; Verschlüsselung/AAD an Instanzkennung + Handle binden. Keine per-User-/Projektvererbung oder Scope-Fallbacks. Bestehende Daten werden verschlüsselt migriert; Klartext wird weder in Logs noch temporären Migrationstabellen ausgegeben.
7. Memory, Threads, Events, Traces, Produktadapter, Trigger und Outbound prüfen. Globale Betreiberansicht ist erlaubt; externe Kanal-Identitäten/Conversation-Bindings bleiben fachliche Provenienz und kein Berechtigungsersatz.
8. Architekturtests verbieten rollen-/scopebasierte Tool- und Betreiberberechtigungen. Fachliche ID-Nutzung, Message-/Run-Zuordnung und Prüfung aktueller Claims/Attempts bleiben ausdrücklich erlaubt; diese begründen keine zusätzlichen Toolrechte.
9. Die fachlichen Schlüssel für Monty beibehalten: exakte Eingabereferenz, Turn-/Run-ID, Conversation-/Thread-ID, Reply-Binding, Parent-/Child-Beziehung sowie Claim-/Attempt-Identität. Scope-Vereinfachung darf weder Nachrichtenauswahl noch Abbruch-/Fortsetzungsadressierung auf einen einzigen globalen Schlüssel reduzieren. Phase 3a nutzt diese IDs ausdrücklich ohne sie zu neuen Betreiberrollen zu machen.

### Phase 3 — Eine feste Runtime- und Sicherheitsrichtlinie

1. `RuntimeProfile`/`DeploymentMode`-Resolver durch `DefaultSecurityPolicy` (oder gleichwertige einzelne Typen) ersetzen. Kein CLI-/WebUI-/Env-Weg soll ein Profil pro Run, Benutzer oder Workspace wählen können.
2. Aus `EffectiveRuntimePolicy` nur Felder behalten, die realen Host-Mechanismen entsprechen. Eine zentrale Konfiguration setzt Filesystem-, Prozess-, Netzwerk-, Secret-, globale Tool- und Ressourcenregeln. Operationsbezogene Approval-Regeln entfallen.
3. Profilwahl über Env, CLI, TOML und Registry entfernen oder zur Diagnose der einen aktiven Policy umbauen. Benötigte Recipe-/Run-Ausführungsdaten anhand ihrer Bedeutung erhalten; ähnliche Typnamen sind kein Löschkriterium.
4. Einen festen Standard für nicht vertrauenswürdige Ausführung festlegen: sandboxed Prozesslauf, kontrolliertes Filesystem, brokered egress, minimale Umgebung, Secret-Broker, Ressourcenlimits und Audit. Wo Betriebssysteme unterschiedliche Backend-Implementierungen benötigen, hinter einer internen Plattformabstraktion, nicht als wählbare Edition/Profile.
5. Eine zentrale Toolentscheidung am `CapabilityHost` durchgängig verdrahten: Tier 0/1/2, First-party, MCP, Subagenten und administrative Hosttools verwenden dieselbe aktuelle Instanzkonfiguration. Globale Einstellungen speichern und als versionierten wirksamen Satz bereitstellen. Engine-Leasefilter und statische `Ask`-Defaults dürfen keine zusätzliche Freigabeschicht erhalten. ToolSkill-Bindung ist Verfügbarkeit, keine Berechtigungsvergabe.
6. Globale Toolregeln live übernehmen: Settings-Schreibvorgänge sind revisionsgeprüft, parallele Änderungen erzeugen keinen stillen Verlust. Vor jedem Dispatch die aktuelle Revision prüfen; eine Sperrung verhindert nachfolgende Aufrufe auch in laufenden Recipes. Bereits gestartete Aufrufe anhand ihres Status behandeln, ohne erneuten Dispatch. Technische Obligations-/Ressourcen-/Auth-Regeln bleiben erhalten; keine Einzelfreigaben wieder einführen.

   **Stabile Policyidentität gemäß tools.md:** jedes registrierte Capability-ID-/
   Callable-Alias eines Tools einschließlich aller von Tasks behaltenen alten
   Versionen vertrauenswürdig auf dieselbe stabile Tool-UUID abbilden. Eine
   Sperrung dieser Identität sperrt jeden folgenden Dispatch über diese Versionen/
   Aliase. Umbenennung darf weder eine neue Allow-Regel erzeugen noch eine alte
   Entscheidung konservieren. Fehlende, widersprüchliche oder mehrdeutige
   Zuordnung sowie fehlende/ungültige aktuelle Policy führen zu fail closed.
   Der heutige Capability-ID-basierte Authorizer ist Ausgangspunkt, kein Beleg
   einer vorhandenen UUID-basierten Policy/API; Mapping und Konsistenz tatsächlich
   implementieren. Diese Live-Entscheidung bleibt vom gepinnten Code getrennt.

7. `RuntimeKind`/MCP/FirstParty/System-/Prozess-Lanes nur dann zusammenlegen, wenn Sandbox, Secret-Übergabe, Netzwerk und Ergebnis-/Prozesskontrolle gleichwertig sind. Backend-Auswahl darf nicht mehr an Produktedition, User oder Runtime-Profil gekoppelt sein.
8. Tests beweisen: fehlender/ungültiger Betreiber-Token sperrt Verwaltung; global gesperrte Tools erzeugen keine neuen Effekte; globale Zulassung erfordert keine zusätzliche Aufruffreigabe; Loop/Extension kann die globale Einstellung nicht umgehen. Aufruf aus allen Produkt-/Toolpfaden, Live-Sperrung, parallele Settingsänderung, Sandbox sowie Netzwerk-/Secret-Broker prüfen. Eine angenommene Aufgabe benötigt keine Browser-Session für jeden Toolcall.

   Zusätzlich Version 1 suspendieren, Version 2 mit geänderter Dispatch-ID/Alias
   aktivieren und das Tool global sperren: beide Versionen verweigern folgende
   Calls. Fehlende/konfligierende Mappings und fehlende Policy verweigern Dispatch;
   kein gespeichertes Bind-Time-Allow ersetzt die aktuelle Entscheidung.

### Phase 3a — Globalen Monty beim Boot starten und dauerhaft betreiben

**Voraussetzung — Monty auf 1.0.0 migrieren, bevor der globale Produktionsdienst verdrahtet wird:**

Online-Prüfung vom 2026-10-06: [Monty 1.0.0 vom 2026-09-25](https://github.com/pydantic/monty/releases/tag/v1.0.0) ist das geprüfte stabile Upgradeziel; Engine und Komposition verwenden derzeit den Git-Tag v0.0.16. Das Upgrade ist eine Infrastrukturmigration, keine Änderung des Recipe-/Zwei-Kanal-Vertrags und noch nicht umgesetzt. Folgende Arbeiten gehören vor die Fortsetzungs-/Fairness-/Live-Budget-Nachweise dieser Phase:

- Beide Monty-Dependencies und Cargo.lock gemeinsam auf dieselbe geprüfte 1.0.0-Version festlegen. Rust-Mindestversion 1.96 in Workspace, CI, Docker und Release-Builds berücksichtigen; das bisherige Workspace-Minimum 1.94 reicht nicht. Lokal vorhandenes Rust 1.98.1 genügt. Keine ungeprüfte main-Revision verwenden.
- API-, Typ-, Resume-, Fehler- und Serialisierungsänderungen inventarisieren und die bestehenden Host-/Recipe-Adapter anpassen. [Feed-/Turn-Zeitlimits](https://github.com/pydantic/monty/pull/880) zählen nur ausführende VM-Zeit; ein Feed umfasst einen Snippet-Aufruf, ein Monty-Turn endet am nächsten Hostübergang und ist nicht identisch mit einem BrassClaw-Task. Den 600-Sekunden-Taskvertrag deshalb über alle zugehörigen Feeds, Schritte, verschachtelten Ausführungen und Fortsetzungen führen. Keine Session-Lebensdauerbegrenzung und keinen Reset des Taskverbrauchs beim Resume einführen.
- Die [generische Custom-Tracker-Schnittstelle wurde entfernt](https://github.com/pydantic/monty/pull/613/files). Der für v0.0.16 mögliche eigene `ResourceTracker` ist kein unverändert übertragbarer Lösungsweg. In der [1.0.0-Ressourcenimplementierung](https://raw.githubusercontent.com/pydantic/monty/v1.0.0/crates/monty-types/src/resource.rs) setzen `set_max_feed_duration` und `set_max_turn_duration` ihre jeweiligen Zeitakkumulatoren zurück. Einen nachgewiesenen Adapter-/Upstream-Erweiterungsvertrag für Live-Revisionen und erhaltenen Verbrauch festlegen; Upgraden allein erfüllt das nicht. Falls eine Engine-Erweiterung benötigt wird, isoliert, getestet und ausdrücklich versioniert halten.
- [`max_allocations` wurde upstream entfernt](https://github.com/pydantic/monty/pull/611). Vor Settings-/DB-/UI-Cutover seine Disposition festlegen: nur mit einem tatsächlich nachgewiesenen separaten Task-Allokationszähler weiter anbieten; andernfalls als abgelösten Parameter ausdrücklich migrieren und WebUI/API/Abnahme konsistent auf unterstützte Ressourcenregeln umstellen. Bestehende Betreiberwerte nicht stillschweigend ignorieren oder in Bytes umdeuten. Kein unwirksames Eingabefeld und kein erfundenes 1.0.0-Allokationslimit ausliefern.
- Das 1.0.0-Speicherlimit benötigt den installierten und aktivierten `monty-alloc`-Allocator. Dessen prozessweite Messung/Baseline ist nicht automatisch ein isolierter Monty-Heapzähler im BrassClaw-Rust-Prozess. Vor Einbau Messbereich, Nebenläufigkeit, Fremdallokationen, globale Allocator-Kompatibilität und Fehlerbereich prüfen; konfigurierbare Heapziele und technischen Allocator-Backstop getrennt und ohne Doppelzählung abbilden. Bleibt Einbettung im selben Prozess technisch ungeeignet, eine isolierte, instanzweit langlebige Hosting-Lösung prüfen; keine globale VM pro Task und keine ungeprüfte Übernahme einer Poolarchitektur als zweiter Recipe-Loop. Ohne wirksamen Allocator- und Live-Limit-Nachweis kein Speicherabnahmeerfolg.
- Zuerst Caller-Level-Kompatibilität nachweisen: bestehende Tier-0-/Tier-1-/No-Match-Recipes, Isolation zwischen unabhängigen Tasks/Attempts, erhaltenen Recipe-Zustand, abhängige Toolketten, A wartet → B läuft → A setzt fort, Hostfehler/Cancel und verschachtelte `host.run_program`-Fortsetzungen. Zusätzlich neue Zeit-/Speicherregeln, Control-Reaktionszeit und verbrauchserhaltende Live-Revisionen prüfen. Dump-/Fortsetzungsformate ändern sich; vor Versionswechsel alte Aufgaben drainen/reconciliieren, keine Kompatibilität alter RAM-/Dumpzustände behaupten. Speicher-/Async-Korrekturen des Releases sind Upgradegründe, kein Beleg für höhere BrassClaw-Geschwindigkeit; Baseline vor/nach Migration gemäß §5.1 vergleichen.

**Upgrade-Gate:** Erst nach diesen Nachweisen 1.0.0 als produktive Basis übernehmen und die folgenden Phasenschritte darauf abschließen. Einen separaten, überprüfbaren Upgradecommit und Rückkehr zum vorherigen kompatiblen Binary/DB-Stand vorsehen. Ein einfaches Anheben des Git-Tags gilt nicht als abgeschlossene Migration.

**Verbindliche Monty-Ressourceneinstellungen:** `max_duration_secs` und `max_memory_bytes` müssen im Monty-Settings-Tab der WebUI instanzweit einstellbar sein. Der bisher verlangte Parameter `max_allocations` folgt der oben verbindlich festzulegenden Upgrade-Disposition; nur nachweislich wirksame Grenzen anbieten. Die vorhandenen `MontyVmSettings`-/DB-/API-Verträge wiederverwenden und im Zuge der Scope-Vereinfachung auf Instanzwerte überführen. Iststand: Der DB-Store führt alle drei Felder, `monty-vm-tab.js` bearbeitet/speichert bisher nur die Laufzeit, und `MontySession::new` setzt Speicher und Allokationen weiterhin fest. Diese Lücke ausdrücklich schließen; ein gespeicherter DB-Wert allein ist keine wirksame Runtime-Konfiguration. Für Speicher ist „Beim Start bestimmen“ der Standard: einmalig konservativ dimensionieren, danach bis zur Betreiberänderung konstant halten. Den Vertrag um Modus, Reserve und endlichen Ersatzwert erweitern; laufende automatische Anpassung ist ausdrücklich optional. Bestehende explizite Betreiberwerte beim Upgrade erhalten; die bisher fest im Code gesetzten 128 MiB sind kein Zieldefault.

**Budgetbedeutung und Live-Übernahme:**

| Einstellung | Gezählt wird | Verhalten bei Änderung / Überschreitung |
|---|---|---|
| `max_duration_secs` (Default: **600 Sekunden / 10 Minuten**) | Ausführende VM-Zeit je Aufgabe, einschließlich ihrer verschachtelten PythonCode-Ausführung; Idle, Queue und externe Auth-/Provider-/Tool-Waits zählen nicht mit | Aktuelle Revision gilt am nächsten begrenzten Prüfpunkt; bereits verbrauchte Zeit bleibt erhalten. Überschreitung beendet die betroffene Aufgabe. Externe Aufrufdeadlines sind separat. |
| `max_allocations` (Upgrade-Disposition erforderlich) | Falls separat implementiert: kumulative, einer Aufgabe zurechenbare Allokationen über Schritte und Fortsetzungen; in Monty 1.0.0 kein eingebautes Limit | Nur bei nachgewiesener Umsetzung Verbrauch über Settingsänderungen erhalten und den betroffenen Task begrenzen. Andernfalls Parameter ausdrücklich ablösen und Bestandswerte migrieren; kein stiller No-op. |
| `max_memory_bytes` | Wirksame Grenze des gemeinsamen, aktuell belegten Monty-Heaps einschließlich zurechenbarer verschachtelter Ausführung; keine pauschale Grenze für den gesamten Rust-Prozess | Standardmäßig einmalig beim Start aus verfügbarer Kapazität und Reserve konservativ bestimmen; danach keine periodischen Druckmessungen. Manuell live einstellbar; automatische Anpassung nur auf ausdrückliche Aktivierung. Live anpassen, ohne Verbrauchszähler zurückzusetzen. Automatische Senkung unter den lebenden Heap bleibt ausstehend und aktiviert Entlastung/Backpressure; eine nicht einhaltbare manuelle Senkung ausdrücklich ablehnen. Ein globaler Heapfehler kann beaufsichtigte Dienst-Recovery erfordern. |

**API-Ersatzobjekt ohne stille Defaults:** Ein explizites `execution_limits`-Objekt ist ein vollständiger, revisionsgeprüfter Ersatz und muss alle Felder der aktuellen GET-Antwort enthalten. Fehlende, unbekannte, doppelte, falsch typisierte oder nullable Einzelfelder vor Persistenz/Workerwirkungen ablehnen; ein älterer Client darf neue Betreiberwerte nicht durch Deserialize-Defaults zurücksetzen. Fehlendes oder `null` gesetztes gesamtes Objekt behält die bestehenden Top-Level-Patchregeln. Kompatibilitätsdefaults beim Lesen alter DB-Snapshots bleiben davon getrennt. Native HTTP-Abnahme prüft abgewiesene echte Requests mit unveränderter DB-Revision, Laufzeitpolicy und VM-Identität sowie eine gültige Daueränderung ohne Ersatzobjekt.

**Einfaches Speicherbudget (Standard „Beim Start bestimmen“):**

- Drei ausdrücklich getrennte Modi: `startup` bestimmt beim Start einmalig einen konservativen endlichen Grenzwert; `manual` verwendet den Betreiberwert; `automatic` aktiviert optionale langsame Anpassung. Im Standard- und manuellen Modus kein periodischer Speicherdruck-Monitor und keine nachträgliche Senkung aufgrund kleiner Druckwerte. Bestehende konfigurierte Grenzen beim Upgrade als `manual` erhalten, statt sie umzudeuten.
- Defaults für neue Instanzen: Start-/Ersatzobergrenze und Mindestbudget 512 MiB, Reserve 512 MiB, optionales Messintervall 60 Sekunden und Wachstumsschritt/-spielraum jeweils 64 MiB. Das sind Budgetgrenzen, keine vorab belegten Speicherreservierungen. `startup` darf anhand der einmalig gemessenen Kapazität nur oberhalb des Standardbudgets dimensionieren und niemals unter 512 MiB fallen. Geringere Messwerte behalten mindestens dieses Budget mit sichtbarem Status; ein kleiner Messwert allein verhindert den Systemstart nicht. `manual` hält den Betreiberwert live und akzeptiert keine Werte unter 512 MiB. Bestehende kleinere konfigurierte Werte werden beim Upgrade auf 512 MiB angehoben und ihre Settingsrevision fortgeschrieben; größere Werte und Modi bleiben erhalten. Die ausdrücklich aktivierte Automatik behält ihre unabhängigen sicheren Druck-/Kapazitätsreaktionen.
- Beim Start verfügbare Kapazität einschließlich nachweisbarer Host-/Containergrenzen einmalig erfassen. Reserve für Betriebssystem, andere Anwendungen, Rust-Host und PostgreSQL abziehen. Den Startwert auf den konfigurierten endlichen Ersatz-/Startwert begrenzen: verfügbarer RAM ist keine Aufforderung, ihn vollständig Monty zuzuweisen. Ohne verlässliche Messung den dokumentierten endlichen Ersatzwert verwenden und die Messlücke sichtbar machen. Kein unbegründeter RAM-Prozentsatz, keine Vorabreservierung, keine Seitenbegehung oder erzwungene Seitenrückgewinnung.
- Nach einem Schritt nur entbehrliche temporäre Daten freigeben; typisierte Ergebnisse für spätere Recipe-Schritte behalten. Nach abgeschlossenem/abgebrochenem Task dessen transiente Ausführungskontexte und Child-Heaps nach bestätigter Übergabe freigeben. Lebende Fortsetzungen, persistente History und ungeklärte Wirkungsevidenz nicht löschen. Aufbewahrte Evidenz über einen dauerhaften Recovery-Eigentümer verwalten; VM-Speicher ist kein Checkpoint.
- Begrenzte Aufnahme/Parallelität und wirksame gemeinsame Heapgrenze bleiben unabhängig von Messungen bestehen. Volle Queues warten ohne Polling mit derselben Attempt-Identität; Control und aktive Ergebnisse bleiben verarbeitbar. Ein kleiner/einzelner erhöhter Druckwert darf nicht die gesamte Aufnahme pausieren. Im optionalen Automatikmodus nur bei kritischem Druck oder nachgewiesen unzureichender Kapazität pausieren; bei fehlender/veralteter Messung kein Wachstum, aber mit vorhandener endlicher wirksamer Grenze weiterarbeiten, solange deren belegter Heap noch sicheren konfigurierten Spielraum hat.
- Optionaler Automatikmodus: native Messungen frühestens im konfigurierten Intervall (Default 60 Sekunden), ohne engen Work-Polling-Loop/LLM. Vorhandenen Heap einmal zählen; Reserve separat. Schrittweise bedarfsgerechte Erhöhung, kontrollierte Senkung und veraltete Messungen behandeln. Erhöhter Druck allein verhindert optionales Wachstum, beendet aber keine laufenden Tasks und pausiert bei ausreichender Kapazität keine neue Arbeit. Kritischer Druck oder Kapazität unter lebendem Heap darf Aufnahme begrenzen; wirksame Grenze niemals unter belegten Heap setzen.
- Modus, Speicherwert, Reserve, endlicher Start-/Ersatzwert und Automatikintervall sind über bestehende WebUI/DB/API-Verträge verwaltbar. Gültige manuelle Änderungen werden live bestätigt; nicht erfüllbare Senkungen vor Persistenz ablehnen. Serialisierte Worker-Feasibility und DB-CAS/Commit-Ergebnis koordinieren: keine stale Inspect-Prüfung und keine Erhöhung mit neu zugelassenen Allokationen vor gesichertem Commit. Ein abgebrochener HTTP-Waiter darf eine angenommene Veröffentlichung nicht zurückziehen. Gewünschte Settingsrevision, bestätigte wirksame Heaprevision, Heapverbrauch, Start-/Ersatzgrund und optionale Messaktualität anzeigen; keine DB-Schreibvorgänge pro automatischer Messung.

Der Default für `max_duration_secs` beträgt instanzweit 600 Sekunden und gilt bei fehlender expliziter Betreiberkonfiguration in Config, DB, API und WebUI einheitlich. Bestehende explizite Werte beim Upgrade erhalten. Diese Grenze betrifft ausschließlich Rechenzeit pro Aufgabe; die Lebensdauer des globalen Monty bleibt bis zum Shutdown unbegrenzt.

Keine zusätzliche feste 30–3600-Sekunden-Spanne erzwingen. Der Betreiber setzt eine positive ganze Sekundenanzahl; die aktuelle PostgreSQL-`INT`-Darstellung begrenzt sie technisch auf 1–2147483647. DB, API, WebUI, Rust und Worker prüfen denselben Bereich. Der Upgrade ändert weder gespeicherte Werte noch ihre Revision. 0, negative Zahlen, Bruchteile und Werte außerhalb der Darstellung werden vor Mutation abgelehnt; fehlende/nullable Patchfelder bleiben nach dem bestehenden PATCH-Vertrag unverändert. Diese Darstellungsgrenze ist kein zusätzlicher Task-/Orchestrator-Timeout.

Zusätzliche Grenzen für taskgebundene Ausführungskontexte, Inbox, Fortsetzungen und stdout im Settings-Katalog getrennt ausweisen. Der globale Dienst muss seine begrenzte Kontrollverarbeitung auch nach einem Taskbudgetfehler fortsetzen können; dafür die Trennung von Taskverbrauch und Dienstaufwand technisch nachweisen.

Die Hosting- und Settingsfristen getrennt im bestehenden `execution_limits`-Vertrag führen: `startup_timeout_millis` und `response_timeout_millis` begrenzen Workerstart beziehungsweise IPC; `settings_source_timeout_millis` (Default 2000 ms) begrenzt einzelne Settings-Datenbank-, Lock- und Messoperationen, `settings_uptake_timeout_millis` (Default 5000 ms) die Veröffentlichung beziehungsweise den bestätigten Settings-Snapshot-Wait. Positive ganze Millisekunden und die tatsächliche native Uhrdarstellung validieren; keine künstliche Reihenfolge zwischen Quell- und Bestätigungsfrist verlangen. Ein angenommener Vorgang hält die Fristen seiner bestätigten Revision über Persistenz, Commit-Readback und Acknowledge fest. Spätere Änderungen dürfen ihn nicht rückwirkend verkürzen. Neue Fristen werden mit dem vollständigen bestätigten Einstellungssatz wirksam; ein abgelaufener Waiter zieht angenommenes Worker-/Persistenzgeschehen nicht zurück. Gleichbleibende Revision mit abweichendem vollständigem Snapshot ist ein Fehler und muss vor Heap-/Workerwirkungen abgewiesen werden. Migrationen ergänzen nur fehlende Felder, erhalten explizite Werte und erhöhen eine betroffene Revision einmalig. Native Abnahme muss reale DB-Lockwartezeit über der angeforderten Nachfolgefrist, unveränderte Heapwerte bei Revisionskorruption und Live-WebUI-Übernahme im selben globalen Monty nachweisen. Unabhängige Ownership-/Recoveryfristen und weitere technische Grenzen bleiben Teil des vollständigen Settings-Cutovers.

Alle unterstützten Ressourcensettings gelangen ohne Prozess-, Gateway- oder Monty-Neustart in die laufende Runtime. Der Dienst übernimmt den zusammenhängenden, validierten Einstellungssatz mit Revision am nächsten sicheren Prüfpunkt, auch bei aktiven/suspendierten Aufgaben. Rust-seitige Ressourcenprüfung und Monty-Tracker verwenden denselben wirksamen Einstellungssatz samt Revision; getrennte Defaults oder veraltete Kopien dürfen keine abweichenden Grenzen erzwingen. Eine Revision gilt erst als vollständig übernommen, wenn beide Prüfpfade sie verwenden; ausstehende oder fehlgeschlagene Übernahme bleibt sichtbar. Daueränderungen erhalten die bereits verbrauchte Task-Rechenzeit, auch über Schritte und Fortsetzungen hinweg. Maximale Übernahme-/Control-Reaktionszeit und VM-Zeitscheibe vor Produktionsverdrahtung festlegen und messen. Reine Python-Schleifen dürfen die Verwaltungsroute oder den Control-Kanal nicht unbegrenzt blockieren. Persistenz und Runtime-Acknowledge können scheitern: gespeicherte Zielrevision, wirksame Revision und fehlgeschlagene/ausstehende Übernahme anzeigen; Wiederanlauf übernimmt nur gültige Werte. Eine UI-Speicherbestätigung allein beweist keine wirksame Übernahme.

**Instanz-Ownership separat konfigurieren:** Im selben `execution_limits`-Satz `ownership_check_timeout_millis` (Default 2000 ms), `ownership_heartbeat_interval_millis` (Default 2000 ms) und `max_pending_ownership_checks` (Default 8) über DB/API/WebUI führen. Positive ganze Werte und native Uhrdarstellung prüfen; explizite Werte bei Upgrade erhalten. Angenommene Checks behalten ihre absolute Frist und ihren tatsächlichen Kapazitätsverbrauch trotz Settingsänderung oder verschwundenem HTTP-Waiter. Senkung unter laufenden Verbrauch lässt diese Arbeit auslaufen und zeigt Kapazitätsschuld; neue Aufnahme wartet beziehungsweise meldet Überlastung. Volle Aufnahme oder bereits vor Ausführung abgelaufene Anfragen sind keine verlorene DB-Ownership und dürfen den globalen Dienst nicht beenden. Heartbeats rechnen ab dem letzten erfolgreichen echten DB-Check; andere Settingsänderungen verschieben diesen Zeitpunkt nicht. Neue Ownership-Grenzen und bestätigte Revision erst nach Worker-/Rust-Acknowledge kohärent veröffentlichen, tatsächliche ausstehende Checks und wirksame Ownership-Revision anzeigen. Native Abnahme umfasst reale PostgreSQL-Sessionbeendigung mit Dienst-Fencing, vollständige Live-WebUI-Übernahme im selben VM sowie erhaltene Deadlines/Kapazitätsschuld. Das ersetzt keine dauerhafte Generationssperre und Wirkungsreconciliation für prozessübergreifende Fatal-Recovery; diese bleiben gesondert nachzuweisen.

**Abbruch-Bestätigung separat konfigurieren:** `cancellation_ack_timeout_millis` (Default 5000 ms) gehört in denselben vollständigen `execution_limits`-Satz. Positive ganze Werte und native Uhrdarstellung validieren, explizite Werte beim Upgrade erhalten. Der Driver sperrt den exakten Attempt und übermittelt den Abbruch vor Policy-Lookup oder anderem Warten. Der angenommene Bestätigungswait hält die zuletzt vollständig bestätigte Settingsrevision samt Frist; spätere Änderungen wirken auf neue Waits, niemals rückwirkend auf diesen. Der interne Policy-Leseport hält keinen Service-Owner und liest keine neuere bloß gewünschte DB-Konfiguration. Seine letzte bestätigte Policy bleibt auch für die Abwicklung eines fehlgeschlagenen Dienstes nutzbar und erteilt keine Ausführungsberechtigung. Fristablauf meldet fehlende Bestätigung und behält Attempt, Originalreceipt und ungeklärte Wirkungsevidenz; weder erfolgreicher Abbruch noch Freigabe oder Replay daraus ableiten. Native Abnahme senkt die Frist per WebUI bei realem Provider-Wait im selben globalen VM, prüft unveränderte Frist des älteren Stop-Waits, erhaltene durable Admission/Kapazität und die spätere idempotente Abwicklung desselben Receipts ohne verspätete Antwortpublikation.

**Leerlauf-Abgleich einstellbar halten:** `settings_reconcile_interval_millis` (Default 1000 ms) ersetzt das fest verdrahtete Intervall des Settings-Controllers im selben vollständig bestätigten `execution_limits`-Satz. Positive ganze Werte und native Uhrdarstellung vor Persistenz/Publikation prüfen; explizite Werte beim Upgrade erhalten. Jeder neue Timer verwendet die letzte vollständig bestätigte Revision. Ein WebUI-Save besitzt einen unabhängigen, verlustfreien Wake und wartet auch bei langem Leerlauf-Intervall nicht darauf. Laufende Source-/ACK-/Abbruch-Waits behalten ihre zuvor angenommenen Fristen. Im Startup-/Manual-Modus bewirkt ein Timer keinen OS-Speicher-Read. Optionaler Automatic-Modus berücksichtigt zusätzlich den tatsächlichen letzten bestätigten Messzeitpunkt und die separate Messperiode; ein langer Settings-Abgleich darf die nächste Automatic-Messung nicht aufschieben. Nach Fehlern begrenzt positiver Backoff die Wiederholung; kein sofortiger Busy-Loop wegen eines bereits fälligen Messzeitpunkts. Sehr ferne logische Deadlines bleiben erhalten, auch wenn der native Timer intern in darstellbaren Abschnitten wartet; Zwischen-Wakes lösen weder Abgleich noch Messungen aus. Native Abnahme prüft periodische Übernahme einer echten DB-Revision ohne WebUI-Wake, unmittelbare WebUI-Übernahme bei langem Intervall im selben globalen VM und die reale Automatic-Messperiode trotz längerem Settings-Intervall. Transport-/Allocator-/Root-/Flow-Grenzen bleiben separate Implementierungsarbeit.

**Verbindliche Tokenbudget-Regel:** `token_budgets_enabled` ist standardmäßig `false`. Bei ausgeschalteten Tokenbudgets keine künstliche Begrenzung von Retrieval, Vorwissen, History oder Taskverbrauch. Tokenzählung zur Beobachtung bleibt möglich. Technische Kontext- und Ausgabelimits des verwendeten Modells bleiben bestehen. Alle Verbraucher beachten denselben globalen Schalter; feste Konstanten wie `RETRIEVAL_TOKEN_BUDGET` oder `PRIOR_KNOWLEDGE_TOKEN_BUDGET` dürfen ihn nicht umgehen. Ressourcenlimits für Rechenzeit, Allokationen und Speicher bleiben eigenständige Regeln.

**Gemeinsame Dauerrevision für Rust und Monty:** Änderungen von `max_duration_secs` im WebUI werden als ein validierter, revisionsgeprüfter Instanzsatz veröffentlicht. Rust-Supervision und Monty-Tracker lesen dieselbe wirksame Revision und denselben taskbezogenen Verbrauch; keine unabhängigen Startzeitkopien, per-Chat-Werte oder nacheinander aktualisierten Grenzwerte. Aktive Aufgaben übernehmen die Änderung am begrenzten Prüfpunkt ohne Verbrauchsreset. Eine Senkung unter den bereits verbrauchten Wert beendet die betroffene Aufgabe nach dem Taskfehlervertrag. Externe Aufrufdeadlines bleiben separat; eine Rust-Wall-Clock-Timeout-Kopie des Monty-Rechenzeitlimits darf Idle-/Auth-/Tool-Waits nicht als Task-Rechenzeit verbuchen. Die API bestätigt die gemeinsame Übernahme erst nach Runtime-Acknowledge, andernfalls zeigt sie ausstehende/fehlgeschlagene Übernahme. Caller-Level-Tests ändern den Wert während aktiver Ausführung und während eines externen Waits und prüfen die Revision und Wirkung auf beiden Seiten.

**Nachgewiesene API-Lücke und Upgrade-Auswirkung:** Im bisherigen Monty v0.0.16 zählt `LimitedTracker` Allokationen über seine Lebensdauer; `set_max_duration` setzt die Startzeit zurück, öffentliche Setter für Speicher/Allokationslimit fehlen. In 1.0.0 gelten stattdessen der konkrete ResourceTracker, Feed-/Turn-Zeit und Allocator-Speicherprüfung gemäß Upgrade-Voraussetzung; auch dort setzen die Zeitsetter Akkumulatoren zurück. Vor dem Cutover einen auf der gewählten 1.0.0-API nachgewiesenen Engine-/Hosting-Vertrag mit erhaltener Verbrauchsrechnung, Taskzuordnung, Live-Grenzen und Zugriff an allen verwendeten Fortschrittszuständen implementieren und testen. Keine erfundenen Reset-/Snapshot-APIs, unbegrenzten Limits oder neue globale VM pro Task als Ersatz verwenden.

**Verbindliche Reihenfolge:** Betroffene Verbraucher, Kontext-/Policy-Verträge und Migrationsentwurf müssen feststehen. Monty-Upgrade samt Ressourcen-Disposition sowie Fortsetzungs-, Fairness- und Live-Limit-Machbarkeit zuerst nachweisen; dann den Produktionsdienst verdrahten. Temporäre alte SQL-/Scope-Typen hinter bestehenden Adaptern halten; produktive Daten erst über Phase 6/8 migrieren. Jeden Schritt mit Caller-Level-Tests abschließen. VM-Lebenszyklus ist Infrastruktur; Aufgaben bleiben Recipe-/PythonCode-gesteuert.

1. **Lebenszyklus und Ownership als neutralen Vertrag festlegen.** In `brassclaw_turns/src/run_profile/driver.rs` den vorhandenen Port zu einer Vorgangsübergabe an den globalen Dienst weiterentwickeln, ohne DB-, Engine- oder HostRuntime-Implementierungen zu importieren. Komposition besitzt einen privaten Service-Handle mit Inbox, Status und Shutdown/Join; genau eine Tokio-Service-Task besitzt und treibt die globale `MontySession`. Keine zweite Instanz in WebUI, REPL, MCP oder Triggern; kein `SessionRegistry<TurnScope, MontySession>` mehr als Orchestrator-Owner. Lebenszyklus mindestens `Starting → Ready → Stopping → Stopped`, zusätzlich `Failed`; Run-Ergebnisse sind separate Zustände. Wenn mehrere Produktadapter in derselben Runtime hängen, teilen sie denselben Handle. Einen zweiten Prozess auf derselben Instanzablage vor Dienststart durch eine Instanz-Lock/Lease mit sauberem Crash-Release verhindern; mehrere unabhängige Instanzablagen bleiben zulässig. Erst vorhandene Instanz-/Embedded-PG-Locks prüfen, keine zweite Lock-Konvention erfinden.

   Instanzidentität auch bei externer PostgreSQL-DB eindeutig festlegen. Mehrere Prozesse mit verschiedenen lokalen Pfaden dürfen denselben logischen Instanzbestand nicht parallel besitzen. Den passenden DB-/Instanzlock wiederverwenden; ein Embedded-PG-Datenverzeichnislock allein beweist keinen exklusiven Orchestratorbesitz.

2. **Boot-Prerequisites aus der WebUI herausziehen.** In `webui.rs` die bestehenden `seed_*`, Queue-Recovery, Content-Integritätschecks und `init_*`-Ladungen zu genau einer gemeinsam genutzten Kompositionsinitialisierung extrahieren. Ihre Abhängigkeiten und Reihenfolge erhalten; `BootedDb` bleibt der Migrationsnachweis. `runtime.rs`, CLI `serve` und `repl` auf explizites Bauen/Initialisieren/Starten umstellen. Ziel: DB/Migrations → Seeder → Integritätsprüfung → Host-/Component-/Kohai-Ports → verifizierter Orchestrator-Code → globale VM → initialer Work-Wait-Handshake → Worker/Produzenten → Ingress/Readiness. Für erforderliche Komponenten Fehler weitergeben statt nur warnen. `build_webui_services` darf keine nachträgliche Voraussetzung für Monty mehr herstellen. Worker, Trigger-Poller und Kanaladapter dürfen vor dem Handshake weder neue Arbeit claimen noch Side Effects ausführen. Jeder Zwischenfehler räumt bereits erworbene Locks/Tasks/DB-Ressourcen auf.

   **Prefix als Teilnehmer derselben Initialisierung:** Der Prefix-Owner liefert
   die exakten Reference-/Compiler-/Profil-/Design-Seedpakete, Abhängigkeiten und
   Integritätsprüfungen nach [Prefix §6](docs/plans/prefix_v3_upgrade.md#6-installation-time-db-seeding-preferred-combined-approach).
   Diese Phase integriert sie in die vorhandene gemeinsame Initialisierung;
   keine zweite Bootstrap-Registry, WebUI-Initialisierung oder Startwarteschlange.
   Pflichtbestandteile des globalen Root-/Hostvertrags müssen vor Ready geprüft
   sein. Eine noch nicht erzeugte Prefixgeneration, ein fehlender optionaler
   Domainbestand oder ein unkonfigurierter Provider ist dagegen ein sichtbarer
   Produktzustand und darf einen sonst sicheren Runtime-Boot nicht verhindern.
   Runtime-Ready, Build-Bereitschaft und Modell-/Serving-Bereitschaft getrennt
   ausweisen. Ein initialer Build wird erst nach dem Handshake als gewöhnlicher
   qualifizierter Auftrag aufgenommen; Boot wartet weder auf seine Fertigstellung
   noch auf Netzwerk/GPU/Modellaufrufe. Fehlende lokale Compiler-/Tokenizerartefakte
   blockieren den betroffenen Build, nicht automatisch die Betreiberverwaltung.

3. **Thread-freien VM-Boot ermöglichen.** `prepare_monty_session`, `MontySession::new` und `build_orchestrator_inputs` in `engine/src/executor/orchestrator.rs` sind heute an `Thread` und dessen Kontext gebunden. Einen neutralen Boot-Input mit Instanz-/Protokollversion und leerem Taskzustand einführen; keinen Dummy-Chat, Dummy-Run, Betreiber-Grant oder fingierten Userprompt anlegen. `basic_mode.py` muss vor der ersten Arbeit ausschließlich zulässige Boot-/Wait-Operationen ausführen; auch das bisher vor `await_next_turn()` stehende `check_signals()` darf keinen fiktiven Run verlangen. Taskgebundene Tools ohne aktiven Kontext verweigern. Die einzige globale VM beim Boot kompilieren/starten und bis zu ihrem ersten Eingabe-Wait treiben. Fehlender/ungültiger Code, Integritätsfehler, Boot-Timeout, frühes `FINAL` oder fehlender Handshake verhindern Ready; die erste Chatnachricht darf diese Fehler nicht erst entdecken.

4. **Work-/Control-Ereignisse und begrenzte Inbox definieren.** Eingaben tragen Protokollversion, ihre echten fachlichen Task-/Thread-/Turn-/Run-Referenzen, Worker-Claim-/Attempt-Identität und Deadline. Chat-Eingaben behalten die genaue AcceptedMessageRef und das Reply-Binding. Hostseitig aufgenommene interne Produktaufträge nutzen ihre tatsächliche dauerhaft gebundene Eingabereferenz und Abschlussart gemäß dem gemeinsamen Admission-/Recoveryvertrag unten; keine fingierte Benutzer-Nachricht oder Reply-Pflicht einführen. Payloadauflösung erfolgt im Host-Adapter. Control unterscheidet Run-Abbruch, externe Auth-Fortsetzung, Child-/Tool-Ergebnis, Settingsrevision und Shutdown. Python erhält validierte Monty-Werte; unbekannte Versionen/Typen scheitern explizit. Leere Nachricht, geschlossener Sender und Run-Stop bedeuten kein globales `FINAL`. Begrenzte Workqueue, Backpressure und garantierte Control-Kapazität/Priorität unter Last festlegen. Die persistente Turn-Ablage bleibt Arbeitsquelle; die Inbox ist Transport und kein zweiter dauerhafter Scheduler.

   **Recipe-Zustand gehört Monty.** IBS kompiliert das Recipe in ein `BuildInstruction`; Monty führt die Schritte aus und hält benötigte Zwischenwerte im jeweiligen Recipe-Ausführungskontext. Kein verpflichtendes leeres `{}` pro PythonCode-Schritt und kein Zusammenlegen von Schritten als Ersatz für fehlende Zustandsweitergabe. Explizite Child-Tasks erhalten typisierte Eingaben und Ergebnisse über denselben globalen Dienst; sie starten keine weitere Monty-VM. Ein unterstützter nativer Tool-Subprozess bleibt eine separate Implementierungsgrenze, kein zweiter Orchestrator. Laufzeitdaten als validierte Werte transportieren, nicht in Python-Quelltext interpolieren. Unabhängige Vorgänge und Attempts bleiben getrennt; Fortsetzung nach Waits erhält den zugehörigen Kontext, Retry-/Checkpoint-Restore ist ausdrücklich validiert und beendet/abgebrochenes Task-State wird freigegeben. Durch den Produktions-Caller eine Kette A → B mit B als Verbraucher von A nachweisen, einschließlich Child-Ausführung, externer Wartephase, interleavtem fremden Task und veraltetem Attempt. Die bisherige Fresh-Step-Ausführung ist eine zu behebende Implementierungslücke.

5. **Exakte Nachricht statt „latest“ auflösen.** `PersistentMontyDriver::drive_turn` verwendete vor Implementierungsbeginn `latest_thread_message(...Submitted)`. Der exakte Lookup der `LoopRunContext.accepted_message_ref` samt Inhalt-/Referenz-/Turn-/Run-/Thread-Prüfung ist inzwischen über `submitted_user_message` umgesetzt und gezielt getestet; die folgenden History-/Idempotenz-Anforderungen bleiben Teil dieses Schritts. Vorhandene Message-/Host-Lookups wiederverwenden oder einen schmalen neutralen Port ergänzen; keine SQL-Abfrage im Runner. Fehlende Referenz oder DB-Fehler ist ein expliziter Run-Fehler, kein leerer Text. History bis zur konkreten Eingabe ordnen und diese exakt einmal aufnehmen; später akzeptierte Nachrichten dürfen weder den Input ersetzen noch vorzeitig im LLM-Prompt erscheinen. Deduplizierung über vorhandene Message-/Turn-Idempotenz erhalten. Retry desselben Inputs erzeugt keinen zweiten Task oder Reply.

6. **Globale VM und Vorgangsdaten trennen.** History/Status pro Task aus bestehenden Stores laden und am konkreten Input begrenzen. Python-Variablen, Ergebnisse, Credentials und taskgebundene Bindings dürfen nicht zwischen Vorgängen übergehen. Technische Ausführungskontexte bleiben taskbezogen; Toolrechte stammen stets aus der aktuellen globalen Konfiguration. Daten nach Abschluss freigeben; suspendierte Referenzen begrenzen. `host.run_program` erhält taskgebundene Eingaben und Ergebnisse; Monty hält benötigten Recipe-Zustand auch über Child-Ausführung und Waits. Frisches leeres State pro Schritt ist eine bestehende Implementierungslücke, keine Zielvorgabe.

   Recipe, Variante, `step_link`, ToolSkills, PythonCode und LLM-Kontextkomponenten mit ihren tatsächlich verwendeten Revisionen/Checksums am Vorgang festhalten. Aktive und suspendierte Vorgänge verwenden diese Revisionen weiter; neue validierte Revisionen gelten für folgende Vorgänge. Alte benötigte Revisionen nicht vorzeitig löschen. IBS bleibt ein Compiler mit ephemeral `BuildInstruction`; keine neue persistente Instruction-Tabelle einführen. Retainierte Programmausführung und Revisionsreferenzen nach dem bestehenden Fortsetzungsvertrag halten. Monty-Root-Code bleibt für die Dienstgeneration fix und wechselt nur kontrolliert nach Reconciliation; Ressourcen- und Toolsettings wechseln live ohne Code-Neustart.

   Phase 0a vollständig anwenden: auch Tooldefinitionen und tatsächliche
   Implementierungshandles/Artefakte samt kompatiblem Adapter/ABI, Skill-
   Assoziationen, exakte Approval-IDs, Inputlayout und alle transitiven
   Abhängigkeiten aus derselben Generation pinnen und erhalten. Bei fehlender
   Kombinationsapproval oder inkompatiblen neuesten aktiven Verträgen vor
   Effekten scheitern; keine ältere Version oder latest-Handler einsetzen.
   Tatsächliche ToolSkill-Vorbereitung und typisierte inputs-/Ergebnishandoffs
   durch verschachteltes host.run_program und Wait/Resume verifizieren.

7. **Runner-Leases und Ownership der Ausführung erhalten.** `brassclaw_reborn/src/turn_runner.rs` bleibt für Claim/Heartbeat, Fristen und Exit-Anwendung zuständig und übergibt Vorgänge über den neutralen Port. Keine Veränderung der Recipe-Schrittreihenfolge in Rust. Aktuell `invoke_driver` unter `tokio::select!`/Timeout zu droppen darf künftig nicht die vom Dienst besessene globale VM zerstören oder unbeaufsichtigte Side Effects weiterlaufen lassen. Abbruch muss als adressiertes Ereignis bestätigt werden; vor jedem Dispatch/Reply prüfen, dass Run/Attempt/Lease noch aktuell sind. Warteschlangenzeit, Claim-Laufzeit, ausführende Zeit und Gate-Wartezeit explizit unterscheiden. Verlorener Claim verhindert neue Effekte/Replies durch den alten Attempt. Bestehende Trusted-Trigger-Minting- und Kernel-Grenzen erhalten; ein globaler Kanal ist keine Trusted-Ingress-Abkürzung.

   Der aktuelle Worker wartet innerhalb `execute_claimed_run` auf das ganze Driver-Ergebnis. Bei einem Task-Yield daher über bestehende Suspend-/Gate-/Checkpoint-Transitions den Wartezustand samt Fortsetzungsreferenz persistieren und den Worker für andere Claims freigeben; Resume erst nach neuem gültigem Claim. Eine blockierende `oneshot` pro vollständig wartendem Parent löst das Problem nicht. Queue-Admission darf einen schon bestehenden durable Run nicht verlieren; Fehler zwischen Submit, Mark-Submitted, Enqueue und Acknowledge müssen über die vorhandenen Idempotenz-/Replay-Regeln wiederaufnehmbar bleiben. Für Replies/Events die tatsächlich verfügbaren Store-Transaktionen und Unique-Keys prüfen, nicht eine nicht vorhandene atomare Cross-Store-Operation voraussetzen.

8. **Warten und faire Fortsetzung lösen.** Auth-, Subagent-, MCP-, Provider- und Cancel-Pfade tatsächlich verfolgen; keine nur in alten Plänen genannten Wrapper voraussetzen. Kein Parent hält globale Ausführung während eines Child-Waits. Python entscheidet fachliche Suspend-/Resume-/Child-Sequenzen; Rust transportiert Ereignisse und hält Ausführungskontexte. Keine rekursiven Self-Waits oder Sperren über externe Awaits. Eine Task-Zeitscheibe und überprüfbare Control-Reaktionszeit verhindern, dass lange reine Python-Ausführung andere Arbeit oder Settingsänderungen aushungert. Mechanisches VM-Yield/Transport darf kein Rust-Recipe-Loop werden. Globale VM-Ausführung bleibt exklusiv; wartende Arbeit gibt den Worker frei. Externe Auth-Gates bleiben, operationsbezogene Tool-Approval-Gates entfallen.

   **Technischer Vorabnachweis:** `MontySession` hat heute nur ein `parked_call` und setzt beim Resume genau diesen Python-Aufruf fort. Ein anderer `Thread` als Argument ersetzt weder den Python-Stack noch dessen Variablen. Deshalb zuerst mit der gepinnten Monty-Version einen echten Ablauf A wartet → B läuft → A wird fortgesetzt demonstrieren. Erforderlich ist eine Rückkehr des Task-Waits an die globale Python-Ereignisschleife mit expliziter Continuation-ID; eine nur in Rust geparkte Root-VM kann währenddessen keinen anderen Task sequenzieren. Fortsetzungen enthalten mindestens Task-/Attempt-/Codegeneration, Recipe-/Schrittposition, erwarteten Ereignistyp und Referenzen auf bereits festgehaltene Ergebnisse; keinen ungetesteten Interpreter-Snapshot annehmen. Suspension mitten in `host.run_program`/einer abhängigen Toolkette muss den exakten Aufruf fortsetzen, ohne bereits ausgeführte Tools zu wiederholen. Wenn dafür ein kurzlebiger, vom globalen Orchestrator gesteuerter Programmausführungskontext erhalten werden muss, ist er an den Vorgang gebunden und besitzt keine eigene dauerhafte Chat-/Orchestrator-Schleife. Lange Provider-/Tool-Waits müssen entsprechende adressierte Completion-/Cancel-Ereignisse liefern; späte Ergebnisse mit alter Attempt-/Dienstgeneration verwerfen, ohne ihre Auditspur zu löschen. Erst nach diesem Nachweis die Produktionsverdrahtung umstellen.

   Der vorhandene Monty-A→B→A-Test ist eine API-Voraussetzung. Zusätzlich denselben Ablauf durch produktive Aufnahme, konkrete Recipes, verschachteltes `host.run_program`, Worker-Freigabe und Antwortpersistenz prüfen. Taskfehler, lange Python-Schleife, Live-Settings und Shutdown unter Last gehören zum Vorabnachweis.

9. **Signale, Replies und Fehler voneinander isolieren.** Signaladressierung mindestens durch Run-ID + Attempt ersetzen/ergänzen; ein verspätetes `Stop` für Run A darf nicht den nächsten Run desselben Chats oder die globale VM beenden. Reply-Persistenz aus `persist_new_assistant_messages` weiterverwenden, aber am konkreten Task/Message/Reply-Binding festmachen und gegen Doppelschreiben absichern. Chatvorgänge erst nach bestätigter Reply-Persistenz abschließen; interne ResultOnly-Aufträge nach dauerhaft bestätigter Ergebnis-/Effect-Settlement-Persistenz ohne synthetische Assistant-Antwort. Die bestehende Turn-/Event-Transition jeweils genau einmal gemäß ihrer echten Abschlussart anwenden; ein Produkt-Statuslabel ist kein Abschlussnachweis. Fehler eines Tasks lässt den Dienst bereit, sofern die VM nachweislich konsistent ist. Ist sie beschädigt, `Failed` melden, Aufnahme/Dispatch sperren und Recovery beaufsichtigen. Den alten `SessionGuard::Drop` mit unüberwachtem asynchronem Re-Park nicht auf eine globale Singleton-VM übertragen.

10. **Budgets für Dauerbetrieb nachweisen.** Den oben definierten 1.0.0-Engine-/Hosting-Vertrag für Taskzeit, gegebenenfalls separat nachgewiesene Taskallokationen und konfigurierbaren Monty-Speicher samt Allocator-Backstop umsetzen. Taskverbrauch über Schritte/Fortsetzungen erhalten; nach Abschluss neue Aufgaben mit eigenem Budget starten. Tokenverbrauch je Vorgang getrennt erfassen. Startdimensionierung ohne laufende Messungen, kleine manuelle Limits, optionale automatische Erhöhung/Senkung, RAM-Reserve, milden Druck ohne Aufnahmestopp, kritischen Druck, Messausfälle, Idle-/Auth-/Tool-Waits und globalen Speicherfehler testen. Gemeinsame Heaprechnung über Root und verschachtelte Ausführung ohne Doppelzählung nachweisen. Inbox, Fortsetzungen, stdout und History-Caches begrenzen; vollständige relevante Ausgaben persistent erhalten. Diagnosemetadaten enthalten keine Roh-Prompts/Secrets. Keine unbegrenzten Limits oder Taskabschluss durch Neustart der globalen VM kaschieren.

11. **Shutdown, Crash-Recovery und Codewechsel koordinieren.** `RebornRuntime::shutdown` erweitern: Admission/Produzenten schließen → bestehende Vorgänge bounded drainen oder adressiert abbrechen/suspendieren → Ergebnisse/Status persistieren → globalen Dienst stoppen und seine Task joinen → verbleibende Store-/Provider-Handles freigeben → Embedded-PG zuletzt stoppen. Fehlerpfade derselben Reihenfolge folgen. Für Neustarts einen persistierten Run-/Attempt-/Effect-Reconciliation-Vertrag verwenden; die in RAM gehaltene VM ist kein Snapshot. Keine automatischen Neuversuche eines gesamten Runs nach VM-Crash, Reply-DB-Fehler oder Timeout, wenn externe Effekte bereits erfolgt sein können. Bestätigte Effekte nicht wiederholen; unbekannter Effektstatus wird als ungeklärt sichtbar und benötigt sichere, operationsspezifische Klärung. Kein „exactly once“ für externe Systeme behaupten, die weder Idempotency-Key noch Statusabfrage unterstützen. Neue VM nur durch den Supervisor und erst nach Statusklärung aktivieren; fataler Fehler darf keine parallele Ersatzinstanz erzeugen. Keine stille Tier-2- oder Per-Chat-Fallback-Ausführung.

   Reconciliation als konkrete Betreiberfunktion vorsehen: betroffener Vorgang, bestätigte/ungeklärte Effekte, vorhandene Idempotency-Keys/Statusabfragen und zulässige Fortsetzung anzeigen. Das ist Fehlerbehandlung, keine zusätzliche Toolfreigabe. Authentifizierte Diagnose/Repair muss bei fehlgeschlagenem Orchestratorboot möglich bleiben, ohne Turn-Ingress freizugeben; bestehende CLI-Verwaltung wiederverwenden. Fehler eines begonnenen Recipe gemäß §1.1 behandeln.

   Den durablen Dispatch-/Retryvertrag aus Phase 0a vor dem realen Hostcall
   anwenden: Invocation, Dispatchabsicht und Versuchszähler persistieren,
   Completion/Effektstatus und Ergebnis getrennt nachvollziehbar festhalten.
   Crash zwischen Aufnahme, Dispatch und Completion führt ohne sichere Evidenz
   zu ungeklärtem Status. Restore übernimmt Zähler, Argumente/Key, ursprüngliche
   Versionen und bestätigte Effekte; ein neuer Worker-/Dienstattempt setzt nichts
   zurück. Zulässige Wiederholung folgt exakt skills.md, einschließlich aktueller
   Policy/Freshness und outcome-spezifischer Sicherheitsnachweise. Fehler beim
   notwendigen persistierten Dispatchrecord verhindern neue Effekte.

12. **Zusammenhängend migrieren und abnehmen.** `PersistentMontyDriver` zum Übergabeadapter machen oder durch einen klar benannten Adapter ersetzen; Registry ausschließlich als fachliche Daten-/Continuation-Ablage behalten, falls erforderlich, niemals für globale Chat-VMs. `session_registry.rs`, Engine-/Crate-CLAUDE, `docs/agents-v3/{01-architecture-overview,12-agent-loop,13-orchestrator-default-py}.md`, Architektur-Prefix, Boot-Seeds und C.6-Planverweise auf das Ziel ausrichten. Geänderte Systemkomponenten samt Checksums mit Seeder/Repair migrieren; Python-Codeänderung im Dateisystem allein erreicht produktive DB-Komponenten nicht. Vorher alte Inflight-Runs drainen/reconciliieren; neue Ereignis-/Codegenerationen dürfen keine alten RAM-Fortsetzungen übernehmen. „VM gestartet“ im Readiness-Vertrag durch einen echten Handshake-Test belegen, nicht durch `Option::is_some()` oder Seed-Zählung.

Die [gemeinsamen Runtime-Verträge für die Komponentenmigration](#gemeinsame-runtime-verträge-für-die-komponentenmigration)
konkretisieren diese Phase außerdem für Parent-/Child-Ausführung, gewöhnliche
MCP-Chats und Listener-/Kohai-Lifecycle. Die folgenden Produkt-/Reviewverträge
verwenden dieselbe Admission, Custody und Recovery; keine parallelen Owner.

#### Gemeinsamer Admission- und Recoveryvertrag für Produktaufträge

Dieser Vertrag konkretisiert Phase 3a Punkte 4/7/9/11 und Phase 0a Punkte 9–10
für interne Prefix-, Sources-, Mess-, Design- und Validatoraufträge. Vorhandene dauerhafte
Turn-Admission, Transaktions-Teilnehmer, reservierte interne Bindings,
`LoopCompletionKind::ResultOnly` und native Settlement-/Reconciliation-Owner
wiederverwenden. Deren begrenzte Unterstützung ist kein Auftrag zum Neubau und
keine allgemeine Freigabe aller Produkt-Recipes. Siehe die tatsächlichen
[Prefix-Aufnahmepfade](crates/brassclaw_reborn_composition/src/prefix_job_submission.rs)
und [Abschlussarten](crates/brassclaw_turns/src/loop_exit.rs).

**Geprüfte native Voraussetzung — 30. Oktober 2026:** Die optionale positive
Output-Einstellung aus ursprünglichen typisierten Recipe-Argumenten erreicht
über vorhandene PG-Custody/Gateway den echten SDK-/Final-Guard-Vertrag, auch bei
deaktivierten diskretionären Tokenbudgets. Ein aktiviertes engeres Limit lehnt
ab, statt zu clampen. [Begrenzter Nachweis](docs/validation/2026-10-30-original-model-output.md).
Diese Infrastruktur wiederverwenden; Tier1-ResultOnly-Aufnahme, erforderliche
Child-/Review-Custody und tatsächliche Free-Qualifikation bleiben die folgenden
offenen Verträge. Kein zweiter Modellexecutor oder abgeleiteter Freigabenachweis.


**Begrenzte Runtime-Abnahme — 30. Oktober 2026:**
[BUILD-004](docs/validation/2026-10-30-internal-model-admission.md) trennt die aus
dem ursprünglichen Claim abgeleitete ResultOnly-Abschlussart vom unverändert
wirksamen deterministischen Modellverbot. V191, globale Aufnahme, echte native
Modell-Custody/SDK-Übergabe, persistierter Abschluss und vorhandene Consumer haben
zwölf unterschiedliche Fälle/0 ignoriert sowie strenge verbundene/Default-Prüfung.
Den vorhandenen optionalen realen HostFactory-Parameter der Bootstrap-Aufnahme
wiederverwenden. [BUILD-008](docs/validation/2026-10-31-shared-model-capture.md)
ergänzt denselben gemeinsamen Capture-/Metadaten-/Native-Binding-Handoff für den
vorhandenen enthaltenen Vorstart-/Entwicklungsbeobachter. Neun native/Consumer-
Fälle und strenge verbundene/Default-Prüfung bestehen, einschließlich originaler
unbegonnener Claim-/Herkunftsprüfung und Capture vor Binding. Der tatsächliche
sealed I2-Source bleibt zu integrieren; daraus folgt kein zweiter Live-VM-Pfad.
Die separate native Custody des vorhandenen Sempai-Reviews vor dem Hauptaufruf
hat mit [BUILD-011](docs/validation/2026-10-31-related-model-custody.md) zehn
unterschiedliche Fälle/0 ignoriert und verbundene/Default-/neutrale strenge Abnahme:
eigene Work-ID/Anfrage/Route/Prefix und unveränderte Wire-/Provider-/Loop-Originale,
eigene Übermittlungssperre und spätere Speicherung nach Stornierung ohne Replay.
Volle Anfragebytes bleiben in PG, im RAM nur deren geprüfter Digest. Dies ist
kein Post-primary-Review- oder Free-Nachweis.
[BUILD-012](docs/validation/2026-10-31-review-unsubmitted.md) ergänzt die vorhandene
Accounting-Anbindung nach nativ bestätigter Nichtübermittlung: Der tatsächliche
Governor/PG-/SDK-Verbraucher unterscheidet denselben Budgetfehler vor dem Senden
von einer tatsächlich empfangenen Length-Antwort. Fünf Fälle/0 ignoriert und
strenge verbundene/Default-Prüfung bestehen; nur belegte Nichtübermittlung gibt
die Review-Reservierung frei. Unklare Effekte behalten ihre Reservierung. Dies
ist keine dauerhafte Crash-Recovery oder Provider-Abrechnungsqualifikation.
[BUILD-009](docs/validation/2026-10-31-model-host-compatibility.md) migriert die
historischen Modell-Host-Tests auf dieselben tatsächlichen Composition-Owner:
Prepare/Bind mit vollständiger konsistenter Auswahl, Reply/History vor Binding
und ein gemeinsamer unveränderter Metadatenparser für native und isolierte
Admission-Consumer. 24 frische Fälle/0 ignoriert, verbundene sowie isolierte
strenge Prüfung und CI-/YAML-/Formatprüfungen bestehen. Die native Capture-Seal
bleibt von beobachteten Daten getrennt; allgemeine Pure-/I1-/I2-Freigabe folgt
daraus nicht.
Konkrete Produkt-/Review-Qualifikation,
dauerhafte Wiederaufnahme und die übrigen
folgenden Anforderungen bleiben offen. Keine neue Queue, DTO-Berechtigung,
Modellschleife oder abgeschlossene Free-/Serving-Freigabe daraus ableiten.

1. **Authentische Aufnahme und Abschlussart.** Nur vertrauenswürdige Host-Admission
   bindet interne Herkunft, konkrete Recipe/Variante, typisierte Eingabe,
   Katalog-/Artefaktauswahl und Abschlussart an den Run. Öffentliches Chat-/MCP-
   oder Modelldatum kann diese Bindung nicht auswählen oder imitieren. Interne
   Aufträge matchen keinen fingierten Userprompt und wechseln bei Fehlern nicht
   nach Tier 2. ResultOnly ist ein persistierter Ergebnisabschluss, keine
   Assistant-Nachricht; gewöhnliche Chat-/MCP-Workflows behalten ihre eigene
   Reply-/History-Verantwortung. Tier-1-Aufträge brauchen den tatsächlich
   unterstützten Modell-/Tier-Vertrag, keine Umdeklaration als Tier 0.
2. **Auftragsdefinition und Run eindeutig verbinden.** Bestehende idempotente
   Aufnahme-/Transaktionsgrenzen so erweitern, dass bestätigte Produktdefinition,
   Requestidentität, genaue Input-/Workflowauswahl und ursprünglicher Run vor
   Effekten dauerhaft zusammengehören. Wo derselbe PostgreSQL-Owner beteiligt
   ist, dessen vorhandenen Transaktions-Teilnehmer nutzen. Bei getrennten
   Persistenzgrenzen einen ausdrücklichen idempotenten Prepare-/Reconciliation-
   Vertrag festlegen und vor Effekten die vollständige Bindung nachweisen.
   Dieselbe Request-ID mit demselben bestätigten Inhalt liefert denselben Auftrag;
   geänderter Inhalt oder eine andere Auswahl unter derselben ID ist ein Konflikt.
   Bei verlorener Commit-/HTTP-Bestätigung über die ursprüngliche Identität
   nachlesen/reconciliieren; keinen neuen Run, Snapshot oder Request erzeugen.
   Eine allein gespeicherte Produktdefinition ist noch keine erfolgte Aufnahme.
3. **Status als reparierbare Projektion.** Produktstufen, Fortschritt und Artefakt-
   verweise stammen aus dem zugeordneten Run und seinen echten Ergebnissen.
   Die vorhandene Event-/Statusprojektion muss nach Reconnect/Neustart fehlende
   oder doppelte Updates über ursprüngliche Run-/Invocation-/Ereignisidentitäten
   idempotent abgleichen können. Eine verloren gegangene UI-Verbindung oder ein
   Projektionsfehler startet keine Ausführung neu. Empfangene native Ergebnisse
   und ihre Custody bleiben bis zum bestätigten Settlement erhalten; ein Fehler
   der Produktanzeige oder einer späteren Antwort wiederholt keinen Effekt.
4. **Operationsspezifische Klärung anbinden.** Prefix liefert geprüfte Compiler-
   Artefakt-/Statusabfragen und Publikationsreceipts unter dem bestehenden
   Invocation-/Effect-Vertrag. Der gemeinsame Owner bewahrt Originalargumente,
   Keys, Counts, Auswahlsnapshots und confirmed/unresolved-Zustände und erzwingt
   Fencing. Recipe/Python entscheidet die fachlichen Klärungsschritte; Rust stellt
   Aufnahme, Custody, Statuszugriff und technische Sperren bereit. Kein universeller
   neuer Hook-Executor, Recovery-Worker oder zweites permanentes Journal.
   Ein Compilerexit 0, eine Stufenmeldung oder ein Run ohne geprüftes Endartefakt
   ist kein Prefix-Erfolg; domänenspezifische Validierung bleibt beim Prefix-Owner.
5. **Abbruch und Wiederanlauf exakt erhalten.** Browserabbruch ist kein Runabbruch.
   Expliziter Abbruch benutzt den ursprünglichen Run/Attempt und die gemeinsame
   Ack-/Fencing-Regel. Ein Ack-Timeout bedeutet ungeklärten Status, nicht bestätigten
   Abbruch oder freie Kapazität. Späte Ergebnisse werden dem Original zugeordnet,
   ohne stale Publikation oder erneuten Dispatch; benötigte Wirkungsevidenz bleibt
   erhalten. Bei Reclaim/Restart zuerst ursprüngliche Aufnahme, Effect-Records und
   Custody klären. Keine automatische Wiederholung des Gesamtauftrags und kein
   Rücksetzen von Counts. Die Ausführung bleibt bei `stop`/`max_attempts: 1`, solange der
   gemeinsame dauerhafte Retryadapter nicht implementiert und qualifiziert ist;
   Sicherheitsnachweise allein schalten ihn nicht frei.

Diese Anforderungen erweitern ausschließlich die vorhandenen Owner und Ports.
Konkrete Schemaergänzungen erst nach Prüfung ihrer tatsächlichen Records/Leser
festlegen; diese Liste erfindet keine API-Felder oder neue Queue. Prefix speichert
seine Fachdefinitionen, Artefakte und unveränderlichen Ausführungsreferenzen und
liefert die Integrationsfälle aus [Prefix §7](docs/plans/prefix_v3_upgrade.md#7-persistence-and-generation-lifecycle).

#### Gemeinsame Runtime-Anbindung der Komponentenprüfung

Diese Phase implementiert die gemeinsame Ausführungs-/Custody-Unterstützung für
[Validator Steps 3/6/9–12](validator_v3.md#concurrent-implementation-with-simplified-v3).
Der Validator-Owner behält Submissions, Review-Subjects, Check-/Coverage-Regeln,
semantische/behaviorale Bewertung, Q1/Q2-/Association-/Pure-Evidenz und kontrollierte
Seed-Provenienz. Bestehende Review-Stores, sealed Subjects/Observations, globale
Admission und authentifizierte Q2-Route wiederverwenden. Die fehlende allgemeine
Pure-Approval-Unterstützung bleibt beim Validator; keine neue Ausnahme im Runtime-Owner.

1. **Privates Review-Work eindeutig aufnehmen.** Der Validator liefert die
   unveränderliche Submission samt Candidate-/Base-/Dependency-Subject und
   Check-Set. Die Runtime bindet diese Herkunft getrennt vom ausgewählten bereits
   genehmigten Validator-Recipe/Variante/Layout/Engine-/Host-Artefaktgraph an den
   ursprünglichen Review-Run und seine Abschlussart. Beides vor Effekten dauerhaft
   festhalten; dasselbe Eingabeobjekt oder dieselbe UUID ersetzt die getrennten
   Identitäten nicht. Den vorhandenen Submission-Transaktionspfad mit der
   gemeinsamen Aufnahme verknüpfen; für ungewissen Commit den ursprünglichen
   Datensatz nachlesen statt ein zweites Review anzulegen. Ein bewusstes Resubmit
   erzeugt eine neue Subject-/Reviewidentität; Transportretry aktualisiert weder
   Candidate noch Validator auf latest. Review-Work bleibt in bestehenden Stores
   auffindbar; vorhandene Zustellung/Reconciliation wiederverwenden, keine zweite
   Turnqueue, Review-Poller-Schleife oder synthetic Chat-/MCP-Route.
2. **Ausführung und Evidenzfreigabe getrennt übergeben.** Die Runtime bewahrt pro
   Run/Attempt/Occurrence die tatsächlich ausgeführte Quelle/Interpreteridentität,
   genaue Inputs/Ergebnisse/Native-Aufrufe, ursprüngliche Beobachtungsgrenzen und
   deren confirmed/unresolved-Status. Die vorhandenen sealed Observations bleiben
   dem ursprünglichen Subject und Validatorgraph zugeordnet; ein fremder Run,
   stale Attempt, vom Kandidaten geliefertes Flag oder stdout begründet keine
   erfolgreiche Beobachtung. Spätere Limitänderungen schreiben vergangene
   Beobachtungen nicht um. Ressourcen-, Host-, Cancel- oder Custodyfehler bleiben
   maßgeblich, auch bei abgefangener Python-Exception und Erfolgspayload.
   Ein Runabschluss ist kein Q1-Erfolg: Der Validator-Writer prüft genaues Subject,
   verlangte Regeln/Occurrences und authentische vollständige Beobachtungen vor
   Evidenzcommit. Fehlende/interrupted/übersprungene Checks bleiben unvollständig;
   Kandidatenablehnung, Infrastrukturfehler und unbekannte Wirkung unterscheiden.
3. **Settlement und Review-Projektion wiederaufnehmbar koppeln.** Native Ergebnisse
   bis zum bestätigten gemeinsamen Settlement erhalten. Der Validator-Owner
   schreibt seine Evidenz über den vorhandenen vertrauenswürdigen Writer mit
   stabiler Review-/Evidenzidentität; die Runtime liefert Originalrun und Custody.
   Fehler/verlorene Bestätigung zwischen Ergebnis, Evidenzcommit und Queue/UI-
   Projektion sind über dieselben Records abgleichbar, ohne den Check/Tool-Effekt
   neu auszuführen oder einen Pass zu synthetisieren. Duplikate idempotent lesen,
   abweichende Inhalte als Konflikt melden. Späte Ergebnisse eines alten Subjects
   dürfen dessen Historie ergänzen, aber kein neueres Resubmit oder Q2 freigeben.
   Cancel-/Ack-/Recovery-Vertrag aus dem gemeinsamen Admissionvertrag anwenden;
   ungeklärte Wirkung sperrt Reviewfreigabe. Ein ausschließlich reviewbezogener
   Fehler stoppt keine unabhängigen Tasks; echte globale Runtimefehler folgen
   unverändert dem gemeinsamen Fencing-/Fatal-Recovery-Vertrag.
4. **Begrenzte Prüfumgebung aus vorhandenen Mechanismen bereitstellen.** Der
   genehmigte Validatorgraph ist ausführbares Programm; ungeprüfte Candidate-
   Quellen/Graphen sind bei Inspektion ausschließlich Daten. Nur ein ausdrücklich
   qualifizierter Draft-Testing-Pfad darf Candidate-Code mit benannten Fixtures,
   erlaubten Hostadaptern und technischen Grenzen ausführen. Das ist keine
   Aufnahme des Drafts in normales Matching und verlangt nicht dessen eigene
   vorherige Q1/Q2. Bestehende abgeschirmte Prüf-/Worker-/Kernelgrenzen erweitern;
   keine zweite globale VM, allgemeiner Rust-Validatorloop oder breite Hostfreigabe.
   Runtime liefert taskbezogene Compute-/Preparation-/Heap-/Hostdaten-/Print-/
   Suspension-Grenzen, qualifizierte Interruption und native Watchdogs über die
   vorhandenen Accounts/Settings. Der Validator definiert seine endlichen Check-
   und Fixtureprofile, nicht eigene Ressourcencontroller. Kontrollierte Testeffekte
   und deren Aufräum-/Recovery-Evidenz vollständig erhalten; ein Infrastrukturausfall
   ist keine Kandidatenablehnung und ein erfolgreicher Parser kein Verhaltenstest.
   Bedingte/wiederholte Checks nutzen nur den qualifizierten V35-13/14-Vertrag.
5. **Boot und Providerbereitschaft ohne Selbstapproval integrieren.** Validator
   liefert exakte qualifizierte Pakete, Herkunft/Behavior-Nachweise und abhängige
   Class-Routen; die eine Initialisierung prüft/registriert sie vor Producer-Ready.
   Installationseigene Verifikation bleibt noncircular: kein Validator zertifiziert
   seinen eigenen unbekannten Nachfolger und keine Erstinstallation wartet auf
   dessen eigene Q1/Q2. Root-/Pflichtintegritätsfehler sperren unsichere Freigabe;
   offene authored Reviews oder optionale modellabhängige Checks sind getrennte
   Pending-/Unconfigured-Zustände. Deterministische Review-Routen und Verwaltung
   brauchen weder generierten Modell-Prefix noch einen Provider. Modellschritte
   verwenden §10.1 und den einzigen Prefix-Requestguard: fehlende Bereitschaft
   hält den betroffenen Check unvollständig, statt versteckt einen anderen Provider,
   Prefix oder Tier-2-Pfad zu nutzen. Keine Reviewer-Providerregistry/-Transportschicht.
6. **Menschliches Q2 an vorhandene Betreiber-Identität binden.** Phase 1 liefert
   authentifizierten Caller und CSRF/Origin-Schutz, ohne separate Reviewerrolle.
   Der Validator-Owner prüft im bestehenden exakten View-/Approval-Writer die
   Subject-/Base-/Dependency-/Q1-/Behavior-Referenzen und transaktionale Frische.
   Ein Parameter `human`, Modelloutput oder Component-Management-Call ersetzt
   keine menschliche Betreiberaktion. Unvollständige/überholte Reviews sind nicht
   freigabefähig; ein Q2-Commit mit verlorener Antwort wird idempotent rekonstruiert,
   nicht als neue Entscheidung auf einem neueren Subject wiederholt. Ausführungs-
   beobachtungen, Bootstrap-Nachweise und Reviewfreigabe bleiben getrennte Records
   mit ihren jeweiligen Herkunfts-/Verknüpfungsprüfungen. Seeds behalten ihren separaten vertrauenswürdigen Vertrag ohne Instanz-Q2.

Diese Übergaben sind Anforderungen an bestehende Ports/Records, keine neuen
JSON-Felder oder ein zweites Review-Framework. Aktivierung folgt §8.2: Validator
liefert gültige Approval-/Support-Prädikate und Routebindungen, die gemeinsame
Katalogtransaktion publiziert sie. Ein Modell-Prefix des neuen Katalogs wird danach
vom Prefix-Owner gebaut; fehlende Modellbereitschaft sperrt nur abhängige Calls.
Die Review-/Bootstrap- und Laufzeitabnahme muss vor erster Aktivierung selbst
bestanden sein; diese Reihenfolge erlaubt keine ungeprüften Class-Routen.
Gemeinsame Caller-/Recovery-Nachweise einmal am implementierenden Owner führen
und im Validator wiederverwenden. Dort nur fehlende fachliche Subject-/Coverage-/
Approval-Fälle ergänzen; die Tabellen verlangen keine zweite identische Testpipeline.

**Pflichttests für Phase 3a (durch den Produktions-Caller):**

| Fall | Nachzuweisendes Verhalten |
|---|---|
| Monty-1.0.0-Upgrade | Beide Dependencies/Lockfile konsistent; unterstützte Build-Toolchains bestehen. Recipe-/Resume-/Hostfehler-Kompatibilität, aktive Rechenzeit über mehrere Feeds, erhaltene Live-Revisionen, Allokations-Disposition und tatsächliche Allocator-Speicherbegrenzung sind durch Caller-Level-Tests belegt; alte Fortsetzungen werden vor Versionswechsel reconciliiert. |
| Runtime bereit ohne erzeugten Prefix/Provider | Pflicht-Seeds/Integrität vor Ready; leere Generationen oder unkonfigurierter Provider bleiben sichtbare Produktzustände. Qualifizierter Build mit lokalen Artefakten startet erst nach Ready ohne Modell-/Netzwerkaufruf; fehlendes optionales Profil blockiert nur seine Aktion. |
| Interner ResultOnly-Auftrag | Hostgebundene Herkunft und Auswahl, reale Eingabereferenz und genau ein persistierter Ergebnisabschluss ohne Assistant-Reply; gleichzeitiger normaler Chat behält eigene History/Reply. Öffentlicher Input kann den internen Pfad nicht auswählen. |
| Auftrag/Admission mit verlorener Bestätigung | Crash oder verlorenes COMMIT/HTTP vor/nach Bindung: ursprünglicher Request/Run wird eindeutig rekonstruiert; Konflikt bleibt Konflikt, kein zusätzlicher Run oder Dispatch. |
| ResultOnly-Settlement und Statusprojektion | Erfolgreicher nativer Effekt mit fehlgeschlagenem Ergebnis-/Projektionswrite, doppelten/späten Events und Neustart: echte Ergebnisse/Custody erhalten, Projektion reparierbar, kein Effekt-Replay und kein synthetischer Erfolg. |
| Produktabbruch mit unklarem Ausgang | Cancel-Ack-Timeout/Claim-Verlust und spätes Compiler-/Publikationsergebnis bleiben am Originalrun; stale Updates aktivieren nichts, ungeklärte Effekte und gebundene Kapazität verschwinden nicht durch UI-Neustart. |
| Review-Aufnahme und Resubmit | Dasselbe bestätigte Subject/Validator/Request führt trotz verlorener Bestätigung zu demselben Run; geändertes Resubmit erhält neue Identität. Interner Pfad erzeugt keinen Chat-Reply, kein Rematching und keinen zweiten Claim. |
| Review-Resultat, Evidence-Commit und UI-Reparatur | Native Resultate/Custody bleiben bei fehlgeschlagenem/verlorenem Evidenzcommit oder Projektionsupdate erhalten; idempotenter Originalabgleich ohne erneute Checkeffekte. Stale/wrong-subject/abgefangene Hostfehler können weder Q1 noch Q2 freigeben. |
| Draft-Testing und Ressourcen | Candidate bei Inspektion nur Daten, Ausführung nur im qualifizierten begrenzten Draftprofil. Taskbudget/Print-/Hostdaten-/Suspension-/native Grenzen isolieren den Review; echte globale Fehler werden gefenct, kein zweiter VM-/Ressourcenowner. |
| Review-Boot und menschliche Entscheidung | Noncircular Seedprüfung vor Review-Produzenten; Pending/fehlender Modellprovider blockiert keine sichere unabhängige Route. Q2 nur als authentifizierte exakte menschliche Entscheidung, unverändert beim verlorenen Response rekonstruierbar. |
| Frische und bestehende DB, noch kein Chat | Seeding/Prüfung vor VM-Start; genau eine globale VM wartet vor Ready; auch REPL-/Nicht-WebUI-Boot nutzt diesen Pfad. |
| Zweiter Produkteinstieg / zweiter Instanzprozess | Gleiche Runtime teilt Handle; gleiche Instanzablage erlaubt keinen zweiten Owner; fehlgeschlagener Boot/Crash gibt Besitz kontrolliert frei. |
| Fehlende/korrupte Orchestrator-Zeile, Syntaxfehler, früher VM-Abschluss | Boot scheitert vor Worker-Claims/Ingress; gestartete Ressourcen werden beendet; keine spätere lazy VM. |
| Chat A → B → A und zwei Inputs im selben Chat | Eine globale Dienstgeneration; richtige History, genaue Eingabereferenz und Antwortadresse; kein späterer Input vorzeitig im Prompt; keine doppelte Usernachricht. |
| Externe Auth wartet, anderer Chat schreibt, Parent wartet auf Child | Unabhängige Arbeit und benötigte Fortsetzungs-/Child-Ereignisse erreichen Monty; kein Deadlock, rekursives Self-Wait oder zusätzliches Tool-Approval. |
| Queue voll / doppelter Versand / Receiver geschlossen | Definierte Backpressure; akzeptierte Arbeit bleibt in DB erreichbar; Control-Ereignisse gehen nicht verloren; keine doppelte Ausführung/Antwort. |
| Run-Abbruch, Timeout, Claim-Verlust, verspätete Signale | Kein globales `FINAL`; kein Effekt/Reply durch veralteten Attempt; nächster Run bleibt unbeeinträchtigt. |
| Lange Idle-Phase und viele Turns mit kleinen Limits | Globale VM bleibt; Task-Rechenzeitlimits bleiben wirksam; künstliche Tokenlimits gelten nur bei aktiviertem globalem Tokenbudget-Schalter; Speicher/stdout/History-Caches wachsen nicht unbegrenzt. |
| Monty-Limits während Idle, aktiver Ausführung und suspendiertem Task ändern | Kein Neustart/Wechsel der VM-Generation; Übernahme innerhalb der definierten Reaktionszeit, Verbrauchszähler bleiben erhalten. Tasklimitfehler und globaler Heapfehler werden getrennt behandelt. Nicht einhaltbare manuelle Heap-Senkung wird explizit abgelehnt; bisherige wirksame Revision bleibt. Automatische Senkung bleibt bei höherem lebendem Heap als Ziel ausstehend; Entlastung und Backpressure werden wirksam. UI/API zeigen Ziel-/Effektivrevision und Übernahmefehler. Rust-Ressourcenprüfung und Monty-Tracker verwenden nach bestätigter Übernahme dieselbe Revision und dieselben Grenzen; Erhöhung/Senkung während aktiver Tasks erhält bereits verbrauchte Rechenzeit, externe Deadlines und ausgeschlossene Wartezeiten bleiben getrennt. |
| Start-/manuelles Budget und optionale Automatik | Standard bleibt nach einmaliger Dimensionierung konstant; keine periodischen Druckmessungen. Bestehende Betreiberwerte bleiben erhalten. Live manuelle Änderungen ohne VM-Neustart; unsichere Senkungen vor DB-Commit abgelehnt. Optionale Automatik wächst nur bei verlässlicher Kapazität; milder Druck pausiert keine Aufnahme, fehlende Messung verhindert Wachstum ohne pauschalen Stillstand. Kritischer Druck/ungenügende Kapazität begrenzt Aufnahme; Fortsetzungen bleiben erhalten, keine Doppelzählung/Oszillation. |
| Tool-/Extensionänderung zwischen Tasks | Globale Toolregeln gelten vor jedem folgenden Dispatch; keine zusätzlichen Taskgrants. Taskgebundene Bindings werden freigegeben, Komponentenrevisionen nach ihrem Vertrag gewählt. |
| Crash/Reply-Persistenzfehler nach externem Effekt | Kein blindes Replay; bestätigte und ungeklärte Effekte unterscheidbar; Run-/Reply-Zuordnung bleibt korrekt. |
| Shutdown idle, busy, mit Auth-/Child-Wait und vollem Eingang | Dienst-Task gejoint, Status persistiert, keine späten Replies/Calls auf geschlossenen Stores; Postgres beendet zuletzt. |
| Code-/Schema-Upgrade mit offenen Vorgängen | Alte Version zuerst drain/reconcile; geprüfter neuer Code startet einmal; persistierte Arbeit korrekt adressiert, keine alte RAM-Session übernommen. |
| Lange Python-Schleife und volle Workqueue | Begrenzte Zeitscheiben; andere Aufgaben, Cancel und Settings erreichen die definierten Reaktionszeiten; kein Rust-Recipe-Loop. |
| Recipe-Schritt scheitert nach erfolgreichem Toolcall | Kein stiller Tier-2-Neustart oder Tool-Replay; bestätigte und ungeklärte Effekte sowie Fehlerantwort korrekt persistiert. |
| Komponentenrevision während Task-Wait ändern | Fortsetzung verwendet ihre ursprünglichen Revisionen; folgende Vorgänge verwenden die neue validierte Revision; globale Toolregeln bleiben aktuell. |
| Retainierte Toolimplementierung während Wait ändern | Alter Task nutzt ursprüngliches Artefakt/Adapter und Approval; neuer Task nutzt den neuen genehmigten Stand; fehlendes altes Artefakt erzeugt expliziten Fehler statt latest-Substitution. |
| Reclaim/Crash zwischen Dispatchabsicht, Effekt und Completion | Invocation-Zähler und Key/Argumente bleiben; fehlende Completion ist ungeklärt, nicht effektlos; nur genehmigte sichere Wiederholung/Reconciliation, keine neue VM als Replay-Abkürzung. |
| Tokenbudgets ausgeschaltet, Inhalte über bisheriger Budgetkonstante | Retrieval, Vorwissen, History und Taskverbrauch werden nicht durch künstliche Tokenbudgets begrenzt; Recipe-Hint und erneutes Retrieval beachten denselben Schalter. Tokenzählung funktioniert weiter; technische Modellgrenzen bleiben wirksam. |
| Settingspersistenz / Runtime-Acknowledge scheitert | Kein falscher Übernahmeerfolg; Ziel-/Effektivrevision und Fehler sichtbar; kontrollierter Wiederanlauf. |

**Abnahmekriterium:** Der gemeinsame Produktionsboot startet ohne erste Chatnachricht genau einen verifizierten Monty und wartet auf den Ready-Handshake. Tests durch den Produktions-Caller belegen Kontexttrennung, Fortsetzungen, Fairness, Live-Budgets und Recovery. Native PostgreSQL-Testumgebung verwenden; übersprungene DB-/Runtime-Tests zählen nicht als Nachweis. Zusätzlich gilt die vollständige Recipe-Abnahme aus Phase 7.

### Phase 4 — Vollversion als einziger Produkt-Build

1. Cargo-Features inventarisieren und klassifizieren: Produkt-/Deployment-Varianten entfernen; reine Plattform-/Backend-, Test- und optionale Entwicklungsfeatures nur behalten, wenn sie nachweisbar keine abweichende Produktedition erzeugen.
2. Die Funktionsliste aus Phase 0 im regulären Build absichern. Feature-Flags für Produkteditionen entfernen; Plattformbedingungen und reine Testunterstützung erhalten. Funktionen müssen erreichbar und ausführbar sein, nicht nur einkompiliert.
3. Unterstützte Produkteinstiege ausdrücklich benennen. TUI/WebUI/Provider/Channels/Extensions als Produktoberflächen bzw. Einstellungen/Integrationen behandeln. Nur belegte Legacy-Doppelpfade entfernen; gewünschte Fähigkeiten erhalten.
4. README, Installationsskripte, Dockerfiles, Release-Workflow und Cargo-dist-Metadaten auf eine Produktvariante und einen kanonischen Einstieg ausrichten. Zielplattformen dürfen weiterhin separat kompiliert werden; sie stellen keine Editionen dar.
5. Einen koordinierten Upgradepfad mit erforderlichen versionierten Einzelschritten anbieten. Unbekannte alte Profile nicht permissiv mappen; Migration meldet Mehrdeutigkeiten und stoppt vor Datenänderungen.
6. Alle lang laufenden Produktoberflächen an den in Phase 3a gestarteten globalen Dienst anschließen; Feature-Kombinationen ohne verifizierten Orchestrator-Boot nicht als vollständiges Produkt ausliefern. Verwaltungs-CLI bleibt ohne Hintergrund-Agentenstart.

### Phase 5 — WebUI auf eine Betreiberoberfläche reduzieren

**Monty-Settings:** „Maximale Rechenzeit pro Aufgabe“ (`max_duration_secs`, Sekunden; Default **600 Sekunden / 10 Minuten**), gemeinsames konfigurierbares Monty-Speicherbudget und nur bei nachgewiesener separater Unterstützung Taskallokationen (`max_allocations`, Anzahl) vollständig verwalten. Einen abgelösten Allokationsparameter gemäß Phase 3a ausdrücklich migrieren und aus der aktiven UI entfernen. Speicher standardmäßig „Beim Start bestimmen“; manuelles Budget und optionale langsame Automatik anbieten. Speicherwert, Reserve, Start-/Ersatzwert und Automatikintervall mit eindeutiger MiB-/Sekundenumrechnung verwalten. Heapverbrauch, berechnetes Ziel, wirksames `max_memory_bytes`, RAM-/Druckmessung samt Aktualität und Anpassungsgrund anzeigen. Budgetbedeutung, Ersatzwerte, Grenzen und Übernahme nach Phase 3a erklären. Ziel-/Effektivrevision, ausstehende automatische Senkung und Ablehnung einer nicht einhaltbaren manuellen Senkung unterscheiden. Alle gültigen Übernahmen erfolgen ohne Neustart; tatsächliche Limitprüfung und Automatik end-to-end testen.

1. Ein Betreiber-Navigationsmodell ohne User-/Tenant-/Project-Rollenwechsel, Security-Profile-Auswahl oder pro-Projekt Security-Einstellungen festlegen.
2. Eine Seite/ein eigener Tab pro Schutzebene anlegen. Jede Ansicht zeigt den aktiven Wert, den wirksamen Default, Wertebereich/Optionen, kurze Folgenbeschreibung, Neustartbedarf und Validierungsfehler. Alle Änderungen gelten instanzweit.
3. **Sandbox:** unterstützte Backend-, Mount-, Pfad-, Prozess-/Befehls-, Env- und Ressourcenparameter vollständig verwalten. Im Katalog angeben, ob Änderungen laufende Prozesse erreichen oder erst folgende Aufrufe betreffen. Keine unwirksamen Schalter; technische Regeln bleiben von globaler Toolzulassung getrennt.
4. **Secret-Schutz:** zentralen Key-Provider/-Status, Rotation/Recovery und Metadaten (Name, Provider/Verwendung, Ablauf/Status, letzte Nutzung) verwalten. Setzen/Ersetzen/Löschen und technische Credential-/Ablaufregeln vollständig anbieten. Klartext wird nach dem Setzen nicht erneut zurückgeliefert. Secret-Brokerparameter begründen keine operationsbezogene Toolfreigabe.
5. **Netzwerkregeln:** unterstützte Egress-/Proxy-, Host-/IP-/Port-, DNS-/Redirect-, Zeit- und Größenregeln zentral verwalten. Änderungen erreichen den verwendeten Resolver/Transport; Wirkung auf bestehende Verbindungen und neue Requests ausdrücklich festlegen und testen.
6. **Tool Permissions:** vollständigen Runtime-Toolkatalog mit Beschreibung, Effekten, globaler Zulassung/Sperrung und unterstützten Parametern anzeigen. Keine User-/Projekt-/Run-Auswahl, keine Einmalfreigaben, keine zusätzlichen `Ask`-Gates. Altdaten mit `ask` im Upgrade explizit behandeln; keine stille permissive Umdeutung. ToolSkills/Skills sind getrennte Komponentenkataloge. Aktuelle Runtime-Wirkung einschließlich bereits laufender Recipes end-to-end testen.
7. **Audit:** Kategorien, Sink, Export, Filter und zulässige Retention vollständig verwalten. Dauerhaft zu erhaltende LLM-History, Schritte und relevante Ausgaben von löschbaren Diagnose-/Auditdaten unterscheiden; allgemeine Retention darf diese History nicht löschen. Erforderliche Ereignisse und Secret-/Token-Redaktion erhalten.
8. Globale Konfigurationsseiten in Instanz-Einstellungen überführen. „Alle Daten einsehen“ über einen vollständigen Betreiberzugriff auf Settings, Runs, Threads, Audit, Secrets-Metadaten und Integrationen; geheime Werte bleiben aus Sicherheitsgründen nur set-/rotate-/clear-bar, nicht rücklesbar.
9. Projekte und Threads dürfen weiter als Navigations-/Organisationshilfen erscheinen. Entferne ausschließlich policywirksame Scopes; wo der Nutzer „Projekt“ als Arbeitsbereich nutzt, entweder das UI-Konzept als Gruppierung belassen oder separat aus dem Produktumfang streichen.
10. Platzhalter gegen die Funktionsliste aus Phase 0 prüfen. Gewünschte Funktionen mit vorhandenen Komponenten/Recipes und echten v2-Verträgen fertigstellen. Nur nachweislich obsolete oder außerhalb der festgelegten Vollversion liegende UI/API-Reste entfernen und begründen. Kein Löschen gewünschter Fähigkeiten als Ersatz für Implementierung.
11. Vorgesehene Gateway-Neustartfunktion über einen authentifizierten, CSRF-geschützten Managementpfad implementieren. Kontrollierter Shutdown nach Phase 3a, Erhalt des Verwaltungszugangs bzw. dokumentierter CLI-Recovery und ehrlicher Restartstatus gehören zum Vertrag. Eine ausdrückliche UI-Neustartaktion ist keine zusätzliche Toolfreigabe.
12. WebUI-Settings API zentralisieren und generierte/vertraglich getestete Request-/Response-Schemas nutzen, damit UI und Server dieselbe Instanzkonfiguration darstellen.
13. UI-Tests decken vollständige Betreiberverwaltung, fehlende Rollenwahl, globale Tools, Intents/Varianteneditor und Matching-Test, Live-Monty-Limits, Ziel-/Effektivrevision, redigierte Secrets und Zugangsschutz ab. Settings-Erfolg durch tatsächliches Runtime-Verhalten belegen.
14. Chat-Senden, externe Auth-Fortsetzung und Cancel als Eingabe-/Control-Übergabe betreiben. Operationsbezogene Tool-Gate-Resolve-Pfade entfallen; Q2 behält seine eigene menschliche Validierungsroute. Chatoperationen und SSE-/WS-Reconnect erzeugen/stoppen keine globale VM. Startup/Recovery/Shutdown zeigen tatsächlichen Status und bestätigen nur dauerhaft aufgenommene Arbeit.
15. **Komponenten und Approval zusammenhängend verwalten.** Skill-Prosa und
    zugeordneten PythonCode gemeinsam sichtbar, aber als neue unveränderliche
    Revisionen bearbeitbar machen. ToolSkill-Bindingmetadaten und Toolimplementierung
    getrennt anzeigen; Beschreibung ist kein ausführbarer Code. Vollständige
    Assoziation, ausgewählte/geprüfte Revisionen, Approval-Provenienz und fehlende
    Verträge/Evidenz darstellen. Entwurf, individuelle Validierung, exakte
    Kombinationsapproval, Aktivierung und tatsächliche Callable-Verfügbarkeit
    unterscheiden. Unterstützte Store-/Review-APIs mit Phase 0a implementieren,
    nicht aus heutigen Konstruktoren herleiten. Fehlende Approval/Artefakte oder
    inkompatible Verträge verhindern Aktivierung; unveränderte Skillrevisionen
    dürfen mit separat genehmigten neuen Kombinationen wiederverwendet werden.
    UI-/API-Tests prüfen authored versus trusted system_seed ohne Label-Bypass,
    konkurrierende Revisionen und unveränderte historische Taskauswahl.

### Phase 6 — Datenbank-, Dateisystem- und Secret-Migration

1. Vor Schemaänderungen Backup/Restore und Dry-Run für jede bestehende Installation bereitstellen.
2. Historische Berechtigungspartitionen in eine Instanz überführen; Projekte als Organisation erhalten. Stabile IDs bevorzugen. Nur tatsächliche Kollisionen remappen und alle abhängigen Foreign Keys, Events, Thread-/Kanal-Bindings sowie Komponenten-/Intentbeziehungen konsistent nachführen. Diesen Entwurf bereits vor den betroffenen Änderungen in Phase 2/3 erstellen.
3. Vorhandene Secrets erhalten nur dann ihre Semantik, wenn AAD/Schlüsselversion sicher migrierbar sind; sonst sicheren Neu-Set-Vorgang verlangen. Migration darf nie durch Scope-Kollaps Ciphertext über falsche IDs entschlüsseln.
4. Nur alte Rollen-/Scopeberechtigungen, Toolgrants und Invocation-Approval-Leases entfernen, nachdem alle Leser/Schreiber umgestellt sind. Worker-Claims/Attempt-Tokens, externe Auth-/Accountdaten, Q1/Q2 und fachliche Identitäten erhalten. Alte Toolsettings einschließlich `ask` mit dokumentierter Betreiberentscheidung in das globale Modell überführen.
5. Schema-, Index-, Constraint- und nötige Namespaceänderungen durch neue versionierte Migrationen einführen. Bereits angewandte Migrationen und deren Prüfsummen unverändert lassen. Komponentenchecksums sind ein gesonderter Integritätsvertrag. Datenmigrationen wiederaufnehmbar und idempotent gestalten; kein stilles Löschen produktiver Daten.
6. Integrationstests mit alten und neuen Datenständen, Kollisionen, Teilfehlern, Wiederanlauf und Rollback/Restore-Fall durchführen.
7. Task-/Attempt-/Continuation-/Reply-Beziehungen sowie Recipe-UUIDs, Varianten, `step_link`, Komponenten-Includes, `dependency_registry` und Intent-Zuordnungen vollständig erhalten bzw. konsistent remappen. Migrationsprüfung weist jede Referenz auf eine vorhandene passende Revision nach; keine dangling UUIDs oder durch Konflikte überschriebene Varianten. Prefix-/Cache-Frische gegen den migrierten validierten Bestand nach §8.2 abgleichen; notwendige Neubauten nach Ready über den qualifizierten Prefix-Auftrag ausführen, ohne eine Generation ungeprüft zu aktivieren oder den Migrationsrunner zum Compiler zu machen. Nur notwendige Felder ergänzen; keine zweite dauerhafte Turnqueue oder persistente `BuildInstruction`-Tabelle. RAM-Sitzungen sind keine DB-migrierbaren Snapshots.
8. **Phase-0a-Verträge mitmigrieren.** Versionierte Komponenten/Assoziationen,
   getrennte vertrauenswürdige Approval-Evidenz, Actual-Tool-Artefakte/Adapter,
   Taskmanifeste, durablen Invocation-/Retry-/Effektstatus und stabile Policy-
   Mappings samt Aufbewahrung implementieren und erhalten. Fehlende Altverträge
   oder Evidenz explizit als offen behandeln; weder Labels noch rekonstruierte
   UUID-Paare ersetzen Verhaltens-/Q2-Nachweise. Ursprüngliche geprüfte Records
   und historische Taskreferenzen unverändert erhalten. Nötiges Remapping
   ausführungsrelevanter Identitäten/Verträge nicht als nachträgliches Umschreiben
   einer alten Approval ausgeben: neue aktive Kombination mit nachvollziehbarer
   Migration und erforderlichem validiertem Evidenzvertrag bereitstellen.
   Backups enthalten benötigte Implementierungsartefakte und Wiederherstellungs-
   referenzen; Restore prüft Verfügbarkeit/Kompatibilität sowie Zähler/Status.
   Keine bereinigende Löschung noch benötigter Versionen, Approval-Datensätze,
   Checkpoints oder Deduplication-Records.
9. **Verbundene Betreiber-Stores statt stiller Legacy-Adapter.** Den vorhandenen
   `ReductionRuleStore` direkt aus dem produktiven PostgreSQL-Pool in die WebUI-
   Fassade verdrahten; das nie initialisierte `pg_memory_doc_store`-Feld und den
   unbrauchbaren MemoryDoc-Adapter entfernen. Eine eigene Ruleset-Tabelle nimmt
   validierte Altregeln beim ersten Lesen transaktional auf, ohne Originalzeilen
   zu löschen. Historischen Projekt-UUID einmal aus dem ursprünglichen Slug
   ableiten; keinen gespeicherten UUID erneut hashen. Ungültige JSON-/Regelwerte
   und doppelte IDs explizit melden. Eine vollständige validierte Ersetzung kann
   fehlerhafte Regeln reparieren; eine explizit leere neue Liste bleibt
   maßgeblich und darf alte Regeln nicht wieder aufleben lassen. Cold-Read-Races,
   Originalerhalt, Korruption, Ersetzung und Lesen nach Runtime-Neustart mit
   echtem PostgreSQL über die gewöhnliche WebUI-Fassade prüfen. Der entfernte
   Engine-Loader wird dadurch nicht wieder aktiviert; Daten-IDs verleihen keine
   Toolberechtigung. Implementierungs-/Abnahmestand in
   `docs/plans/simplified-v3-implementation.md` verfolgen.

10. **Komponenten-, Prefix- und Review-Daten gemeinsam migrieren und sichern.** Der
    vorhandene Migrationsowner nimmt die Teilnehmerdaten aus
    [Prefix §12](docs/plans/prefix_v3_upgrade.md#coordinated-upgrade-and-recovery)
    auf: Legacy-Bundles, Quellenoriginale/Registrierungen und Beobachtungen,
    Profile, akzeptierte Designs/Model-Spezifikationen, Generationen/Composites,
    Consumer-/Providerbindungen, Job-/Run-/Invocationverweise sowie benötigte
    Compiler-/Tokenizer-/Template-/native Artefakte und ihre exakten Manifest-
    referenzen. Prefix liefert die fachlichen Migrationen und Prüfer; dieser Plan
    führt sie mit den übrigen Teilnehmern unter derselben Versions-/Lock-/Backup-
    Grenze aus. Kein zweiter Migrationsrunner, Datenkatalog oder Restorebefehl.

    Vor dem Backup die Referenzen laufender/suspendierter Aufträge und ungeklärter
    Effekte erfassen und benötigte Artefakte vor Bereinigung schützen. DB-Stand und
    externe Artefakte nach dem gemeinsamen Drain/Fencing über ein vollständiges
    geprüftes Inventar zusammen sichern; eine DB-only-Sicherung ist unvollständig,
    wenn benötigte Dateien fehlen. Restore prüft tatsächliche Bytes/Checksums,
    kompatible Reader/Adapter, Original-IDs und alte Task-/Prefixauswahl samt Counts
    und Receipts. Fehlende/unvertrauenswürdige Altbelege bleiben Legacy/Unverified;
    keine nachträgliche Approval oder stilles Ersetzen durch den neuesten Compiler.
    Externe Inferenzprozesse/GPU-Caches werden nicht als wiederhergestellter Zustand
    behauptet; nach Restore ist Cachewiederverwendung neu zu verifizieren.

    Abnahme über denselben Koordinator: frische Installation, Upgrade mit
    Overrides und laufendem Build, unterbrochene Migration, fehlendes/korruptes
    Quell- oder Compilerartefakt und echter Restore inklusive beider Modellconsumer.
    Fachliche Snapshotreferenzen bleiben erhalten; keine portable alte RAM-VM
    oder automatische erneute Publikation unterstellen.

    Der Validator liefert als weiteren Teilnehmer Legacy-/neue Submissions,
    versiegelte Candidate-/Base-/Dependency-Subjects, davon getrennte Validator-
    Recipe-/Rule-/Occurrence-/Source-/Engine-/Native-Auswahlen, Beobachtungen,
    Q1-/Behavior-/Human-Q2-/Seed-/Association-/Pure-Evidenz, ursprüngliche
    Run-/Attempt-/Custodyverweise, Routen und Overrides. Derselbe Koordinator
    sichert diese vollständigen Closure-/Provenienzreferenzen und prüft sie beim
    Restore mit den Validator-eigenen vertrauenswürdigen Lesern. Ein vorhandener
    Datensatz oder erfolgreicher SQL-Import erzeugt keine nachträgliche Approval;
    fehlende/unsealed Altbelege bleiben ausdrücklich reviewbedürftig. Keine
    zweite Review-Migration, Erfolgsrekonstruktion aus Labels oder Restore-Queue.
    Gemeinsame Fälle: wartendes Q2, Resubmit/Validatorwechsel während offenen
    Reviews, verlorener Evidenz-/Approval-Commit, deaktivierte Routen/Overrides,
    fehlendes Interpreter-/Sourceartefakt und inkompatibler Upgrade-Rollback.

    Composition liefert die Familien-/Usage-/Alias-/Consumer- und Override-Ledger,
    alten/neuen Export-/Command- und nativen Closure-Artefakte sowie benötigte
    Source-/Derived-Dokumentreferenzen aus dem gemeinsamen Migrationsvertrag.
    Mit derselben Backupgrenze sichern und tatsächliche alte Loader-/ABI-/Reader-
    Verfügbarkeit beim Restore prüfen. Ausgelieferte Successors überschreiben
    keine Operatorversion anhand eines Namens. Alte benötigte Artefakte nicht
    wegen des abgeschlossenen neuen Familienimports löschen; fehlende alte
    Ausführbarkeit blockiert ein Upgrade, das alte Tasks unfortsetzbar machen
    würde; keine latest-Substitution.

### Phase 7 — Tests, Architekturverträge und Dokumentation bereinigen

1. Architekturtests auf §1.1 ausrichten: vollständige Betreiberverwaltung, globale Toolregeln, eine Distribution und ein Orchestrator. ToolSkill-Bindung führt keine Tools aus; Python/Recipes sequenzieren, Kernel setzt Regeln durch. Kein zweiter Rust-Recipe-Loop.
2. Alte Profil-/Deployment-Matrix-Tests durch kompakte Policy-Invariantentests ersetzen. Alte Multi-Tenant-Isolationstests nicht kommentarlos löschen; als bewusst aufgegebene Betriebsform dokumentieren und Migrationstrennungen testen.
3. Caller-Level-Tests für Capability, Prozess, Netzwerk, Secrets, Filesystem, MCP, Subagenten, Trigger/Inbound und Verwaltung erhalten. Einzelfreigabetests durch globale Allow-/Deny- und Live-Sperrtests ersetzen. Claims/Attempts, externe Auth und Q2 separat weiterprüfen.
4. Reborn-E2E- und Playwright-Szenarien vereinheitlichen; veraltete README-Annahmen (z. B. alte `libsql`-Buildpfade) aktualisieren. Platzhalter-UI kann nicht als Feature-Erfolg zählen.
5. Root-/Crate-Verträge, Operator-Doku, Implementierungsbericht und komponierte/seedingrelevante Beschreibungen auf dasselbe Ziel bringen. Historische Testergebnisse nicht umschreiben. Alte operationsbezogene Approval-Vorgaben als abgelöst markieren; Tierregeln wie Shell-/Subagent-Tier-1 bleiben unabhängig von Toolzulassung bestehen.
6. Zugehörige Tests während jedes Umsetzungspakets abschließen. Vor Cutover Format, Clippy, Workspace-, native PostgreSQL-, Architektur-, UI-, vollständige E2E-/Auth-/Migrations-/Restoreprüfungen ausführen. Funktionsliste vollständig abnehmen; übersprungene Pflichtprüfungen sind offen.
7. Phase-3a-Matrix und Recipe-Matrix unten in CI durch den Produktionspfad prüfen. Gemeinsamen Boot, Seeds/Checksums, Grenzen und unabhängigen Run-Abbruch nachweisen. Benchmarks aus §5 mit derselben Umgebung wiederholen. Vor Rust-Build/Test/Check/Clippy freien NVMe-Platz gemäß CLAUDE.md prüfen; Cargo-Zielverzeichnis bleibt `/Users/ollama/brassclaw-target`.
8. Phase 0a und alle vier Ground-truth-Guides verbindlich in die Architektur-,
   Store-/Migrations-, Validator-, IBS-/Composer-, Host-/Runner- und UI-Abnahme
   einbeziehen. Matrixfälle unten mit realen unterstützten Pfaden und beobachteten
   Argumenten/Effekten prüfen. Ergebnisse dem getesteten Stand/Caller zuordnen;
   referenzierte Leitfäden oder Schema-Unit-Tests allein schließen kein Paket ab.

**Pflichttests für den vollständigen Recipe-Pfad:**

| Fall | Nachweis |
|---|---|
| Tier-0-Match | Richtiger Intent/Variante/Variablen → IBS → Bindung → PythonCode → Antwort/History; null LLM-Aufrufe. |
| Tier-1-Match | Vorgesehene LLM-Schritte mit korrektem Komponenten-/Recipe-Kontext, Reihenfolge und anschließendem Dispatch; keine verdeckten zusätzlichen LLM-Aufrufe. |
| Echtes No-Match / Tier 2 | Instruction-/Recipe-gesteuerte Promptbildung mit aktueller Kohai-Prefixgeneration und zur konkreten Eingabe gehörender History. |
| Mehrdeutigkeit / technischer Matchingfehler | Auswahl nachvollziehbar bzw. expliziter Fehler; keine Gleichsetzung mit No-Match. |
| Schrittfehler nach erfolgreichem Effekt | Kein Task-Neustart als Tier 2, kein Replay; Ergebnisse/Fehler/History korrekt gespeichert. |
| Step-Isolation / abhängige Kette | Unabhängige Tasks/Attempts sind isoliert; benötigte Zwischenwerte bleiben innerhalb eines Recipe erhalten, auch über Child-Ausführung und externe Auth-/Tool-Waits. |
| Sempai / Q1 / Q2 | Erfolgreicher Tier-2-Verlauf erzeugt Vorschlag; automatisches Q1 und menschliches Q2; erst validierte geeignete Komponenten werden aktiv und ein erneuter Match verwendet sie. |
| Live-Änderung / Fortsetzung | UUID-/Variantenverweise und verwendete Revisionen bleiben korrekt; neue Tasks sehen neue aktive Revision und Prefix, globale Toolsettings gelten sofort vor folgenden Dispatches. |
| Sämtliche Produkteinstiege | CLI/WebUI/Trigger/Kanäle und unterstützte MCP-/Subagentpfade verwenden denselben Recipe-/Kernel-Vertrag. |
| Vollständige Assoziation / exakte Kombinationsapproval | Skill/code/ToolSkill/Tool und transitive Revisionen/Checksums stimmen mit vertrauenswürdiger approval_id überein; einzeln validierte, aber ungeprüfte Kombination verweigert Aktivierung/Assembly. |
| Approval-Provenienz / semantischer Review | Authored verlangt Q1, menschliches Q2 und Verhaltensnachweise; nur trusted system_seed erlaubt q2_ref=null mit erforderlicher Evidenz. Source/status allein qualifizieren nicht. Prosa/Code-Bedeutung wird vor Aktivierung geprüft; Tier-0-Startup verwendet keine LLM-Semantikprüfung. |
| Neueste aktive Kombination inkompatibel oder ohne Approval | Expliziter Assemblyfehler vor Effekten; kein stiller älterer Stand und kein Tier-2-Fallback. Bereits gepinnter alter Task behält seine ursprüngliche genehmigte Kombination. |
| Matching-/Aktivierungsrace | Recipe/Variante/step_link/Inputlayout, Komponenten, Artefakte und Approval stammen aus einer Generation; kein gemischtes Manifest. |
| Retainierte Toolartefakte / Host-Upgrade | Alter Task ruft seine tatsächliche alte Implementierung/Adapter auf, neuer die neue; falsches/fehlendes Artefakt oder inkompatible ABI scheitert explizit. Restore/Upgrade übernimmt keine ungeprüfte alte RAM-Ausführung. |
| Genau eine UUID / Bindingpaar | Leere/mehrfache component-Includes, falsche Klassen und unpaarige Bindings werden abgelehnt. Binding allein führt nichts aus und erzeugt keinen Grant; passende Codeusage kann nach Vorbereitung genau ihre deklarierte Operation ausführen. |
| Interne PythonCode-Komposition | Erlaubte kleine Helpers werden geordnet mit passenden Verträgen und Versionen assembliert; Zyklen, fehlende Referenzen, Symbol-/Inputkonflikte und versteckte unabhängige Toolcalls werden abgelehnt. |
| Pure Logic / abhängige Kette | Pure Logic mit deklariertem result braucht null Bindings/Toolcalls/LLM-Aufrufe. Erlaubte direkte Kette deckt jeden Toolcall ab und erhält State/Continuation ohne Replay; unabhängige Calls bleiben getrennte Schritte. |
| Reale ToolSkill-Vorbereitung | Deskriptor/Assoziation löst korrektes Tool, Callable/Adapter und Implementierung auf; fehlendes Binding/Registrierung oder widersprüchliche Parameter/Selektoren scheitern. Legacy-Metadatum oder rust_directive allein gilt nicht als Ladeerfolg. |
| Typisierte inputs / fehlerhafte Captures | Exakte Referenzgrammatik, deklarierte lokale Zuordnung und validierte Capturetypen erreichen Monty; Regexfehler, fehlende/duplizierte/unzugeordnete Inputs, forward references oder Ausdrücke werden vor Nutzung abgelehnt. |
| Feindlicher String / Quelltextintegrität | Quotes, Backslashes, Newlines und Python-looking Text bleiben Daten in inputs; Body/Checksum unverändert, keine Quellsubstitution oder Auswertung als Referenz aus Benutzerdaten. |
| Rekursive Schemas / Missing-Null-Default | Listenitems, Objektfelder/typisierte Extras, Presence und Nullability geprüft; Defaults nur für fehlende Consumerinputs und unabhängig pro Binding. Ungültige Defaults/Werte oder Outputs nie repariert/fabriziert. |
| Numerische und berechnete Argumente | Bool, falscher Typ, Oversize/Transportverlust, ungültige berechnete Grenzen vor Dispatch abgelehnt; Intervallprofil aus skills.md prüft 2147483647 als Grenze und 2147483648 als Ablehnung, nicht als universelles VM-Maximum. |
| Ergebnis-/Consumer-Kompatibilität | Deklarierte Kanten und tatsächliche Outputs rekursiv geprüft; fehlende/optionale/nullfähige Felder und Listenzugriff sicher behandelt. Startup liest keine noch nicht existierenden Ergebnisse; fehlerhafter Output löst kein Effekt-Replay aus. |
| Globale Sperre über Versionen/Aliase | Alt-/Neuversion und alle Dispatch-IDs derselben Toolidentität verweigern Folgeaufrufe nach Sperre. Fehlende oder widersprüchliche Mappings oder Policy fail closed; keine neue Allow-Regel durch Umbenennung. |
| Exakter Retryvertrag / persistente Zähler | max_attempts=3 erlaubt höchstens Initialdispatch plus zwei zulässige Wiederholungen, auch über Wait/Reclaim/Crash. Bool/Bruchzahlen, retry mit not_assumed oder ungültige Outcome-/Evidenzrecords werden abgelehnt. |
| Retryevidenz / unbekannte Completion | Nur deklarierte confirmed_no_effect_transient/unknown_completion mit passender trusted read_only/deduplicated-Evidenz. Dedup benutzt ursprünglichen Key/Argumente und dauerhaft gehaltene Records; Timeout allein ist kein No-Effect-Beweis. |
| Durabler Dispatchrecord / Fehler nach abgeschlossenem Effekt | Fehler bei notwendiger Intent-/Zählerpersistenz verhindert Dispatch; Crash ohne Completion bleibt ungeklärt. Bestätigter Effekt wird nach Output-/Reply-/Folgeschrittfehler nicht wiederholt, Counts und Manifest bleiben erhalten. |
| Schema-/Datenmigration und Restore | Altverträge/Evidenzlücken sichtbar, gültige Records/Artefakte/Manifeste/Counts erhalten; keine gefälschte Approval oder Löschung benötigter Versionen. Restore nutzt kompatiblen realen Caller, nicht nur erfolgreich geladenes JSON. |
| Shell / spawn_subagent | Jeder konsumierende Recipe-Pfad bleibt Tier 1 trotz fixer Parameter, Seeds, Approval oder globaler Toolzulassung. |

### Phase 8 — Cutover und Entfernung des Legacy-Modells

1. v3-Schema/Config-Format versionieren; `doctor` zeigt vor Upgrade alle zu migrierenden Profile, Datenpartitionen und unsicheren Mehrdeutigkeiten.
2. Upgrade mit genau einem Management-/Migrationsowner: Konfigurations-Dry-Run → Admission stoppen → Vorgänge drainen/reconciliieren → Orchestrator/Worker stoppen → konsistentes Backup von DB, nötigen Dateien und Schlüsselmaterial samt geprüftem Restore → versionierte Datenmigration → Secret-/Referenzprüfung → kompatibles Binary/Seeder und gegebenenfalls ausdrücklich angeforderter Repair → Komponentenintegrität → globaler Monty bis Ready → Worker/Ingress → Betreiber- und Recipe-Smoke-Tests. Embedded-PG für Backup/Migration durch diesen Owner verfügbar halten oder kontrolliert exklusiv neu starten; beim finalen Shutdown zuletzt stoppen. Kein Mischbetrieb alter/neuer Orchestratoren und kein pauschaler Repair über Betreiberanpassungen.
3. Bei Mehrdeutigkeiten abbrechen und Anweisung zur Betreiberentscheidung ausgeben; keine automatische Auswahl von `Full`, `Yolo` oder einem anderen permissiven Profil.
4. Legacy-Profilparser, Rollen-/Scopefallbacks, Einzelfreigabepfade, Doppelkompositionen und obsolete Fixtures erst nach Funktions-, Migrations- und Leistungsnachweis entfernen. Interne Plattformbackends, Worker-Claims und gewünschte Produktoberflächen erhalten.
5. Release-Notes und Rollbackpfad nennen; Rollback bedeutet Wiederherstellung des vorherigen Backups, falls das alte Schema nach der Migration nicht rückwärtskompatibel ist.
6. **Komponenten-Cutover-Gate:** Phase 0a sowie die Phase-3a-/Phase-7-Matrizen
   müssen mit tatsächlichen Store-/Host-/Runner-Verbrauchern bestanden sein.
   Zusätzlich vor Worker-/Turn-Ingress-Freigabe aktive Kombinationen, Evidence-
   Provenienz, genaue Version-/Artefaktreferenzen, tatsächliche Bindings und
   stabile Live-Policy-Mappings prüfen. Fehlende erforderliche Approval, typisierte
   Runtime, Artefakte oder sichere Retry-/Recoverypersistenz verhindert normalen
   Recipe-Cutover; keine Label-Umdeutung oder ältere Komponenten als Ersatz.
   Authentifizierte Diagnose/Verwaltung und explizite Draft-Validierung bleiben
   gemäß ihren technischen Verträgen möglich. Ein Statusupdate im Plan oder
   ein erfolgreiches Seeding ersetzt weder die Abnahme noch das Gate.

7. **Teilnehmer vollständig in dasselbe Cutover aufnehmen.** Vor Beginn die
   tatsächlichen Komponenten-/Prefix-/Provider-/Source-/Compiler-/Review-Teilnehmer
   aus Phase 6 item 10
   und ihre abhängigen Aufträge im bestehenden Upgrade-Inventar erfassen. Ihre
   Migrationen, Prüfungen und Restorebelege gehören zum selben Backup-/Schema-/
   Binary-Cutover; keine nachgelagerte Prefix-Sondermigration nach geöffnetem
   Ingress. Der gemeinsame Koordinator ruft bestehende fachliche Ports/Prüfer auf,
   ohne Recipe-Sequenzierung oder einen zusätzlichen Plugin-/Jobrunner zu erfinden.
   Fehlende Pflichtartefakte/Referenzen für sichere Ausführung verhindern die
   betroffene Freigabe; Diagnose/Verwaltung bleiben nach Phase 3a möglich.
   Runtime-Ready verlangt keinen frisch generierten Serving-Prefix. Modellconsumer
   werden zusätzlich nach §8.2 und Prefix Gate A1/C2 freigegeben; kein alter
   inkompatibler Prefix oder Providerfallback als vermeintlich erfolgreicher Boot.
   Gemeinsame Abnahme enthält unterbrochenes Upgrade/Restore, alte behaltene
   Taskreferenzen und getrennte Auswahl- gegenüber Binary-/DB-Rollbacks. Ein
   Prefix-Auswahlrollback ändert weder Schema noch Orchestratorgeneration.
   Validator-Seed-/Routenwechsel und Evidenzreader werden in demselben Cutover
   geprüft: alte genehmigte Review-Tasks behalten ihre Auswahl, neue Tasks nutzen
   nur qualifizierte Routen. Physischer Restore und Runtime-Sicherheit gehören
   diesem Owner; semantische Approval-/Bootstrap-Provenienz und exact-view Q2
   bleiben beim Validator. Kein zweiter Erfolgswriter und keine Ausnahme für
   unvollständig migrierte aktive Validatoren.
   Composition-Familien liefern Seed-/Alias-/Command-/Consumerwechsel und ihre
   genauen Restoreprüfungen an denselben Koordinator. Familienweise qualifizierte
   Freigabe bleibt möglich; eine neue Familie darf weder eine ungeprüfte Nachbar-
   familie aktivieren noch unveränderte gemeinsame Infrastruktur erneut aufbauen.

## 5. Globale Abnahmekriterien

- Es gibt eine BrassClaw-Produktdistribution ohne Edition-/Deployment-Auswahl.
- Die Funktionsliste aus Phase 0 ist vollständig abgenommen: jede gewünschte Fähigkeit besitzt erreichbare UI/API-/Produktpfade, Komponenten-/Recipe-Abdeckung und einen erfolgreichen Produktionspfadtest. Entfernte Altoberflächen sind begründet; Löschen ersetzt keine gewünschte Funktion.
- Monty startet einmal beim Instanzboot, wartet schon vor der ersten Chatnachricht auf Arbeit und bleibt im Hintergrund bis zum Shutdown aktiv; keine globale VM pro Chat/Turn und keine Idle-LLM-Kosten. Readiness, Vorgangskontexte, Cancel-/Auth-/Child-Fortsetzungen, Budgettrennung und Crash-Reconciliation erfüllen Phase 3a.
- Ein zentraler Default-Sicherheitsvertrag wird pro Instanz angewendet; kein Profil kann pro User, Projekt, Thread oder Run gewechselt werden.
- Ein gültiger WebUI-Betreiber-Token ermöglicht Zugriff auf sämtliche implementierten Betreiberfunktionen; ohne gültige Authentifizierung gibt es keinen Zugriff.
- Sandbox, Secret-Schutz, Netzwerk, globale Tool Permissions und Audit besitzen je einen eigenen vollständigen Settings-Bereich. Es gibt keine zusätzlichen operationsbezogenen Toolfreigaben. Jede Änderung folgt ihrer dokumentierten Übernahmeregel; UI/API unterscheiden Ziel- und wirksame Revision.
- Konfigurationsrücklesen entspricht dem gespeicherten/validierten Effektivwert; es gibt keine stillen Überschreibungen durch Env, DB oder Run-Profil.
- Monty 1.0.0 ist nach dem Upgrade-Gate in Phase 3a integriert; sämtliche unterstützten Monty-Ressourcenlimits sind im WebUI verwaltbar und gültige Änderungen werden ohne Neustart wirksam. Die Disposition des entfernten upstream-Allokationslimits ist umgesetzt, Bestandswerte sind ausdrücklich migriert und der Allocator-Backstop ist nachweislich wirksam. Speicher wird standardmäßig einmalig beim Start konservativ dimensioniert und bleibt ohne periodische Druckmessungen konstant. Manuelle Live-Grenzen und ausdrücklich optionale langsame Automatik sind einstellbar; milder Druck pausiert keine Aufnahme. Tests belegen erhaltenen Taskverbrauch, gemeinsame Live-Heaprechnung, bedarfsgerechtes Wachstum, sichere Druckentlastung, Messausfallverhalten und begrenzte Reaktionszeit; nicht einhaltbare manuelle Senkungen werden ausdrücklich abgelehnt, automatische Senkungen als ausstehend angezeigt. Taskfehler und globale Dienstfehler haben getrennte Recovery.
- Tokenbudgets sind standardmäßig ausgeschaltet. Kein Retrieval-, Vorwissens-, History- oder Taskpfad erzwingt dann künstliche Tokenlimits; Tokenzählung bleibt zur Beobachtung erhalten und technische Modellgrenzen werden berücksichtigt.
- Externe Kanal-Absender erben niemals Betreiberrechte aus dem Browser-Token.
- Projekt-/Thread-IDs sind keine Autorisierungsgrenzen, bleiben aber als Datenreferenzen konsistent.
- Codeausführung, Netzwerk, Secrets und irreversible externe Aktionen laufen weiterhin durch eine explizit getestete zentrale Kontrollstrecke.
- Alle sichtbaren v2-UI-Aktionen besitzen echte Endpunkte oder wurden aus dem UI entfernt; keine TODO-Platzhalter werden als Funktion ausgeliefert.
- Migration erhält Daten und Secrets oder verlangt eine klare, sichere Betreiberaktion; es gibt kein stilles permissives Fallback.
- Architektur-, Integration-, UI-, E2E- und Upgrade-Tests belegen diese Eigenschaften.
- Die Recipe-Matrix aus Phase 7 besteht: Tier 0 ohne LLM, Tier 1 mit vorgesehenen LLM-Schritten, echter No-Match als Tier 2, Sempai/Q1/menschliches Q2, Step-Isolation, Komponentenrevisionen und kein Replay nach Schrittfehlern.
- Phase 0a ist umgesetzt und abgenommen: exakte Assoziations-/Approvalverträge,
  vertrauenswürdige authored/system_seed-Provenienz, vollständige immutable
  Workflow-/Toolartefaktselektion, typisierte rekursive Bindings, geprüfte
  ToolSkill-Vorbereitung und präzise persistente Retry-/Effektverträge. Kein
  stiller Downgrade oder Tier-2-Fallback bei Assemblyfehlern. Schemaentwurf,
  Labels und einzelne Komponentenapprovals ersetzen keine Kombinationsevidenz.
- Globale Toolpolicy wirkt auf alle behaltenen Versionen/Dispatchaliase derselben
  stabilen Toolidentität; fehlende/konfligierende Zuordnung oder aktuelle Policy
  verweigert Dispatch. Crash/Wait/Reclaim erhält Invocation-Zähler, ausgewählte
  Implementierungen und bestätigte/ungeklärte Effekte. Produktionsnachweise aus
  Phase 7 und das Komponenten-Cutover-Gate aus Phase 8 sind Voraussetzung.
- Intents sind zentral und an ihren Komponenten/Varianten verwaltbar; Matching-Vorschau ist nebenwirkungsfrei, aktiver Bestand und historische Auswahl bleiben konsistent.

### 5.1 Leistungsabnahme

Baseline und Vergleich verwenden identische Hardware, Komponenten-/Intentdaten, Lastprofile und Warm-/Cold-Bedingungen. Mehrere Wiederholungen, Streuung und Messgrenzen dokumentieren; echte Provider-/Netzwerklatenz getrennt von lokaler Orchestrierung messen. Ein vereinfachter Codepfad allein gilt nicht als Geschwindigkeitsnachweis.

| Messung | Ziel / Abnahme |
|---|---|
| Tier-0-Kosten | Null LLM-Aufrufe und null LLM-Tokens im Task; kein versteckter LLM-Fallback nach Matching-/Schrittfehler. |
| Warme Tasks | Lokale p50/p95-Latenz und Durchsatz mindestens ohne belastbare Regression. In Phase 0 eine Messstreuungstoleranz festlegen; zunächst 5 % als zu überprüfenden Grenzwert verwenden. |
| Tatsächliche Beschleunigung | Mindestens ein repräsentatives Lastprofil verbessert lokale Tasklatenz oder Durchsatz messbar außerhalb der Streuung. Zielwert und Profil vor Änderung festlegen; zunächst 10 % als Ziel untersuchen. Ergebnis samt möglichen Tradeoffs offen dokumentieren. Ohne Nachweis keine pauschale Aussage „v3 ist schneller“. |
| Boot / erster Vorgang | Cold-Boot bis Ready und erstes Recipe getrennt vergleichen. Keine globale VM-Erzeugung beim ersten Chat; zusätzlicher Bootaufwand muss sichtbar sein. |
| Fairness / Live-Control | Verbindliche maximale VM-Zeitscheibe sowie Cancel-/Settings-Reaktionszeit in Phase 0/3a festlegen; auch unter Workqueue-Last und langer Python-Schleife einhalten. Ohne konkrete Grenzwerte kein Produktionsnachweis. |
| SQL / Komposition | Matching-/IBS-Zeit, Roundtrips und Programmausführungen messen. Caches nur mit nachgewiesener Revisions-/Variablen-/Scopekorrektheit einsetzen; vorhandene IBS-Memoisierung wiederverwenden. |
| Idle / Speicher | Keine Orchestrator-Polling- oder Idle-LLM-Aufrufe; Idle-CPU und Langzeit-RAM messen. Freigegebene Taskdaten verschwinden aus begrenzten Caches; kumulativer Verbrauch früherer Tasks blockiert neue Arbeit nicht. Adaptive Speicherbudgets unter reichlich RAM und konkurrierender Fremdlast vergleichen: Wachstum folgt Bedarf und verfügbarer Kapazität, Reserve bleibt erhalten, Druckentlastung und Messung verursachen keine erhebliche Latenz-/Idle-CPU-Regression. Heap, Prozess-RAM und Systemdruck getrennt messen. |
| Tier 1 / Tier 2 | Vorgesehene LLM-Aufrufe und Prefixgeneration korrekt; Providerwartezeit separat ausweisen. Prefix-/Komponentenänderungen erreichen folgende Tasks ohne veraltete Cachetreffer. |

Jede Regression wird vor Cutover erklärt und behoben oder als konkrete, begründete Architekturabwägung dokumentiert. Benchmarkergebnisse sind derzeit offen; dieser Plan behauptet weder erreichte Zielwerte noch vollständige Laufzeitabnahme.

## 6. Risiken und bewusste Grenzen

- **Token als Vollzugang:** Tokenverlust kompromittiert die gesamte Instanz. Deshalb sind sichere Speicherung, Rotation, lokale Standardbindung, Fernzugriffsschutz, CSRF/XSS-Abwehr und Audit Pflichtbestandteile des Zielmodells.
- **Ein Betreiber heißt nicht ein vertrauenswürdiger Agent:** Modell- und Extension-Code sind nicht automatisch vertrauenswürdig, nur weil ein Betreiber die WebUI nutzt. Capability-Gates und Ausführungsisolation betreffen Codeherkunft und Seiteneffekte und bleiben bestehen.
- **Mehrbenutzerdaten in vorhandenen Instanzen:** das Zusammenführen kann Eigentums- und Namenskollisionen erzeugen. Die Migration muss konservativ sein und bei Unklarheit anhalten.
- **ID-Nutzung ist nicht gleich Scope-Autorisierung:** zu aggressive Entfernung kann Foreign Keys, Event-Replay, Idempotenz, Secret-AAD oder Kanal-Zuordnungen brechen.
- **„Vollversion“ ist Produktvollständigkeit:** alle festgelegten Funktionen sind erreichbar. Globale Toolzulassung ersetzt Einzelfreigaben; technische Host-, Shell-, Netzwerk- und Secretgrenzen bleiben gemäß Instanzkonfiguration bestehen.
- **Sicherheitsrichtlinie im WebUI:** Änderungen an einer zentralen Instanzrichtlinie haben große Reichweite. UI muss Auswirkungen verständlich machen, Änderungen auditieren und riskante Einstellungen explizit bestätigen.
- **Globaler Orchestrator als gemeinsamer Fehlerbereich:** unadressierter Stop, kumulatives Taskbudget, gemeinsamer Verlauf, blockierender Parent oder lange Python-Schleife können die Instanz treffen. Kontexttrennung, faire Zeitscheiben, Control-Kapazität, Taskbudgets und globale Heap-Recovery sind Pflichtnachweise. Ein globaler Speicherfehler ist kein garantiert isolierter Taskfehler.

## 7. Geprüfte Orientierung

- `README.md`, `CONTRIBUTING.md`, Root `AGENTS.md`, `CLAUDE.md`, `crates/README.md`, `crates/Architecture.md`.
- `docs/reborn/contracts/{runtime-profiles,runtime-selection,capability-access,kernel-boundary,host-api,auth-product,settings-config,secrets,filesystem,network,approvals,turn-persistence}.md`.
- CLI-/Composition-/WebUI-Ingress- und Host-API-Agent-Verträge.
- Workspace-Manifeste, `src/main.rs`, Reborn CLI/Composition, zentrale Policy-/Scope-Dateien, Testverzeichnisse, E2E- und CI-Konfiguration.
- Ergänzung 2026-10-06: Root-Architekturabschnitte, `engine/CLAUDE.md`, C.6-Produktionsdriver-Pläne, `persistent_monty_driver.rs`, `session_registry.rs`, `orchestrator.rs`, `basic_mode.py`, Boot-/Shutdown-Komposition, Runner-Abbruch und AcceptedMessageRef-Verträge wurden für Phase 3a statisch gegengeprüft. Root-Dokumente definieren jetzt das globale Ziel; ältere Per-Conversation-Beschreibungen müssen beim Implementieren ersetzt werden. Keine Garantie der Fehlerfreiheit ohne die geforderten Laufzeittests.

**Repositoryzustand bei Auditbeginn:** `git status` zeigte bereits `M .DS_Store`; diese Änderung wurde nicht verändert.

**Planrevision 2026-10-06:** Die Einzelprüfung führte zur Bereinigung der Hauptschritte auf globale Toolregeln, zum Recipe-/Fehlervertrag, zur Trennung von Taskbudgets und globalem Heap, zur Revisions-/Cache-/Migrationsabnahme und zu messbaren Leistungszielen. Dies ist eine Planänderung, keine neue Implementierungs- oder Testerfolgsmeldung.

## 8. Ergänzung — Intents im WebUI verwalten und Matching nachvollziehbar machen

**Ziel:** Betreiber können Intents zentral und direkt an den Komponenten bzw. Recipe-Varianten einsehen und bearbeiten. Die tatsächlich wirksame Zuordnung zu einer Komponente und bei Recipes zu deren Variante bleibt nachvollziehbar. Diese Ergänzung gehört zur WebUI-Umsetzung in Phase 5 und zur Abnahme in Phase 7; sie führt keinen zweiten Matching- oder Recipe-Loop ein.

### 8.1 Vorhandene Funktionen und offene Lücken

| Bereich | Aktueller Stand der statischen Codeprüfung |
|---|---|
| Einstellungen → Recipes → Recipe aufklappen | Zeigt hinterlegte Intent-Beispiele, auch bei Varianten; nur lesbare Anzeige. |
| Einstellungen → Validation Queue → Intent template preview | Prüft `%`-Templates lokal und zeigt Hinweise; speichert nichts und testet keine tatsächliche Recipe-Auswahl. |
| Backend | `GET` und `PUT /api/settings/intent-inputs` sowie `DELETE /api/settings/intent-inputs/{class_code}/{component_id}` sind vorhanden und bei PostgreSQL + skills-db mit dem Store verdrahtet. |
| Verwaltungsoberfläche | Die UI-Anbindung für Auflisten, Hinzufügen, Ändern und Löschen fehlt. |

Belege: `crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js` dokumentiert die noch ungenutzten CRUD-Funktionen; `components/component-detail-pane.js` zeigt die Beispiele an; `components/intent-template-preview-panel.js` bietet ausschließlich lokale Vorschau. Die Backend-Verdrahtung liegt in `crates/brassclaw_reborn_composition/src/webui.rs`, der Store in `pg_intent_inputs_store.rs`.

Intent-Beispiele werden als `intent_examples` direkt an Komponenten gespeichert; Recipes besitzen zusätzlich Beispiele je Eintrag in `variants`, zusammen mit dessen `step_link`. `reborn_intent_inputs` enthält den tatsächlichen Matching-Bestand mit Ziel-Komponenten-ID, Klasse, Score, Herkunft und Review-Status. Dort können auch gelernte Ausdrücke liegen. Die Oberfläche muss hinterlegte Beispiele und aktive Matching-Einträge unterscheidbar anzeigen und ihre Zuordnung konsistent halten; gelernte Einträge dürfen nicht unbemerkt verloren gehen.

Vor der vollständigen UI-Anbindung diese Backend-Lücken schließen:

- **Recipe-Varianten:** Der generische Upsert übergibt `step_link = None`. Da der Seeder bei Konflikten `step_link` überschreibt, kann ein vorhandener Variantenverweis verloren gehen. Den Schreibvertrag um eine eindeutige Variantenzuordnung ergänzen und den zugehörigen `step_link` serverseitig aus der gültigen Recipe-Variante bestimmen bzw. erhalten.
- **Variantenkonflikte:** Der bestehende Unique-Key enthält keine Variantenkennung. Derselbe Ausdruck für mehrere Varianten desselben Recipe kann den Variantenverweis überschreiben. Im Editor Konflikte vor dem Schreiben erkennen; echte alternative Zuordnungen nur über einen ausdrücklichen, versionierten Mehrdeutigkeits-/Variantenvertrag speichern. Kein stilles Last-write-wins-Seeding. Der Migrationsentwurf regelt zulässige Mehrfachzuordnung und erhält bestehende Verweise.
- **Einzelne Intents:** Der Schreibvertrag adressiert keine bestehende Intent-ID; geänderter Text wird damit nicht gezielt als Änderung derselben Zeile behandelt. Bearbeitung und Löschen einzelner Einträge ermöglichen. Der vorhandene komponentenweite Purge bleibt eine ausdrücklich getrennte Operation.
- **Vollständige Liste:** Der Store begrenzt die Ausgabe derzeit auf 500 Einträge ohne Seitennavigation. Pagination und Such-/Filtermöglichkeiten ergänzen, damit sämtliche Einträge erreichbar sind.

### 8.2 Vorgeschlagene WebUI-Funktionen

1. **Eigener Tab „Intents“ im Component Catalog:** durchsuchbare, paginierte Liste mit Ausdruck, Ziel-Komponente bzw. Recipe, Recipe-Variante, Herkunft, Score und Review-/Aktivierungsstatus. Ziel und Variante sind zur jeweiligen Detailansicht verlinkt. Die Oberfläche folgt dem instanzweiten Betreibermodell ohne neue User-/Projekt-Berechtigungsauswahl.
2. **Intent-Editor direkt an Komponenten und Recipe-Varianten:** hinterlegte Beispiele und Matching-Einträge anzeigen; einzeln hinzufügen, ändern und löschen. `%`-Vorschau wiederverwenden. Zielvariante und Konflikte explizit darstellen; andere Komponententypen benötigen keine fingierte Variante. Stabile Intent-ID/Revision für Änderung und Löschung verwenden.
3. **Matching-Testfeld:** dieselbe Auswahl-/Variablenlogik wie im Produktpfad nutzen, mit ausdrücklich schreibfreier Ausführung. Recipe/Variante, Variablen, Mehrdeutigkeit, No-Match und technische Fehler anzeigen. Keine Tool-/Recipe-/LLM-Ausführung und keine Änderung von Scores, Lern-/Disambiguierungsdaten. Vorschau eines Entwurfs klar von aktiver Auswahl unterscheiden.
4. **Konsistente Revisionen und Validierung:** Entwurf, Q1/Q2-Status und aktive Komponentenrevision trennen. Alle authored-Änderungen, einschließlich Änderungen bisheriger Builtins, benötigen automatisches Q1, menschliches Q2, Verhaltensnachweise und die passende exakte Kombinationsapproval vor Aktivierung; trusted system_seed bleibt der gesonderte kontrollierte Bootstrapvertrag aus Phase 0a; Bearbeitung überschreibt keine noch von Vorgängen verwendete Revision. Aktivierung von Komponenten-/Variantenbeispielen und Matching-Einträgen transaktional, revisionsgeprüft und ohne Teilzustände durchführen. Gelernte Einträge mit Herkunft erhalten. Bestehende IBS-/Komponenten-/Matching-Caches nach Revision aktualisieren; Kohai-Prefix aus validierten Komponenten erneuern. Aktive Prefix-/Cachegeneration und ausstehende Erneuerung sichtbar machen; folgende Tasks erhalten einen konsistenten aktiven Stand.

   **Katalogaktivierung und Prefixbereitschaft ohne Kreisabhängigkeit:**
   Eine Kataloggeneration umfasst konsistente genehmigte Komponenten, Matching,
   Bindings, Approval und ausführbare native Artefakte. Deren Aktivierung benötigt
   keinen erst aus dieser Generation zu erzeugenden Modell-Prefix. In derselben
   vorhandenen Aktivierungs-/Invalidierungsgrenze die betroffenen Prefix-Abhängigkeiten
   revisionsgeprüft als erneuerungsbedürftig festhalten; vorhandene dauerhafte
   Änderungszustellung oder wiederaufnehmbare Reconciliation verwenden, keine
   zweite Aktivierungsregistry. Scheitert die Katalogtransaktion, bleibt die alte
   Kataloggeneration aktiv. Nach erfolgreichem Commit können qualifizierte
   modellfreie Builds die neue aktive Auswahl als vollständigen Snapshot lesen.

   Der Prefix-Owner baut/validiert daraus seine unveränderliche Generation nach
   [Prefix §7](docs/plans/prefix_v3_upgrade.md#7-persistence-and-generation-lifecycle).
   Vor Auswahl/Serving exakt Quelle/Katalog, Design, Provider-/Modelldaten und
   Frische gegen den dann gewünschten Stand prüfen. Ein zwischenzeitlich überholter
   erfolgreicher Build bleibt historisches Artefakt und wird nicht automatisch
   ausgewählt. Prefix-Publikation, Designannahme und Consumeraktivierung bleiben
   die getrennten Produktoperationen des Prefix-Plans; sie schalten keine
   nicht genehmigten Komponenten frei.

   Neue Aufgaben, die einen betroffenen Prefix benötigen, bleiben vor ihren
   abhängigen Effekten explizit wartend/blockiert, bis die passende Generation und
   deren Consumer-Gates bestanden sind. Kein unbemerkter Rückgriff auf alte
   Komponenten oder einen veralteten Prefix, keine Vermischung von Snapshots und
   kein Tier-2-Fallback. Verwaltung, unabhängige Tier-0-Recipes und der modellfreie
   Build dürfen weiterarbeiten. Laufende Tasks behalten ihre ursprünglichen
   kompatiblen Auswahlen, vorbehaltlich aktueller Policy, Providerverfügbarkeit,
   Widerruf und technischen Grenzen; bei fehlender Sicherheit blockieren statt
   umzubinden oder Effekte zu wiederholen. Globale Toolregeln bleiben live.

   Abnahme: Aktivierung bei fehlendem Prefix/Provider, neuer Katalog während
   laufendem Build, verlorenes Invalidierungsereignis/Neustart und fehlgeschlagener
   Neubau. Kein zirkuläres Warten, alte Artefakte/Tasks erhalten und kein neuer
   Modellrequest mit nicht passendem Prefix. Modellfreie Arbeit bleibt möglich.

   **Systemkomponenten:** geprüfte Seeds und Checksums erhalten. Betreiberänderungen als explizite eigene Revision/Kopie mit definiertem Override und Q1/Q2 behandeln; keine stillen Body-/Checksummutationen. Boot/Repair darf Overrides und deaktivierte Intent-Zuordnungen nicht reaktivieren oder überschreiben. Persistente Override-/Deaktivierungsregeln und Verhalten bei Seed-Upgrades vor Implementierung festlegen und testen. Geschützte Root-Orchestratorrevisionen wechseln weiterhin nur nach kontrollierter Reconciliation; gewöhnliche Intents/Recipes werden ohne Dienstneustart aktiv.
5. **Auswahl im Chat-Verlauf sichtbar machen:** für den konkreten Vorgang die tatsächlich verwendete Komponente, das Recipe, die Variante und Tier 0/1 bzw. den No-Match-/Tier-2-Pfad anzeigen. Die Auswahl aus dem ausgeführten Vorgang übernehmen, nicht nachträglich mit dem aktuellen Intent-Bestand neu berechnen. Ein Recipe kann LLM-Schritte enthalten: Nur Tier 0 bedeutet null LLM-Aufrufe; Tier 1 führt die vorgesehenen LLM-Schritte aus.

### 8.3 Abnahme

- Alle Intents sind über die paginierte Liste und ihre Komponenten-/Variantenzuordnung einsehbar; auch Bestände mit mehr als 500 Einträgen bleiben vollständig erreichbar.
- Hinzufügen, Ändern, Löschen und Variantenwechsel eines einzelnen Beispiels aktualisieren Komponenteninhalt und Matching-Bestand konsistent; kein veralteter Ausdruck oder verlorener `step_link` bleibt zurück. Fehlgeschlagene Änderungen erzeugen keine Teilzustände.
- Mehrfachzuordnungen desselben Ausdrucks, konkurrierende Editoränderungen und Draft-/Aktivierungsfehler erzeugen keine still überschriebenen Varianten. Auswahl verwendet nur den vorgesehenen aktiven validierten Stand.
- Der Matching-Test zeigt die gleiche Auswahl und Variablenauflösung wie der produktive Pfad, ohne Tools auszuführen oder produktive Matching-/Lerndaten zu verändern.
- Review-pflichtige Änderungen werden erst nach dem erforderlichen Validierungsweg aktiv; Q2 bleibt menschlich. Freigegebene Änderungen wirken ohne Neustart auf folgende Matching-Vorgänge; benötigte neue Modell-Prefixe durchlaufen die getrennte §8.2-Bereitschaftsprüfung, ohne alten Prefixinhalt als passend auszugeben.
- Der Chat-Verlauf zeigt die tatsächlich verwendete Auswahl auch dann korrekt, wenn Intents oder Recipes später bearbeitet werden. Tier-0-Recipes laufen ohne LLM; Tier-1-Recipes führen ihre vorgesehenen LLM-Schritte aus.
- Bestehende validierte Komponenten, Recipe-Varianten, UUID-Verweise und gelernte Intent-Einträge bleiben beim Umbau erhalten; Betreiberänderungen überstehen Boot/Repair gemäß der festgelegten Systemkomponentenregel.
- Ein suspendierter Vorgang verwendet seine bisherigen Komponentenrevisionen, ein folgender Vorgang den neuen aktiven Matching-/Komponentenstand mit dazu passendem Prefix, sofern benötigt; sonst wartet/blockiert nur der betroffene Verbraucher gemäß §8.2. Toolberechtigungen folgen unabhängig davon stets der aktuellen globalen Einstellung.

## 9. Verbindliche Klarstellung — Vollständige Betreiberverwaltung und globale Tool-Berechtigungen

**Betreiberzugang:** Ein mit dem gültigen Instanz-Token authentifizierter Nutzer kann sämtliche angebotenen Funktionen, Daten und Einstellungen der Instanz verwalten. Dafür gibt es keine zusätzliche Prüfung von Benutzer-, Mandanten-, Projekt- oder Funktionsrollen. Token-Authentifizierung und die bestehenden technischen Schutzmaßnahmen für den Verwaltungszugang bleiben erhalten.

**Tool-Berechtigungen:** Tools werden im WebUI global für die Instanz zugelassen oder gesperrt und mit ihren unterstützten Parametern konfiguriert. Die wirksame globale Einstellung ist die Berechtigungsentscheidung für alle Recipes und Aufgaben. Zusätzliche Tool-Freigaben pro Operation, Aufruf, Run oder Ausführungsversuch sind nicht Bestandteil des Zielmodells; dafür werden keine Invocation-Fingerprint-Freigaben oder aufgabengebundenen Approval-Leases benötigt. Vor jedem tatsächlichen Tool-Dispatch setzt der Kernel die aktuelle globale Tool-Einstellung und die zugehörigen technischen Regeln durch. Ein Recipe oder LLM kann diese Einstellung nicht durch eigene Ablaufentscheidungen überschreiben.

**Recipe-Ausführung:** Die zwei Schritte bleiben unverändert: `channel:"rust"` bindet den ToolSkill, `channel:"orchestrator"` führt den PythonCode-Aufruf `host.<tool>(...)` aus. Das Binden ist keine zusätzliche Berechtigungsvergabe. Ein global gesperrtes Tool wird auch durch einen bereits vorhandenen Recipe-/ToolSkill-Verweis nicht ausführbar. Globale Toolregeln gelten unabhängig davon, ob ein Vorgang Tier 0, Tier 1 oder Tier 2 verwendet.

**Vorgangskontexte:** Conversation-, Message-, Run-, Aufruf- und Attempt-Kennungen bleiben für richtige Eingabezuordnung, History, Antworten, Fortsetzungen, Abbruch, Idempotenz und Audit erhalten. Sie begründen keine eigenen Tool-Berechtigungen. Veraltete oder abgebrochene Versuche dürfen weiterhin keine neuen Effekte auslösen; diese Ausführungskorrektheit ist von der globalen Tool-Zulassung getrennt.

**Einheitlicher Vertrag:** Die Hauptschritte sind auf diese Festlegung ausgerichtet. Ältere Crate-Dokumente, Komponentenbeschreibungen und Implementierungsnotizen mit operationsbezogenen Toolfreigaben sind bei der Umsetzung gemäß Phase 7 abzulösen; historische Testergebnisse bleiben erhalten. Keine alten Einzelfreigaben als versteckte Schicht fortführen. Worker-Claims, externe Dienstauthentifizierung, authored-Q1/Q2 mit menschlichem Q2, der getrennte trusted-system_seed-Evidenzvertrag und technische Sandbox-/Netzwerk-/Secret-/Ressourcenregeln bleiben eigene Verträge. Externe Kanal-Absender erhalten keine Betreiber-Verwaltungsidentität.

**Abnahme:** Ein gültiger Betreiber-Token erschließt die gesamte Verwaltungsoberfläche ohne Rollen-/Scope-Auswahl. Globale Tooländerungen wirken ohne Neustart auf folgende Dispatches, auch innerhalb bereits laufender Recipes. Ein gesperrtes Tool erzeugt keine neuen Effekte; ein zugelassenes Tool benötigt keine zusätzliche operationsbezogene Freigabe. Bereits laufende Toolaufrufe werden nach ihrem tatsächlichen Status behandelt und nicht durch erneuten Dispatch wiederholt. Tests belegen globale Zulassung/Sperrung, aktuelle Einstellungen beim Dispatch, fehlende zusätzliche Approval-Schichten sowie weiterhin korrekte Vorgangs-/Attempt-Zuordnung.


## 10. Ergänzung — Provider ausschließlich in PostgreSQL

**Verbindliches Ziel:** Alle LLM-Providerdefinitionen liegen von Beginn an ausschließlich in `brassclaw_llm_providers`. PostgreSQL ist die einzige Katalog- und Konfigurationsquelle für Boot, CLI, WebUI, Runtime und Live-Reload. Kein eingebetteter JSON-Katalog, Dateioverlay, automatischer Rust-Providerseeder oder Dateifallback. Bestehende LLM-/Login-/Embedding-Fähigkeiten erhalten; ausführbare Protokolladapter und Authimplementierungen bleiben Infrastruktur, ihre Anbieterdefinitionen sind DB-Daten.

**Gefundene weitere Listen und Verbraucher:**

- `providers.json` im Repository ist die vollständige mitgelieferte Definitionsliste; `brassclaw_llm/src/registry.rs::builtin_provider_definitions` bettet sie per `include_str!` ein. Registry-Dateilader kombinieren sie mit einem optionalen Benutzerkatalog.
- `webui.rs::seed_builtin_providers` schreibt bzw. aktualisiert diese Definitionen bei WebUI-/Servicestart. Das ist derselbe Katalog, keine unabhängige zweite Providerliste.
- **V047** enthält eine zusätzliche hartcodierte Liste bekannter Provider-IDs und markiert vorhandene Zeilen als `is_builtin`; sie legt keine Definitionen an. **V048** ist nur ein SQL-Marker (`SELECT 1`) für den Rust-Seeder, keine weitere Definitionsliste.
- `llm_catalog.rs`, `brassclaw_llm/src/resolution.rs` und CLI-Boot verwenden Datei-/Env-Auflösung; `provider_admin.rs` bietet zusätzlich einen eingebetteten Katalogpfad. Auch diese Verbraucher vollständig inventarisieren und auf den DB-Katalog umstellen.
- `migration.rs::step4_migrate_providers` importiert eine Benutzerdatei nach PostgreSQL. Frontend-, Modelllisten-, Provider-Setup-/Login- und Embeddingpfade auf zusätzliche hartcodierte Anbieterdefaults und Dateifallbacks prüfen; Protokollunterstützung ist von Katalogdaten zu unterscheiden.

### 10.1 Umsetzung und Entfernung

1. **DB-Katalog vor erster Nutzung verfügbar machen.** Vor Providerauflösung und vor Monty-/Worker-/Ingress-Freigabe Schema und erforderliche DB-Daten bereitstellen. Für frische Instanzen benötigte Anfangsdefinitionen als versionierte DB-Dateninitialisierung festlegen, nicht als neue Rust-/JSON-Parallelregistry. Einmalige Initialisierung läuft vor Katalogverbrauchern; alle dabei angelegten Provider sind vom Betreiber ausblendbar/deaktivierbar und später wieder hinzufügbar; keine Herkunftssperre. Spätere Starts lesen ausschließlich DB-Zeilen. Keine Start-Synchronisierung gegen eine zweite Anbieterquelle. Ist kein Provider konfiguriert, authentifizierte Betreiberverwaltung zur Einrichtung anbieten und LLM-bedürftige Vorgänge ausdrücklich als unkonfiguriert behandeln; Tier-0-Fähigkeiten ohne LLM bleiben verwendbar. Ein DB-Ausfall ist ein technischer Fehler, kein Anlass für Dateifallback.
2. **Alle Leser auf denselben DB-Vertrag umstellen.** CLI-Boot, CLI-Modelllisten, Runtime, WebUI, Providerverwaltung, Login-/Modellermittlung, Embeddings und Live-Reload verwenden den vorhandenen `PgProviderRepo` bzw. einen neutralen Port darauf. Eine in-memory Registry darf ausschließlich aus DB-Zeilen aufgebaut werden und muss Änderungen, Ausblendung/Deaktivierung und Wiederaktivierung revisionsgerecht übernehmen. Ausgeblendete/inaktive Provider sind im aktiven Katalog und in Runtime-Auswahlpfaden ausgeschlossen; die Verwaltungsfunktion zum erneuten Hinzufügen darf sie gezielt aus der DB lesen. Provider-ID, Protokoll, Modell, URL, Kontextfenster, Setupmetadaten und aktive Auswahl aus DB lesen. Env/Config dürfen keinen alternativen Providerkatalog oder versteckte Anbieter-/Modell-/URL-Overrides erzeugen. DB-Verbindungsbootstrap und Secret-Broker bleiben eigene Verträge; geheime Werte gehören nicht als Klartext in den Katalog.
3. **Datei- und Seedfunktionen entfernen.** `builtin_provider_definitions`, JSON-Einbettung, Registry-Dateilader/-Overlays, `$BRASSCLAW_REBORN_HOME/providers.json`- und `~/.brassclaw/providers.json`-Ladepfade, `seed_builtin_providers`, den wiederkehrenden `upsert_builtin`-Seedpfad, Dateikatalog-Fallbacks und dateibasierte Boot-/Env-Providerauflösung entfernen oder durch die DB-Ports ersetzen. Gemeinsame Provider-Build-/Protokollfunktionen weiterverwenden; kein zweiter LLM-Ausführungspfad. Live-Provideränderungen verwenden den bestehenden Reload-Vertrag mit sichtbarem Übernahmezustand.
4. **Bestände erhalten und alte Importfunktion ablösen.** Vor Entfernung des alten `step4_migrate_providers`-Pfads vorhandene DB-Definitionen, aktive Auswahlen, Modelle/URLs, Secretreferenzen und gegebenenfalls noch ausschließlich in Benutzerdateien liegende Konfiguration inventarisieren und gesichert nach DB überführen. Konflikte explizit melden; keine stillen Überschreibungen. Benutzerdateien sind danach keine aktive Quelle; ihre Löschung nur im ausdrücklichen, gesicherten Upgradeweg, nicht bei jedem Start. Der endgültige Produktpfad enthält keinen `providers.json`-Importer. Bestehende angewendete V047/V048 unverändert erhalten; Schema-/Datenänderungen über neue versionierte Migrationen durchführen, historische Migrationen weder löschen noch Checksummen ändern.
5. **Vollständige Betreiberverwaltung mit wiederherstellbarem Entfernen ermöglichen.** Alle Providerdefinitionen über WebUI/API hinzufügen, ändern, deaktivieren, ausblenden und wieder hinzufügen können. „Entfernen“ bedeutet Ausblenden und Deaktivieren (Soft Delete), keine physische Löschung der Providerdefinition. Die DB-Zeile mit stabiler Provider-ID bleibt erhalten; den bestehenden `deleted_at`-Mechanismus für die Ausblendung prüfen und einen eindeutigen Aktivitätsvertrag festlegen. Kein Provider erhält eine Herkunftssperre, auch kein initial angelegter oder bisher als `is_builtin` markierter Provider. Die bisherige `is_builtin`-Immutabilität und alle darauf beruhenden UI-/API-/Store-Entfernungssperren ablösen; Herkunft bleibt reine Metadaten. Aktive Auswahlreferenzen beim Entfernen transaktional auf eine ausdrücklich gewählte Alternative umstellen oder als unkonfiguriert kennzeichnen. Bereits laufende Aufrufe nach definiertem Abschluss-/Abbruchvertrag behandeln; folgende Aufrufe dürfen inaktive/ausgeblendete Provider nicht auswählen. Über „Provider hinzufügen“ entfernte Definitionen aus der DB anbieten und dieselbe Zeile revisionsgeprüft wieder sichtbar/aktiv setzen, statt Duplikate anzulegen. Vor Aktivierung Modelle/URLs, Adapterfähigkeit und gültige Secret-/Authreferenzen prüfen; fehlende Konfiguration sichtbar ergänzen lassen. Frühere aktive Auswahlen nicht stillschweigend wiederherstellen. Neustarts, Upgrades und Initialisierung dürfen entfernte Provider nicht automatisch reaktivieren; Wiederaufnahme erfolgt ausschließlich durch eine ausdrückliche Betreiberaktion.
6. **Repositorydatei und Begleitpfade löschen.** Nach Umstellung der Verbraucher `providers.json` aus dem Repository löschen. Docker-COPYs, Build-Einbettung, CI-Pfadfilter, Testfixtures, Architekturtests und Dokumentation aktualisieren. Keine Ersatzdatei oder neue konstante Anbieter-Definitionsliste im Rust-/Frontend-Code einführen. Historische Changelog-/Migrationsbeschreibungen als historische Belege erhalten; aktuelle Anleitungen beschreiben ausschließlich DB-Verwaltung.

7. **Wirksame Revisionen verlustfrei an Verbraucher übergeben.** Den vorhandenen
   DB-/Reload-/Settingsvertrag um die nachgewiesen fehlende Übergabe ergänzen:
   stabile Provider-/Modellidentität, exakte Definitions-/Policyrevision und
   gewünschter/wirksamer Übernahmezustand sind für CLI, Runtime, WebUI und die
   Prefix-Verbraucher aus derselben Autorität lesbar. Gleichzeitige Edits werden
   revisionsgeprüft, nicht still überschrieben. Änderung und deren dauerhafte
   Zustell-/Reconciliation-Referenz über die vorhandene Transaktionsgrenze
   verbinden; keine neue Prefix-Eventqueue oder parallele Providerregistry.
   Nachrichten dürfen doppelt/verspätet sein oder fehlen: Verbraucher gleichen
   ihre Revision beim Wiederverbinden/Neustart mit dem DB-Owner ab. Eventdaten
   allein autorisieren keinen Dispatch und ersetzen kein aktuelles Nachlesen.

   Deaktivierung und verschärfte wirksame technische Grenzen dürfen nicht auf
   einen langen Prefix-Build warten. Vor Modell-Dispatch Providerverfügbarkeit
   und aktuelle effektive Grenzen am gemeinsamen Boundary prüfen. Fehlt ein
   verlässlicher Aktualitätsnachweis, den betroffenen Aufruf blockieren statt
   alte Metadaten als Erlaubnis zu verwenden. Ein bereits begonnener Fremdaufruf
   wird nach seinem tatsächlichen Status abgewickelt, nicht automatisch wiederholt.

8. **Prefix-Anbindung im selben Providervertrag abnehmen.** Der Prefix-Owner
   konsumiert diese Revisionen für Antwort-, Sempai- und Designprovider getrennt,
   liefert die in §5.3 seines Plans definierten Modell-/Kapazitätsfelder und
   berechnet Frische/Kompatibilität seiner Generationen und Consumerbindungen.
   Diese Felder erweitern den bestehenden Provider-/Modellowner; unveränderliche
   Mess-/Designsnapshots sind Evidenz, keine zweite mutable Konfiguration.
   Dieser Plan implementiert Zustellung/Readback und technische Provideraufnahme;
   Prefix implementiert Rendering, Partitionmessung und den einen finalen
   Requestgrößenguard nach [Prefix §5.3](docs/plans/prefix_v3_upgrade.md#53-database-owned-model-specifications-and-model-specific-designs).
   Kein zweiter Budgetstore, keine neue Guard-Implementierung in diesem Plan.

   Verbindlicher [Prefix-/Paar-Designvertrag](docs/reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26):
   Design für Formate und Grenzen je Provider-/Modellpaar gemeinsam verwenden;
   Rollenwissen/Inhaltsregeln und fertige Prefixgenerationen getrennt halten.
   Generator, Kohai und gegebenenfalls Sempai nutzen für dasselbe Paar dieselbe
   gespeicherte, akzeptierte Designrevision. Jedes Prefix wird bei der Generierung
   für sein konkretes Zielpaar fertig formatiert und unveränderlich gespeichert.
   Recipe, Orchestrator oder Rolle bestimmen ein spezifisches Prefix;
   ohne solche Auswahl gilt base-prompt,
   daher normalerweise auch bei No-Match. Ein fehlendes/inkompatibles explizites
   Prefix oder ein ungeklärter Auswahlkonflikt blockiert den betroffenen Aufruf,
   statt heimlich base zu wählen. Keine generelle Pflicht zur Base-Komposition.
   Orchestrator-Prompt an Kohai → Kohai formatiert ausschließlich den dynamischen
   Prompt für das Zielpaar anhand des gespeicherten akzeptierten Designs → falls
   Sempai verbunden/aktiviert ist, optimiert/korrigiert er diesen bereits
   formatierten Prompt und gibt den finalen dynamischen Prompt an Kohai zurück
   → Kohai setzt das vorgesehene fertige Prefix zeichengetreu genau einmal davor
   → vollständigen Request prüfen → senden. Kein erneutes Formatieren durch Kohai
   nach Sempais Rückgabe. Für das Zusammenfügen gilt Sempais zurückgegebener finaler
   dynamischer Prompt, nicht der ursprüngliche oder unkorrigierte Zwischenstand.
   Ohne Sempai gilt Kohais bereits formatierter dynamischer Prompt.
   Sempais eigener Aufruf verwendet sein eigenes Prefix,
   das anhand des Designs seines Zielpaars generiert wurde. Generator, Kohai und
   Sempai verwenden bei demselben Provider-/Modellpaar dieselbe gespeicherte,
   akzeptierte Designrevision; die rollenbezogenen Prefixtexte bleiben getrennt.
   Sempais eigenes Prefix bleibt in dessen Review-Aufruf und wird nicht zum
   zurückgegebenen dynamischen Prompt. Die
   [explizite Übergabeabnahme](docs/reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests)
   ist beim Prefix-Owner umzusetzen und hier wiederzuverwenden.
   Manuelle Prefix-/Promptgrenzen sind in allen Ansichten über denselben bestehenden
   revisionsgeprüften Provider-Endpunkt verknüpft; historische Pins sind keine
   zweite editierbare Konfiguration. Providerfakten abrufen oder mit Herkunft
   manuell ergänzen; keine Änderungen an Provider-Host/Servertemplate/Service.
   Die Caller-Abnahme muss dieselben/verschiedene Paare, Standard-/explizite
   Auswahl, exakte Zeichenfolge, synchronisierte Edits und harte Gesamtgrenzen
   auch bei deaktiviertem Budget prüfen. Keine neue Pipeline/Registry in diesem Plan.

   Die [Generierungsdialoge](docs/reborn/model-token-limits.md#generation-prompts-missing-selection-and-model-use)
   bieten OK/Cancel bei fehlendem/inkompatiblem ausgewählten Prefix und nach jedem
   erfolgreichen Use die Base-Prompt-Generierung für das Paar. Vorhandene Job-/
   Readiness-/Waitgrenzen verwenden: keine zweite Queue, kein Modellcall vor Ready,
   keine Wiederholung des Originalinputs. Cancel nach Use lehnt Generierung ab,
   setzt aber die bereits gespeicherte Providerauswahl nicht heimlich zurück.

   Vor Consumer-Auswahl Änderungen seit dem Build prüfen, Prefix-/Design-/
   Targetreferenzen durch denselben vorhandenen Revisionsvertrag festhalten und
   den tatsächlichen Request am finalen Boundary erneut prüfen. Ein alter Task
   wird nicht still auf einen neuen Endpoint/Compiler/Prefix umgebunden; fehlende
   Kompatibilität oder deaktivierter Provider stoppt den betroffenen Dispatch.
   Bei gültigem Providerwechsel ohne passenden Prefix bleibt der gewünschte
   Zustand sichtbar blockiert; keine automatische Modellreparatur oder heimliche
   Wiederherstellung alter Auswahlen. Das Entfernen des letzten Providers lässt
   Verwaltung und qualifizierte providerfreie Aufgaben verwendbar.

   Gemeinsame Caller-Abnahme mit Prefix C2: unterschiedliche Antwort-/Sempai-/
   Designtargets, gleichzeitiger Edit und Dispatch, Tightening bei deaktiviertem
   Tokenbudget, verlorene/doppelte Notification und Neustart, Deaktivierung im
   Wait, letzter Provider sowie Wiederaufnahme ohne implizite Auswahl. Nachweise
   der gemeinsamen Infrastruktur wiederverwenden und nur die tatsächlichen
   Prefix-Consumerfälle ergänzen; keine zweite vollständige Testpipeline.

   Explizite modellgestützte Validatorchecks sind weitere Verbraucher desselben
   Vertrags. Validator wählt über genehmigte Recipes den genauen Model-/Target-
   und Kontextvertrag; diese Phase liefert Revision/Availability/Readback und
   der Prefix-Owner den finalen Größen-/Renderingguard. Keine neue Providerliste,
   Modelltransport-/Budget-/Retryimplementierung im Validator. Deaktivierung,
   fehlender passender Prefix, Outputfehler oder ungeklärte Modellwirkung halten
   den Review unvollständig; kein stiller Fallback und keine Kandidatenablehnung
   allein aus Infrastrukturproblemen. Abnahme verbindet solche Checks mit einem
   unabhängigen deterministischen Review und testet gleiche/verschiedene Targets,
   Live-Änderung im Wait und den verlorenen ursprünglichen Modellreply ohne Replay.

**Abnahme:** Frische und bestehende Instanzen verwenden vom ersten Providerzugriff an ausschließlich PostgreSQL. Tests laufen ohne `providers.json` und ohne Provider-Env-/Datei-Fallback: Boot, CLI und WebUI zeigen denselben Bestand; DB-only hinzugefügte Anbieter sind nutzbar; alle initialen, früheren Builtin- und benutzerangelegten Provider lassen sich über UI/API ausblenden/deaktivieren, einschließlich des letzten Providers. Ihre Definitionen bleiben in der DB und lassen sich über „Provider hinzufügen“ mit derselben ID ohne Duplikate wieder aktivieren. Inaktive/ausgeblendete Provider erscheinen nicht in aktiven Listen und werden nicht zur Ausführung ausgewählt. Das Entfernen aktiver Provider löst ihre Auswahlreferenzen konsistent auf; danach bleibt die Betreiberverwaltung erreichbar und LLM-bedürftige Vorgänge melden fehlende Konfiguration. Änderung, Ausblendung/Deaktivierung und ausdrückliches Wiederhinzufügen funktionieren revisionsgeprüft; Neustart/Upgrade erhalten Betreiberentscheidungen ohne automatische Reaktivierung. Wiederhinzufügen prüft Konfiguration und Authreferenzen und stellt alte aktive Auswahlen nicht ungefragt wieder her. Modell-/Auth-/Embeddingpfade sowie Secretauflösung funktionieren weiter. Fehlende Konfiguration und DB-Fehler sind unterscheidbar. Backup/Restore und Migration erhalten alle Provider-/Auswahl-/Secretreferenzen. Der Repository-Scan weist keine produktiven Dateikatalog-/Rust-Seeder-Verbraucher mehr nach; historische Migrationen bleiben unverändert.

## Offener Folgeauftrag K3-2 ab K4 — Provider-/Design-Ersteinrichtung und Prefix-Lebenszyklus

**New open follow-up; not implemented by this document patch.** Existing completed
sections and original acceptance criteria remain unchanged. This section carries
only the relevant future-consumer delta from [Exact K3-2](docs/plans/Exact_V3_definition_update_at_K3-2.md) and the
[shared selection/lifecycle contract](docs/reborn/model-token-limits.md#open-k3-2-successor--retained-prefix-selection-and-prefix-free-construction-workflows).
It does not reopen K0–K3, replace a current owner's work or create a parallel
implementation. Original sections are read references; no code activation is claimed.

- Root-/Child-/Reviewcapture trägt den qualifizierten Workflowstandard und die
  drei retained Prefixauswahlzustände aus Exact §5.2. Provider/Auth/Katalogbindung,
  inherited absence, Attempt-/Claim-/Cancellationgrenzen bleiben unabhängig wirksam.
- Bestehende `TaskSelection`-/Admission-/Reviewmetadaten kompatibel erweitern, soweit
  nötig. Modell- und tatsächlich verwendete Prefixauswahl kohärent erfassen, nicht
  verschiedene latest-Reads als atomaren Capture deklarieren. Keine neue Manifest-
  oder Admissionverwaltung und kein taskweiter permissiver Skipflag.
- Originales Use-Receipt dauerhaft mit derselben Discovery-Admission verbinden;
  ACKverlust liest/nachführt den ursprünglichen Vorgang, ohne erneute Auswahlwirkung.
- Normale Prefixconsumer behalten ihren Guard. Der prefixfreie Konstruktionworkflow
  benötigt keine Accepted-Prefixgeneration für ihre eigene Erzeugung. Reader/Writer
  und technische Messung qualifizieren, bevor neue Varianten zugelassen werden.


## Open Exact-validation successor — own A functions, S qualification and workflow scope

**Open text handoff, not implemented.** The sequencing prerequisite is terminal
acceptance of Root's actually running bounded BUILD-014 review/model-selection
repair, not completion of all earlier open steps or the obsolete general authored
exact-closure approval design. Immediately afterward, these target rules replace
contradictory still-open implementation instructions in the same existing owner
steps. Retain accepted packages, original evidence and valid old /1 profiles;
do not reopen K0–K3 or alter BUILD-014's frozen inputs/contracts. New-profile
publication still requires the connected successor readers/writers, not prior
implementation of the superseded design. No second plan owner, queue or VM.

Consume the binding target in [skills.md](skills.md#own-functional-approval-and-successor-compatibility): authored installed changes (A) have
own Q1/behavior/human Q2; Skill and implementation code share one functional
subject with distinct immutable IDs. S retains exact Skill-usage qualification
after independently qualified Tool then ToolSkill, outside normal authored queue,
with no additional installed Q2. Provenance belongs to each revision/usage; mixed
graphs are allowed but do not transfer S exact evidence to changed partner bytes.
Existing Single-root/exact-combination/Recipe-deployment APIs remain current
support boundaries until the following connected successor is implemented.
For new implementation immediately after BUILD-014, earlier blanket authored
combination-review and class-10/50 protection instructions no longer govern the
contradictory open work: only S exact usages retain the extra combination record,
and only class10 retains root protection. Existing exact-profile readers keep
their current checks until a qualified successor replaces them; preserving that
support does not require completing an obsolete general A reapproval engine.

### Existing catalogue/runtime owners: one coherent publication

1. After bounded BUILD-014 acceptance, existing K4/Phase0a/3a and catalogue
   owners consume Validator's own A function/compatibility approvals. Read current
   consumer requirements from existing revision/selection stores; no new registry.
   Compatible provider updates technically re-resolve affected new-task selections
   without new consumer revisions, submission or Q1/Q2. Compare requirements and
   concurrent changes within the existing generation transaction/CAS; separate
   successful per-component CAS is not proof of a coherent graph.
2. New tasks choose newest active approved versions; old/waiting/resumed/child
   tasks retain exact original exports/artifacts. Explicit revocation, live policy,
   resource limits and fencing still apply. Retention/recovery stays with existing
   owners; no universal GC, whole-Recipe replay or older-version fallback assumed.
3. Connect all affected prepare/approval/publication/task/private/child/restore/
   MCP/Prefix readers before publishing new A profiles. Preserve original S exact
   checks and old valid authored profiles. S prerequisite proofs and package
   integrity remain with the installation owner, not normal authored Q1.
4. Consume Composition's class50 migration; remove no class10/root protection.
   Real missing runtime/child support stays here, without new VM, scheduler or
   Rust workflow engine. Existing qualified native admission/capture/custody and
   accounting are reused, not rebuilt by the validation change.

**Outgoing gate:** next open K4/K6/K7/Phase6/8 activation/upgrade successor must
consume both Validator evidence and Composition directed readers before new
selection profiles. BUILD-002 Free and BUILD-010 V2 bridge work remain separately
owned. B1/B2 bytes, I1 activation and the running BUILD-014 scope are unchanged.


## Open Prefixless construction successor — existing owner contribution

**Open future work, not implemented.** Consume the one
[construction-selection contract](docs/reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection)
from the adopted [Prefixless plan](Prefixless_redefine_at_K3.md).
This contribution begins after the actually current bounded owner packet and
Root027/necessary028 repair integration, not after completion of all older plans.
Earlier accepted/running steps and protected002/023/I1I2 criteria are unchanged.
Contradictory earlier construction-only requirements are scoped below for the
new qualified workflow revision; original jobs/records keep their old contract.
The shared contract remains normative; this section only assigns implementation.

### Shared lifecycle contribution; consumer, not selection owner

Input: the existing Prefix/Composition owner's exact admitted workflow and
submitted context. Composition owns the actual selection/capture/native-request
implementation listed in its successor; Prefix owns options/qualification/UI
and job identity. This plan supplies only missing shared task/child/continuation
mechanics through its existing Phase0a/3a/6/8 boundaries. No second resolver,
provider snapshot, queue or workflow loop.

1. Inspect actual `LoopTaskModelSelection`/`capture_task_models`,
   `global_recipe_ports` and native child admission. Preserve the original model/
   credential/catalogue absence or exact choice through Wait/Resume/Retry; neither
   runtime pressure nor a new active prefix permits latest lookup or reselection.
2. When Composition identifies a real missing parent/family/occurrence transport
   contract, implement it once in the current shared owner. Do not require literal
   parent capture equality where a legitimate child has its own recipe identity;
   still reject foreign lineage and never lend another pair's prefix/auth.
3. Preserve the actual task/attempt claim, outcome/effect reconciliation and
   original pending request. Old jobs keep their original contracts. Existing
   reference-safe retention consumes new live job references; no new GC engine
   or deletion from an archive-count assumption.

Output to Composition/Prefix: supported original task/child/continuation and
artifact reference lifetime, not a new selection or approval. The next actually
open affected lifecycle/upgrade gate consumes this contribution before new
workflow publication. Focused actual PG/global/child cases: legitimate different
occurrences versus foreign parent, inherited absent model, post-capture activation,
wait/retry original selection/count, live denial and old retained artifact. No
native/activation acceptance is inferred from this appended work instruction.

Scope of preceding construction instructions: “Open K3-2” future-consumer clauses
requiring inherited prefix-free construction/review, no accepted-prefix lookup
throughout the whole workflow or a Maker connection only in v3.5 remain original
historical/old-profile references. They do not govern the new revision's submitted
context. Ordinary independent role/Recipe defaults and reviewer contracts remain.
No old completed step, test, B1/B2 packet, Golden or acceptance result is edited.

Local supersession reference: the preceding **Offener Folgeauftrag K3-2 ab K4 — Provider-/Design-Ersteinrichtung und Prefix-Lebenszyklus**,
characteristic sentence “Der prefixfreie Konstruktionworkflow benötigt keine Accepted-Prefixgeneration”, describes the earlier
construction contract. For the new qualified revision, use the shared successor's
captured submitted context and own-route child/review rules; no automatic runtime
preference and no edits to that earlier section's original criteria.
