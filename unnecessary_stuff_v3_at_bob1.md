# Unnecessary Stuff v3 at Bob1 — belegte Deltas und Umsetzungsstand

**Stand:** 16. November 2026. **Art:** Auditplan mit anschließendem Implementierungsstand.
**Repository:** `/Volumes/SSDE/brassclaw`.
**Geprüfter Main-HEAD für die Auditkorrektur:** `3fa98f706131f28567fe15924f80b972cc0c6217`.
Die Branch-/Integrationsübersicht in Abschnitt 4 beschreibt die separat datierte
Prüfung vom 15. November 2026; sie ist keine neue Vollabnahme dieser Stände.
Uncommittierte Änderungen und vorbereitete lokale/private Stände wurden getrennt
berücksichtigt. Ein Branch ist kein Abnahmenachweis; fehlende Integration ist
umgekehrt kein Auftrag, seine vorhandene Implementierung erneut zu entwickeln.

## Aktueller Umsetzungsstand

Der anschließende Implementierungsauftrag ersetzt die ursprüngliche Beschränkung
auf Planänderungen. Die folgenden Auditbelege beschreiben den Ausgangsstand;
sie behaupten nicht, dass die dort genannten Fehler weiterhin unverändert bestehen.

| Delta | Aktueller Stand |
| --- | --- |
| L-01 | Implementiert und abgenommen. Der echte Linux-CLI-Lauf `094c40b5-5ae7-468e-8bf5-58d64b22d1ac` besteht alle sechs ausgewählten Fälle einschließlich Full, bestätigtem LocalYolo, fehlendem Vertrag, Umstellungsfehler, ungültigem Host-Home und belegtem MCP-Port. Auch beide aktualisierten Composition-Input-Verträge bestehen, einschließlich LocalSafe ohne Host-Home. Die übrigen 68 CLI-Smoke-Fälle bestehen aus demselben bereits gebauten Programm; insgesamt sind alle 72 Smoke-Fälle grün. |
| L-02 | Implementiert und abgenommen. Frühen allgemeinen Runner ausschließlich im Run-/REPL-Helper entfernt; Factory-Aufnahme vor Schemaverbrauchern und Serve-Aufnahme vor ACL-Seed bleiben erhalten. Dieselben echten positiven REPL-Fälle starten ohne Test-Vormigration und belegen den geordneten Abschluss der eigenen Instanz. |
| L-03 | Dokumentationsabnahme abgeschlossen. Connectionplan auf tatsächlich vorhandenen gemeinsamen Parser/TLS/Poolcode und Event-Store-Delegation korrigiert; weiter offene Caller-/Kapazitäts-/Lifecycleabnahmen bleiben offen. |

Die konkrete zusätzliche Startup-Fehlerbereinigung folgt
[dem begrenzten L-01-Subplan](docs/plans/subplan_step_l01_startup_cleanup_of_bob1.md).
Die vollständigen [Planbelege](docs/validation/2026-11-16-unnecessary-cli-transition.md)
schließen L-01 bis L-03, nicht die separaten nativen CI-Restfälle, vollständige
GitHub-CI oder die übrigen Bob-Pläne.

## 1. Umfang und Ergebnis

Der Audit grenzte **zwei konkrete Codekorrekturen am CLI-Storagewechsel einschließlich
einer begrenzten Composition-Übergabe** und
**eine konkrete Dokumentationskorrektur** ab. Diese Beiträge gehören zu bestehenden
CLI-/Composition-/Connectionownern; sie eröffnen kein zweites Arbeitspaket mit
getrennter Implementierung oder Abnahme. Abschnitt 2 beschreibt ausschließlich
die zusätzlichen, am tatsächlichen Code festgemachten Deltas. Bereits abgedeckte
Arbeit steht nur als Verweis in Abschnitt 3. Vorbereitete Branchstände stehen
als Herkunfts-/Statusnachweis in Abschnitt 4, ohne Merge- oder Entwicklungsauftrag.

Die früheren U-01–U-09-Checklisten, allgemeinen Inventuraufträge, hypothetischen
Storeumbauten, bedingten Neuentwicklungen und pauschalen Testmatrizen sind durch
diese Fassung ersetzt. Der damalige Auditauftrag änderte nur diese Plandatei;
für den anschließenden Implementierungsauftrag gilt der aktuelle Stand oben.

Verbindlich bleiben die aktuellen Verträge in
[recipe.md](/Volumes/SSDE/brassclaw/recipe.md),
[skills.md](/Volumes/SSDE/brassclaw/skills.md),
[tools.md](/Volumes/SSDE/brassclaw/tools.md),
[toolskills.md](/Volumes/SSDE/brassclaw/toolskills.md) und
[simplified_v3.md](/Volumes/SSDE/brassclaw/simplified_v3.md).
Diese Datei wiederholt deren Anforderungen nicht als zusätzliche Aufgaben.

## 2. Konkrete zusätzliche Deltas beim bestehenden Owner

### L-01 — `run` verliert beim Storagewechsel den bereits aufgebauten technischen Vertrag

**Vollständiger Beleg:**
[crates/brassclaw_reborn_cli/src/runtime/mod.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_reborn_cli/src/runtime/mod.rs),
`build_services_input_with_options`, erstellt die aufgelöste Policy, setzt den
Workspace, bei `Full` beziehungsweise bestätigtem `LocalYolo` den Host-Home-Pfad
und gegebenenfalls Google-OAuth-Konfiguration. `build_runtime_input_with_options`
übergibt diesen Input an `execute`. Dessen einziger Aufruf von
`start_postgres_for_run` erstellt anschließend einen neuen `RebornBuildInput`
und ersetzt den vorhandenen vollständig. Dabei wird `local_dev_runtime_policy()`
frisch erzeugt; die übrigen zuvor gesetzten Services-Extras werden nicht übernommen.

[crates/brassclaw_reborn_composition/src/input.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_reborn_composition/src/input.rs),
`RebornBuildInput::postgres_with_reborn_home`, setzt beide Pfadoverrides auf
`None`; `RebornBuildInput::new` setzt auch Auth-/OAuth-/Prozess-/sonstige
Zusatzfelder zurück. Der eigentliche Fehler ist deshalb größer als die verlorene
Policy. Eine isolierte Policykopie wäre ein fehlerhafter Patch: `Full` ist heute
der CLI-Default und benötigt `HostWorkspaceAndHome`. In
[factory.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_reborn_composition/src/factory.rs),
`build_local_dev`, wird diese Policy ohne bestätigten Host-Home-Pfad ausdrücklich
abgewiesen. `transfer_build_input_extras` kann nur weiterreichen, was den
CLI-Storagewechsel zuvor überstanden hat.

`start_postgres_and_upgrade_input` in
[commands/serve.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_reborn_cli/src/commands/serve.rs)
erhält zwar bereits die Policy, erstellt aber ebenfalls einen frischen Input.
Seine Policykopie ist **kein vollständiges Vorbild** für diese Korrektur. Die im
[CHANGELOG.md](/Volumes/SSDE/brassclaw/CHANGELOG.md) verzeichnete Servekorrektur
schließt den getrennten `run`-Fehler nicht.

**Abdeckung und Grenze:** Die langfristige Profil-/Autoritätsvereinfachung bleibt
bei Simplified Phase 3. Dieser begrenzte Beitrag erhält den heute bereits
aufgebauten technischen Vertrag, ohne neue Profile oder Rechte zu schaffen.
Der erledigte [CLI-PG-Subplan](/Volumes/SSDE/brassclaw/docs/agents-v3/subplan_stub_cli_run_embedded_postgres.md)
enthält weiterhin den verlustbehafteten Neuaufbau. Die historische Branchprüfung
in Abschnitt 4 fand keinen vorbereiteten Ersatz. Die betroffenen Codeanker sind
auch am oben genannten neueren HEAD gegenüber dem bisherigen Audit unverändert.

**Exakte dateispezifische Schritte:**

1. In `crates/brassclaw_reborn_composition/src/input.rs`, im vorhandenen
   `impl RebornBuildInput`, eine **neue**, `#[cfg(feature = "postgres")]`
   konsumierende Methode `into_postgres_with_reborn_home` mit diesem Vertrag
   ergänzen. Zum Auditzeitpunkt war dies eine geplante API; ihre inzwischen
   abgeschlossene Implementierung und Abnahme sind oben verzeichnet:

   ```rust
   pub fn into_postgres_with_reborn_home(
       mut self,
       pool: deadpool_postgres::Pool,
       url: brassclaw_secrets::SecretMaterial,
       reborn_home: PathBuf,
   ) -> Result<Self, crate::RebornBuildError>
   ```

   Zunächst fehlende `runtime_policy` mit dem bestehenden Fehler
   `RebornBuildError::MissingRuntimePolicy` ablehnen. Als Storageeingang nur
   `RebornStorageInput::LocalDev` akzeptieren; `Disabled` und bereits bestehendes
   `Postgres` mit `RebornBuildError::InvalidConfig` ablehnen. Keine bestehende
   PG-Verbindung, Masterkeywahl oder Disabled-Semantik still überschreiben.
2. Aus der vorhandenen `LocalDev`-Storagevariante `workspace_root` und
   `host_home_root` mit `Option::take` übernehmen und ausschließlich `self.storage`
   durch `RebornStorageInput::Postgres` ersetzen. Dort die übergebenen `pool`,
   `url`, `reborn_home`, `secret_master_key: None` und beide übernommenen
   Pfadoptionen setzen. Der bisherige LocalDev-Storage-`root` wird entsprechend
   dem bestehenden PG-Vertrag durch das PG-`reborn_home` abgelöst; ausdrücklich
   konfigurierte Workspace-/Homepfade bleiben exakt erhalten. Keine neue
   Dateisystemauflösung, Env-Abfrage, Pfadnormalisierung oder Migration in der Methode.
3. `self` zurückgeben. Weder `Self::new` noch `postgres_with_reborn_home` oder
   einen feldweisen Neuaufbau des gesamten Inputs verwenden. Dadurch bleiben
   `owner_id`, `tenant_id`, `production_trust_policy`, `runtime_policy`,
   `turn_run_wake_notifier`, `runtime_process_binding`, `required_runtime_backends`,
   `require_runtime_http_egress`, `product_auth_ports`, `oauth_provider_configs`
   und `oauth_dcr_provider_configs` unangetastet. Keine neue Transferliste neben
   `transfer_build_input_extras` aufbauen; diese vorhandene Factoryfunktion bleibt unverändert.
4. In `crates/brassclaw_reborn_cli/src/runtime/mod.rs`, `start_postgres_for_run`,
   den vorhandenen `RebornRuntimeInput` als äußeres Objekt behalten und dessen
   Services-Input über `services.take()` entnehmen. Fehlende Services mit einer
   klaren `anyhow::Context`-Fehlermeldung ablehnen; fehlende Policy ebenfalls
   vor Embedded-Start/Poolaufnahme ablehnen. Kein Owner- oder Policyfallback.
   Der tatsächliche Aufrufer liefert bereits einen LocalDev-Input mit Policy.
5. Nach vorhandener Poolbeschaffung den entnommenen Services-Input mit der neuen
   Methode umstellen. Danach wie bisher ausdrücklich
   `.with_tenant_id(input.identity.tenant_id.clone())` anwenden und den umgestellten
   Input in dasselbe äußere Objekt einsetzen. Den frischen `owner_id`-Fallback,
   den Policy-Neuaufbau und den nun unbenutzten lokalen Import
   `local_dev_runtime_policy` entfernen. Keinen bereits anderswo benötigten
   `RebornBuildInput`-Import pauschal entfernen. Sämtliche übrigen Runtimefelder
   bleiben durch Weiterverwendung desselben Objekts erhalten.
6. Fehler nach erfolgreichem eigenen Embedded-Start dürfen die neue Fallibilität
   der Umstellung nicht mit ungeordnetem `Drop` abwickeln: Poolaufbau und
   Inputumstellung in einem begrenzten Ergebnis-Scope auswerten. Vor Rückgabe
   eines Fehlers alle dortigen Pool-/Inputhandles freigeben und für den vorhandenen
   `managed_pg` geordnet `ManagedPostgres::shutdown().await` aufrufen. Der bestehende
   Owner entscheidet selbst, ob er den Server besitzt; externe/reused Server
   nicht stoppen. Ursprünglichen Fehler und möglichen Shutdownfehler beide
   erhalten, keine erfolgreiche Bereinigung vortäuschen. Erfolgsfall übergibt
   unverändert denselben `managed_pg` an den bisherigen Runtime-Lifecycle.
   Keine neue Lifecycleabstraktion oder allgemeine Shutdownreform daraus ableiten.
7. Im vorhandenen CLI-PG-Subplan bei R.1 den verlustbehafteten Neuaufbau durch
   die beschriebene Umstellung ersetzen und die neue Compositionmethode mit
   Pfad nennen. Den ursprünglich erledigten PG-Anschluss nicht erneut als
   offene Arbeit markieren. Das Beispiel gemeinsam mit L-02 aktualisieren.

**Begrenzung:** Keine Serveumstellung oder Änderung der Profile/Resolver in
diesem Delta. Die neue Methode macht die konkrete Speicherumstellung möglich;
sie ist weder ein neuer Policyowner noch ein Runtime-/Recipeworkflow.
Falls der bestehende Simplified-Cutover den Helper vorher ablöst, dort die
Erhaltungsanforderung schließen und diesen Zwischenpatch entfallen lassen.

**Exakte Abnahmen beim bestehenden Testowner:**

- Im vorhandenen `#[cfg(test)] mod tests` von `input.rs` unter PostgreSQLfeature
  den reinen Übergangsvertrag prüfen: `Full`, bestätigtes `LocalYolo` und
  `LocalSafe` behalten ihre jeweils vorher aufgelöste Policy. Vorhandene
  Workspace-/Host-Home-Optionen, Owner und Tenant werden nicht zurückgesetzt;
  ausdrücklich gesetzte Google-OAuth-Daten und technische Backendanforderungen
  bleiben erhalten. Die anschließende CLI-Tenantzuweisung ist als bewusste
  separate Zuordnung zu prüfen. Pfade nicht erneut aus der Umgebung ableiten.
- Für diese Datenübernahme einen echten, noch nicht verbundenen `PgPool` aus
  dem vorhandenen Poolbuilder verwenden. Dieser Test behauptet nur Inputerhalt,
  keine DB-/Lifecycleabnahme. Keine Stubports, erfundenen Erfolgsevidenzen oder
  simulierten Toolaufrufe zum Nachweis anderer Feldgruppen hinzufügen. Deren
  Erhalt folgt zusätzlich aus der geprüften ausschließlichen Storagezuweisung.
- Die neuen Ablehnungen für fehlende Policy sowie `Disabled`/`Postgres` prüfen;
  fehlende CLI-Services müssen vor eigener Serveraufnahme scheitern. Für den
  Umstellungsfehler nach eigener Serveraufnahme geordnetes Ende nachweisen;
  keine bloß fehlende Warnung als Prozessende ausgeben.
- Den echten CLI-/Factorypfad mit `Full` und vorhandenem temporärem Host-Home-
  Verzeichnis bis zur Runtimebereitschaft prüfen; zusätzlich den bestätigten
  `LocalYolo`-Übergang mit demselben Mountvertrag und den engeren `LocalSafe`-
  Vertrag am umgestellten Input nachweisen. Nicht alle Profile durch `LocalDev`
  ersetzen, um einen fehlerhaften Startup zu verdecken. Den vorhandenen
  `build_runtime_input_accepts_confirmed_local_dev_yolo_profile` nicht als
  Nachweis des PG-Wechsels ausgeben: Dieser Test endet davor.
- Den echten Start mit L-02s unten benanntem REPL-Smoke gemeinsam abnehmen;
  unveränderte Nachweise nicht für eine zweite Erfolgsmeldung erneut ausführen.

### L-02 — Überflüssiger Migrationsrunner im `run`-/REPL-Startpfad

**Belegte vollständige Reihenfolge:** `execute` ruft `start_postgres_for_run`
mit allgemeinem Migrationsrunner auf, danach den tatsächlich wirkungslosen
`with_run_local_trigger_fire_access_checker`, anschließend `build_reborn_runtime`.
In [runtime.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_reborn_composition/src/runtime.rs)
liegen vor `build_reborn_services` nur Voraussetzungen wie Policy-, Identity-
und Tokio-Runtimeprüfung, keine Anwendungstabellenabfrage. Die PG-Verzweigung in
[factory.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_reborn_composition/src/factory.rs),
`build_reborn_services`, führt denselben allgemeinen Runner vor Providerinitialisierung
und `build_local_dev` aus. Deren vorgeschalteter Inputtransfer ist reine Datenübernahme.
Die Entfernung des frühen CLI-Aufrufs lässt somit die erste notwendige Migration
vor allen tatsächlichen Schemaverbrauchern bestehen.

Das belegt zwei Runneraufnahmen, nicht zweimal angewandte Migrationen oder einen
gemessenen Performanceverlust. `run_migrations` und `reconcile_history` in
[crates/brassclaw_pg/src/migrations.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_pg/src/migrations.rs)
besitzen bereits Historien-/Prüfsummenprüfung. Die Factory bleibt auch für ihre
anderen Aufrufer Schemaowner. Ungültige Runtimeinputs können nach der Entfernung
vor der ersten Migration abgewiesen werden; Fehlertexte und Fehlerzeitpunkt sind
nicht identisch mit dem früheren CLI-Runner, erfolgreiche Schemaaufnahme bleibt geordnet.

**Abdeckung:** Der bestehende Connectionplan, Phase B, besitzt den CLI-/PG-
Lifecycle, konkretisiert diese Entfernung aber nicht. Der erledigte CLI-PG-
Subplan enthält den zusätzlichen Aufruf weiterhin. Die historische lokale
Branchprüfung in Abschnitt 4 fand keine vorbereitete Entfernung. Dieses Delta
bleibt ein Beitrag zum bestehenden Owner, keine neue Migrationsarchitektur.

**Exakte Änderungen je Datei:**

1. In `crates/brassclaw_reborn_cli/src/runtime/mod.rs`, ausschließlich in
   `start_postgres_for_run`, den gesamten `migrations::run_migrations(&pool)`-
   Block einschließlich seines `.await`/Fehlerkontexts und den lokalen
   `use brassclaw_pg::migrations` entfernen. Poolbeschaffung, L-01s Inputerhalt
   und die Rückgabe des bestehenden Serverowners bleiben erhalten.
2. In `docs/agents-v3/subplan_stub_cli_run_embedded_postgres.md`, R.1, genau
   diesen lokalen Runner samt Import auch aus dem Beispiel entfernen. Erklären,
   dass `build_reborn_runtime` über `build_reborn_services` den allgemeinen
   Anwendungsschema-Runner vor dem ersten Schemaverbraucher aufnimmt. R.2 muss
   weiterhin zuerst PG-Input beschaffen und danach die Runtime aufbauen.
3. Keine Implementierungsänderung an `factory.rs`, `migrations.rs` oder
   `booted_db.rs`; kein Skip-Schalter, Cache, neuer Nachweistyp oder Migrationszähler.
   Eigenständige Managementkommandos und andere Schemaowner bleiben unverändert.
4. `start_postgres_and_upgrade_input` in `serve.rs` ausdrücklich ausnehmen:
   Dort benötigt der vor Runtimeaufbau liegende ACL-Seed bereits das Schema.
   Dessen bestehender Simplified-Cutover ist keine Voraussetzung dieses eng
   begrenzten `run`-Deltas und wird hier nicht vorgezogen.

**Konkrete Abnahme:**

1. In [crates/brassclaw_reborn_cli/tests/smoke.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_reborn_cli/tests/smoke.rs)
   den vorhandenen echten Fall
   `repl_exit_command_exits_cleanly_without_touching_v1_state` verwenden.
   Vor Prozessstart im vorhandenen temporären Verzeichnis `home_dir` tatsächlich
   anlegen: Mit L-01 bleibt der Default `Full` erhalten und sein Host-Home-Pfad
   muss existieren. Den Fall nicht auf `LocalDev` herunterstellen.
2. Frisches `reborn_home` ohne vorhandene PG-Daten und den vorhandenen
   `free_pg_port()` verwenden; keine vorgelagerte Migration über Testcode,
   Configkommando oder vorbereitete Datenbank. Über `isolated_command` bleiben
   fremde DB-Endpunkte ausgeschlossen. Den tatsächlich gestarteten CLI-Prozess
   mit `/exit` beenden lassen.
3. Die bestehenden positiven Assertions behalten: erfolgreicher Prozessabschluss,
   tatsächliches `brassclaw-reborn: runtime started`, leeres Reply-stdout und
   keine v1-Stateverzeichnisse. Fehlende Einzelwarnungen oder ein beliebiger
   Nichtnull-Exit gelten nicht als erfolgreicher Startup.
4. Nach Prozessende für diese eigene Instanz zusätzlich fehlende
   `reborn_home/postgres/data/postmaster.pid`, geschlossenen gewählten PG-Port
   und keine `ManagedPostgres dropped without calling shutdown`-Meldung prüfen.
   Diese Kombination belegt den eigenen geordneten Abschluss; fremde Prozesse
   bleiben unberührt. Keine neue unbetreute Datenbank oder Testmigration starten.
5. Die entfernte Duplikation durch Prüfung der tatsächlichen geänderten
   Aufrufkette belegen. Der erfolgreiche Start allein zählt die Runneraufrufe
   nicht. Ein Mockrunner, pauschaler SQL-Zähler oder Migrationscache ist dafür
   weder erforderlich noch zulässig. Migrationsfehler bleiben im vorhandenen
   Factoryfehlerweg Fehler; daraus keinen neuen universellen Fehlerharness ableiten.

L-01 und L-02 teilen diese echte Startabnahme. Keine neuen Komponentenfixtures,
Provider-/Modellcalls oder erneute Qualifikation ausgelieferter Komponenten.

### L-03 — Connectionplan fordert bereits vorhandene Konsolidierung erneut

**Beleg:** In
[crates/brassclaw_reborn_event_store/src/lib.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_reborn_event_store/src/lib.rs)
delegiert `postgres_backed::build_pool` unmittelbar an
`brassclaw_pg::pool::build_pool`; nur die Event-Store-Fehlerabbildung bleibt lokal.
[pool.rs](/Volumes/SSDE/brassclaw/crates/brassclaw_pg/src/pool.rs)
verwendet bereits den gemeinsamen Owner `connection::prepare`/`tls_connector`.
Der [Connectionplan](/Volumes/SSDE/brassclaw/docs/plans/simplify_connections_v3.5.md)
beschreibt dagegen noch getrennte Builder und warnt operativ davor, den Event-Store
an den damaligen allgemeinen Builder anzuschließen. Dieser technische Anschluss
ist bereits vorhanden; seine Dokumentation ist der konkrete Restfehler.

**Exakte Änderungen ausschließlich in**
`docs/plans/simplify_connections_v3.5.md`:

1. In Abschnitt 2, Tabellenzeile `PostgreSQL`, die Aussage „The current builder
   is not yet sufficient to replace every other builder unchanged“ durch eine
   aktuelle Abgrenzung ersetzen: Gemeinsamer Parser-/TLS-/Poolcode besteht in
   `brassclaw_pg::connection` und `brassclaw_pg::pool`; die vollständige Adoption
   anderer Caller, Kapazität und Lebensdauer ist damit nicht bewiesen. Die
   Zeile auf die vorhandenen relativen Quellpfade
   `../../crates/brassclaw_pg/src/connection.rs` und
   `../../crates/brassclaw_pg/src/pool.rs` verlinken.
2. In derselben Tabelle, Zeile `Event-store connections`, „Its standalone path
   separately parses hosts and enforces TLS“ und den daraus abgeleiteten neuen
   Konsolidierungsauftrag ersetzen: Standalone delegiert bereits über
   `postgres_backed::build_pool` an den gemeinsamen Poolbuilder; `SharedPool`
   bleibt die bestehende Wahl für dieselbe DB und passenden Besitzvertrag.
3. Den gesamten Abschnitt `Confirmed connection-policy divergence` bis vor
   `Other duplicated mechanisms are **audit candidates**` historisch einordnen.
   Das betrifft beide bisherigen Absätze, insbesondere auch „Merely routing
   the event store through today's general builder could remove protections“.
   Die alte Differenz darf nach einer bloßen Überschriftenänderung nicht als
   aktuelle Warnung/Implementierungsvoraussetzung stehen bleiben.
4. Diesen Abschnitt mit einem aktuellen Absatz abschließen: Die Event-Store-
   Delegation und der gemeinsame geparste Verbindungsvertrag sind vorhanden;
   daraus folgt keine Gesamtfreigabe aller Caller oder Deployments. Remote-TLS,
   Zertifikatsprüfung und bestehende Protokollprüfungen bleiben erforderlich.
   Den folgenden Absatz über unterschiedliche Identitäten unverändert erhalten.
5. In Phase B nur den ersten Punkt `Consolidate parsing, endpoint classification,
   TLS mode enforcement, ...` in vorhandenen Implementierungsanteil und offene
   Caller-/Abnahmeanteile aufteilen. Den gemeinsamen Code als vorhanden
   kennzeichnen; keinen erneuten Parser-/TLS-Umbau verlangen. Den zweiten Punkt
   zu den Verbindungsfällen und die übrigen Caller-/Kapazitäts-/Lifecyclepunkte
   nicht pauschal abhaken: Vorhandener Code belegt nicht ihre vollständige Abnahme.

**Reine Dokumentabnahme:** Alle genannten alten Gegenwartsbehauptungen sind
entfernt oder unmissverständlich historisch, und kein unmittelbar folgender Satz
fordert denselben erledigten Umbau erneut. Abschnitt 2, Phase B und deren Exit
beschreiben konsistent vorhandene gemeinsame Mechanik und verbleibenden Umfang.
Die vorhandenen lokalen Links zeigen auf `connection.rs`, `pool.rs` und
Event-Store-`lib.rs`. Keine Checkliste erklärt ohne vorhandenen Nachweis alle
Verbindungsfälle, Kapazität oder Lifecycle für bestanden. Cargo, PG-Start und neue
TLS-Tests sind für diese Textkorrektur nicht erforderlich; die technische
Implementierung und bestehende Nachweise werden nicht verändert.

## 3. Bereits abgedeckte Arbeiten — ausschließlich Verweise

Diese Tabelle vergibt keine zusätzlichen Aufträge und keine eigene Abnahme.
Offene Arbeit bleibt im genannten Plan; vorhandene Implementierung bleibt dort
mit ihrem tatsächlichen Abnahmestand verzeichnet.

| Früheres Thema | Ausschließliche bestehende Zuordnung / Grund für Entfernung aus der Aufgabenliste |
| --- | --- |
| Übertragung ausgelieferter Validierung | Main `71c3057a6ac0f65d730aa220dd9970f6f9a67ac6` implementiert den Startup-Handoff; verbindlicher Vertrag in `AGENTS.md` und [Entwicklungspolicy](/Volumes/SSDE/brassclaw/docs/development-policy.md). Keine erneute Seedqualifikation oder Reimplementation hier. |
| Instanzweite Toolpolicy, Abschaffung von Profilen/Operationsfreigaben/Trigger-ACL und altem WebUI-Mehrbenutzermodell | [simplified_v3.md](/Volumes/SSDE/brassclaw/simplified_v3.md), §§1.1/9 und Phasen 1–3/6/8. Vorhandener retained `InstanceToolAuthorizer` ist kein fehlender Neubau. |
| Sempai Pre-call/Post-turn, fachliche Komponenten und Wiederverwendung | [sempai.md](/Volumes/SSDE/brassclaw/sempai.md), [kohai.md](/Volumes/SSDE/brassclaw/kohai.md), [Sempai-Konversion](/Volumes/SSDE/brassclaw/docs/plans/sempai_v3.5.md) und [Roadmap](/Volumes/SSDE/brassclaw/path_to_v3.5.md), V35-09–16/28. Keine zweite Konversionscheckliste in dieser Datei. |
| Promptformatierung, eigener Reviewerprefix, strukturierter Handoff und vollständige Modellrequestgrenzen | [Zentraler Modell-/Handoffvertrag](/Volumes/SSDE/brassclaw/docs/reborn/model-token-limits.md) und [Prefixplan](/Volumes/SSDE/brassclaw/docs/plans/prefix_v3_upgrade.md). Ein offener Nachfolger ist keine neue hier zu entwerfende Schnittstelle. |
| Sempai-Sonderweg und vollständige technische Toolregeln | Derselbe Simplified-/Sempai-Cutover. Ein spezialisiertes `enabled/revision`-Read allein beweist ohne den gesamten Dispatchpfad keine fehlende Enforcementgrenze; daraus folgt hier kein zusätzlicher Patch. |
| Eigene A-Freigaben, exakte S-Evidenz und kompatible Nachfolger | [B1](/Volumes/SSDE/brassclaw/Repair_bob_1_v3_at_k3.md), [B2](/Volumes/SSDE/brassclaw/Repair_bob_2_v3_at_k3.md), [B3](/Volumes/SSDE/brassclaw/Repair_bob_3_v3_at_k3.md). Vorhandene Subject-/Writer-/Codec- und private Nachfolgerarbeit nicht erneut entwickeln. |
| Mehrere besondere Ausführungen eines ausgewählten Schritts | Bestehender Composition-/B2-Vertrag und V35-13/14; private N-Stände enthalten bereits strukturierte Unterstützung. Keine neue Schleifenengine, Journalstruktur oder Migrationsnummer hier. |
| Native Antwortverwahrung, Commitverlust, Reconciliation, Authcontinuation und Lifecycle | Aktuelle Implementierung und [Statusindex](/Volumes/SSDE/brassclaw/docs/implementation-status.md). Keine zweite Custody-/Recovery-/Auth-Implementierung und kein allgemeines erneutes Audit. |
| CLI-/PG-Shutdown und Serve-Schemareihenfolge außerhalb L-01/L-02 | [Connectionplan](/Volumes/SSDE/brassclaw/docs/plans/simplify_connections_v3.5.md), Phase B, und Simplified Phase 3a. Kein zusätzlicher hypothetischer Fehlerbereinigungsauftrag. |

## 4. Berücksichtigte vorbereitete und private Stände

**Lesende Grundlage vom 15. November 2026:** lokale Branch-/Archivrefs, ihre tatsächlichen Änderungen
seit der jeweiligen gemeinsamen Basis, registrierte Worktrees und deren
uncommittierte Änderungen an den früheren Planquellen. 87 Branch-/Archivrefs
entsprachen damals 85 unterschiedlichen Tips. In 83 Tips mit `start_postgres_for_run`
stand weiterhin die frische LocalDev-Policyerzeugung; in zwei Tips fehlte der
Helper. Eigene Branchdeltas enthielten weder die L-01-Korrektur noch die
L-02-Entfernung oder die L-03-Planberichtigung. Das ist eine lokale Quellbeobachtung,
keine Aussage über nicht zugängliche externe Entwicklungsstände.

Der registrierte Checkout `/private/tmp/brassclaw-j01-implementation` war nicht
mehr als Git-Checkout lesbar; sein Branch-Tip `codex/j01-run40-import` wurde über
die Gitobjekte berücksichtigt. Die übrigen lesbaren registrierten Worktrees
enthielten an den früheren Planquellen keine zusätzlichen uncommittierten
Seitendeltas außerhalb des Hauptarbeitsbaums. Dessen laufende fremde Änderungen
wurden gelesen und nicht überschrieben.

Für den Integrations-/Abnahmestand gelten die vorhandenen
[79-Tip-Inventur](/Volumes/SSDE/brassclaw/docs/validation/2026-11-14-prepared-integration-inventory.md),
[Branch-/Build-Integrationsnachweise](/Volumes/SSDE/brassclaw/docs/validation/2026-11-15-combined-build-and-branch-cleanup.md)
und der aktuelle Statusindex. Ein nicht in Main enthaltener Commit kann bereits
über korrigierte Nachfolger übernommen sein. Daher keine automatische
Neueinführung, Zusammenführung oder Ganzbranchübernahme aus dieser Tabelle.

| Vorbereiteter Stand | Konkrete Bedeutung für diesen bereinigten Plan |
| --- | --- |
| `codex/b1-06-approval-writer` — `d41db6c37f5144d539fc1d7d86165bbda611e0a0` | Guarded Writerstand laut bestehender Inventur bereits über Mainnachfolger abgedeckt. Das frühere offene authored Verhalten nicht wiederherstellen; vollständige B1-Arbeit bleibt bei B1. |
| `codex/b2-02-retained-selection-v2` — `e0be06d8fbe9c08abd551ac522034255a2e90826` | Vorbereitete Vertrags-/Carriage-/Migrationsarbeit, laut Statusindex nicht abgenommen. Kein neuer Selectioncodec als Ersatzauftrag; dieser Branch ist zugleich kein Beleg aktivierter Nachfolgerunterstützung. |
| `codex/bob-b2r04-capture-retained-audit-20261114` — `0296da2b5031af88636327dfb4bd5da57bc5b2b3` | Begrenzter Parserlimit-Anteil bereits separat übernommen; der volle Capture2-Default hat laut Inventur fehlende V207-/V213-Leseranbindung. Kein neuer Capture-/Settingsauftrag aus dieser Datei. |
| `codex/n01-qualified-flow` / `codex/n01-e858-preserved` — `e8583f89691773672a7ed69d18fa6f99fb04a010`; Varianten `456c061284d6a1d7423e0a1730f8532e488dab39` und `7c7efe1ef04bebc87d72d0662f9b22e9c1fffaec` | Strukturierte Flow-/Occurrence-Unterstützung ist bereits vorbereitet. Bestehende Inventur benennt die noch fehlende gemeinsame strikte Leseradoption und eine V214-Kollision. Keine erneute Runtimeentwicklung und keine neue Migrationsnummer hier; auch keine Aktivierungsbehauptung. |
| `codex/q01-capture-policy` — `8ed62ca7b7f01224a3a8e224ba14cdceb8ed9407` | Unabhängige Transaktionscustody bereits übernommen; vollständiger Cold-boot-Nachfolger nicht als transparenter Upgrade unterstützt. Vorhandene Readers erhalten, keinen parallelen Recoveryauftrag anlegen. |
| `codex/r01-independent-seeds` — `6f645ba0fbbeefc712a03a532e5c3e95efd6698b` | Vorbereiteter Gesamtgraph mit bestehenden Capture-/Readerabhängigkeiten; historische Consumer-Usage-Rekonstruktion laut Handoff offen. Nicht als fehlende komplette Neuentwicklung behandeln. |
| `codex/t01-history-qualification` — `92fc13a616fdc2861bb16fd98296f74f6be47843` | Unabhängiger Preview-/Produceranteil bereits übernommen; kompletter History-/Memory-Vertrag hat bestehende R/Q-Voraussetzungen. Kein zweiter Ergebnisstore- oder Historyauftrag. |
| `codex/b3-06-durable-q1-work-wt` — `a069f5df327762391288c3bbc92e7323b44f0695`; `codex/b3-09-webui-review-panels` — `f857421e3fe77b2ec437e4e6ab87fcaf4fe84e0f` | Bestehende Inventur/Statusnachweise unterscheiden übernommene begrenzte Beiträge von weiterhin offenen B3-Gesamtgates. Diese Datei erzeugt weder zweite Ausführungsarbeit noch zweite Reviewoberflächen. |
| `codex/speedup2-implementation` — `04565b6f8705561e46fdced6fa7399147740ec34` | Tatsächlicher eigener Delta betrifft Build-/CI-Skripte; kein vorbereiteter Ersatz für die CLI-Produktstartkorrekturen L-01/L-02. Buildarbeit wird hier weder wiederholt noch als abgenommen erklärt. |

## 5. Gestrichene unbelegte Aufträge

- **Kein Storegraphumbau:** Die PG-Aufrufe stehen in alternativen Zweigen.
  Ein zusätzlich unnötiger konkreter Store wurde hier nicht belegt. Deshalb
  kein neues Inventar, keine Factoryvorverlegung und keine neue Gesamtgraph-API.
- **Kein universeller `PgCapabilityIo`-Ersatz:** Unterschiedliche Adapter haben
  unterschiedliche Aufgaben und Lesesemantik; native Custody existiert bereits.
- **Kein pauschaler Kürzungs-/Cachecutover:** Die private Kürzung und Eviction
  sind im Adaptercode vorhanden. Die frühere Datei belegte aber keinen konkreten
  gewöhnlichen Produktfall, der dadurch ein benötigtes Ergebnis verliert.
  Kein neuer Such-, Test-, Settings- oder Speicherauftrag allein aus dieser
  Möglichkeit. Bestehende Modell-/Ressourcenverträge bleiben maßgeblich.
- **Kein Auftrag zur Zusammenlegung nativer Images oder erneuten Diagnose:**
  Getrennte korrekte Besitzer sind zulässig. [speedup2.md](/Volumes/SSDE/brassclaw/speedup2.md),
  SP-05, besitzt die vorhandene Diagnose; kein zusätzlicher Nachweis ist hier gefordert.
- **Keine neuen DB-Endpunktregeln:** Die vorhandene Präzedenz von
  `BRASSCLAW_PG_URL` vor `DATABASE_URL` wird nicht geändert.
- **Keine neue Test-/Manifest-/Cleanupkampagne:** Geltende Entwicklungsregeln
  decken passende Abnahmen und die Entfernung tatsächlich unbenutzten Codes ab.
  Es besteht kein Anlass, unabhängig von L-01–L-03 Dateien oder Dependencies zu ändern.

## 6. Historische Prüfgrenze der Auditfassung

Zum Auditzeitpunkt geprüft wurden die oben benannten Aufrufketten, vorhandenen
Planaufträge, lokalen/archivierten Branchdeltas und relevanten Worktreeänderungen.
Die Branchübersicht trennt vorbereiteten Code, übernommenen Teilumfang und offene
Abnahme. Daraus folgt weder Mergefreigabe noch vollständige Produktunterstützung.
Keine neue Laufzeitgrenze, kein Stub und kein simulierter Erfolgsnachweis ist
Bestandteil der Restdeltas. L-01 bezeichnet seine neue Übergangsmethode ausdrücklich
als geplant; alle damals als bestehend verwendeten Typen und Methoden sind
Quellanker. Cargo, PostgreSQL und Provider wurden für die ursprüngliche
Dokumentbereinigung nicht gestartet. Damals wurde ausschließlich diese Datei
bearbeitet, ohne Staging, Commit oder Push. Der anschließende Implementierungs-
und Abnahmestand steht oben und in den verlinkten Nachweisen.


**Historische Auditkorrektur vom 16. November 2026:** L-01 ersetzt die unvollständige
Policykopie durch Erhalt des vorhandenen Inputs einschließlich beider
Pfadoverrides und Authkonfiguration. Die neue Übergangsmethode ist ausdrücklich
in dieser ursprünglichen Auditkorrektur als noch zu implementieren bezeichnet;
Fehler-/Ownership- und Testgrenzen sind
benannt. L-02 ist auf die belegte `run`-Kette begrenzt und benennt die echte
Start-/Exitabnahme einschließlich vorhandener Homefixture. L-03 benennt die
konkreten Tabellenzeilen, beide veralteten Absätze und den abzugrenzenden
Phase-B-Punkt. Die relevanten Codeanker sind gegenüber dem Audit-HEAD
`71c3057a6ac0f65d730aa220dd9970f6f9a67ac6` unverändert; der bestehende
Connectionplan wurde im aktuellen Arbeitsbaum gelesen. Dies sind Quellen- und
Dokumentprüfungen, keine Compilation, Testausführung oder Laufzeitabnahme.
