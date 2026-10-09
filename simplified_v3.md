# Simplified v3: Plan zur Vereinfachung von Sicherheitsmodell, Runtime und WebUI

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
  many Skills explain one Tool usage and have associated executable PythonCode;
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

Current code still has plain text substitution, fresh state in nested step
execution and incomplete immutable version manifests/binding preparation. The
new typed inputs interface and strict single-component validation require
implementation and production-path acceptance; do not claim these are shipped.

## Verbindliche Skill-Architektur

Ein **Skill** ist genau ein wiederverwendbares Nutzungsmuster eines Tools für
den Orchestrator: **Prosa-Anleitung plus ausdrücklich zugeordneter ausführbarer
PythonCode**. Prosa erklärt Zweck, Parameter, Voraussetzungen und Ergebnis-/
Fehlerbehandlung; PythonCode implementiert die Benutzung. „Leaf Skill“ meint
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
          → Falls ein Sempai mit dem Kohai verbunden ist wird der Prompt analysiert und optimiert und an Kohai zurückgegeben.
          → Kohai fügt den Prefix hinzu und schickt den gesamten Promt zum LLM
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

Gesichtet wurden die Repository-Orientierung (`README.md`, `CONTRIBUTING.md`, `AGENTS.md`, `CLAUDE.md`), Workspace- und Crate-Struktur, Reborn-Architektur- und Vertragsdokumente, relevante Konfigurationen, Einstiegs- und Kompositionspunkte, Scope-/Profil-/Trust-/Ingress-Implementierungen, WebUI-Module sowie Test- und CI-Inventare. Die ursprüngliche Untersuchung war statisch; die inzwischen ausgeführten gezielten Implementierungsprüfungen stehen in `docs/plans/simplified-v3-implementation.md`. Die Codebase umfasst rund 60 Workspace-Crates, etwa 1.200 Rust-Dateien und zahlreiche crate-lokale Architekturverträge.

### 2.1 Aktuelle Pfade

- **Produktstart:** `src/main.rs` delegiert an `brassclaw_reborn_cli`; CLI-Befehle liegen in `crates/brassclaw_reborn_cli/src/commands/`.
- **Komposition:** `brassclaw_reborn_composition` baut Runtime, Substrate, Loop-Driver, Provider und WebUI zusammen.
- **Requestpfad:** WebUI-Ingress authentifiziert Bearer-Token; WebUI-Handler erhalten `WebUiAuthenticatedCaller`; Komposition reicht in Turn-/Host-APIs weiter.
- **Ausführung:** Turn-Koordination und Runner rufen einen Agent-Loop auf. Capability-Aufrufe laufen über `CapabilityHost`, Autorisierung/Obligations-/Approval-Pfade und Dispatcher/Runtime-Lanes.
- **Monty-Istzustand (Abweichung vom Ziel):** Der Runner ruft `MontyTurnDriverPort::drive_turn` auf. Komposition konstruiert `PersistentMontyDriver` mit einer leeren `MontySessionRegistry`; die erste Eingabe eines `TurnScope` erzeugt über `prepare_monty_session` eine VM. Weitere Turns setzen die chatgebundene VM fort. Das ist weder ein globaler Orchestrator noch ein VM-Start beim Boot.
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
8. **Orchestrator-Lebenszyklus und versteckte Chatbindung:** `basic_mode.py` hält einen Verlauf in VM-Variablen, behandelt leere Eingaben/Stop als `FINAL` und hängt am chatgebundenen Bootstrap. Der genaue Eingabelookup samt Thread-/Turn-/Run-Prüfung ist inzwischen umgesetzt; History-Cutoff und globale Fortsetzungen bleiben offen. Gemeinsames Boot-Seeding wurde aus der WebUI nach `component_boot.rs` verschoben. Die globale VM ist noch nicht produktiv verdrahtet. `MontySession` besitzt einen beim Start angelegten `LimitedTracker`, kumulative Tokens und stdout. Ein Singleton allein würde Kontexte vermischen und Lebenszeitlimits falsch anwenden. Der aktuelle Python-Ablauf wechselt außerdem nach Matching-/Recipe-Fehlern in den LLM-Pfad; das muss dem Fehlervertrag aus §1.1 weichen.

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
2. **Assoziation und Approval getrennt speichern und prüfen.** Die exakten
   `skill-association/1`- und `skill-association-approval/1`-Verträge aus skills.md
   über versionierte Migrationen, neutrale Ports und die unterstützten
   Store-/Reviewpfade implementieren. Stabile UUID-Verweise bleiben versionslos;
   die unveränderliche Assoziation gehört zur ausgewählten Skillrevision.
   Der vertrauenswürdig erzeugte Approval-Datensatz enthält Association-Checksum,
   vollständige eindeutige Komponentenreferenzen mit Klasse/Revision/Checksum,
   validation_mode, Q1-/Q2- und reale Verhaltensnachweise. Checksum-Abdeckung
   umfasst ausführungsrelevante Metadaten, Abhängigkeiten und Toolimplementierungen.
   Doppelte/unbekannte Felder, falsche Klassen, fehlende Referenzen und nicht
   passende Evidenz ablehnen. Reviewdatensätze sind weder selbst ausgestellte
   Freigaben noch Invocation-Leases. Neue Kombinationen benötigen neue Evidenz,
   auch bei kompatiblen Änderungen; unveränderte Komponenten dürfen wiederverwendet
   werden. Autorreview/Q1/Q2/Verhalten prüfen Bedeutung vor Aktivierung; IBS
   verifiziert beim Start nur strukturierte genehmigte Kombinationen, ohne LLM.
3. **Approval-Provenienz eindeutig machen.** authored-Kombinationen benötigen
   erfolgreiche vertrauenswürdige Q1-, menschliche Q2- und Verhaltensnachweise.
   Nur der kontrollierte first-party system_seed-Bootstrap darf q2_ref=null
   führen; auch dort sind die erforderlichen Q1-/Integritäts- und beobachteten
   Verhaltensnachweise nötig. source=system oder validation_status=validated
   reichen nicht. Importierte Altzeilen ohne Evidenz nicht als vollständig
   migriert markieren; fehlende Zuordnungen/Verträge/Nachweise sichtbar machen
   und vor gewöhnlicher Aktivierung auflösen. Existierende Insert-Defaults und
   Content-Checksums nicht als fertige Ziel-Approval-Infrastruktur ausgeben.
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

**Einfaches Speicherbudget (Standard „Beim Start bestimmen“):**

- Drei ausdrücklich getrennte Modi: `startup` bestimmt beim Start einmalig einen konservativen endlichen Grenzwert; `manual` verwendet den Betreiberwert; `automatic` aktiviert optionale langsame Anpassung. Im Standard- und manuellen Modus kein periodischer Speicherdruck-Monitor und keine nachträgliche Senkung aufgrund kleiner Druckwerte. Bestehende konfigurierte Grenzen beim Upgrade als `manual` erhalten, statt sie umzudeuten.
- Defaults für neue Instanzen: Start-/Ersatzobergrenze 512 MiB, Reserve 512 MiB, optionales Messintervall 60 Sekunden und Wachstumsschritt/-spielraum jeweils 64 MiB. Das sind einstellbare Budgetwerte, keine vorab belegten Speicherreservierungen. Der einmalige Startwert ist höchstens die konfigurierte Obergrenze; verlässliche geringere Kapazität abzüglich Reserve reduziert ihn. Bestandswerte bleiben unverändert; beim Upgrade den Modus ausdrücklich als `manual` setzen und die Settingsrevision fortschreiben.
- Beim Start verfügbare Kapazität einschließlich nachweisbarer Host-/Containergrenzen einmalig erfassen. Reserve für Betriebssystem, andere Anwendungen, Rust-Host und PostgreSQL abziehen. Den Startwert auf den konfigurierten endlichen Ersatz-/Startwert begrenzen: verfügbarer RAM ist keine Aufforderung, ihn vollständig Monty zuzuweisen. Ohne verlässliche Messung den dokumentierten endlichen Ersatzwert verwenden und die Messlücke sichtbar machen. Kein unbegründeter RAM-Prozentsatz, keine Vorabreservierung, keine Seitenbegehung oder erzwungene Seitenrückgewinnung.
- Nach einem Schritt nur entbehrliche temporäre Daten freigeben; typisierte Ergebnisse für spätere Recipe-Schritte behalten. Nach abgeschlossenem/abgebrochenem Task dessen transiente Ausführungskontexte und Child-Heaps nach bestätigter Übergabe freigeben. Lebende Fortsetzungen, persistente History und ungeklärte Wirkungsevidenz nicht löschen. Aufbewahrte Evidenz über einen dauerhaften Recovery-Eigentümer verwalten; VM-Speicher ist kein Checkpoint.
- Begrenzte Aufnahme/Parallelität und wirksame gemeinsame Heapgrenze bleiben unabhängig von Messungen bestehen. Volle Queues warten ohne Polling mit derselben Attempt-Identität; Control und aktive Ergebnisse bleiben verarbeitbar. Ein kleiner/einzelner erhöhter Druckwert darf nicht die gesamte Aufnahme pausieren. Im optionalen Automatikmodus nur bei kritischem Druck oder nachgewiesen unzureichender Kapazität pausieren; bei fehlender/veralteter Messung kein Wachstum, aber mit vorhandener endlicher wirksamer Grenze weiterarbeiten, solange deren belegter Heap noch sicheren konfigurierten Spielraum hat.
- Optionaler Automatikmodus: native Messungen frühestens im konfigurierten Intervall (Default 60 Sekunden), ohne engen Work-Polling-Loop/LLM. Vorhandenen Heap einmal zählen; Reserve separat. Schrittweise bedarfsgerechte Erhöhung, kontrollierte Senkung und veraltete Messungen behandeln. Erhöhter Druck allein verhindert optionales Wachstum, beendet aber keine laufenden Tasks und pausiert bei ausreichender Kapazität keine neue Arbeit. Kritischer Druck oder Kapazität unter lebendem Heap darf Aufnahme begrenzen; wirksame Grenze niemals unter belegten Heap setzen.
- Modus, Speicherwert, Reserve, endlicher Start-/Ersatzwert und Automatikintervall sind über bestehende WebUI/DB/API-Verträge verwaltbar. Gültige manuelle Änderungen werden live bestätigt; nicht erfüllbare Senkungen vor Persistenz ablehnen. Serialisierte Worker-Feasibility und DB-CAS/Commit-Ergebnis koordinieren: keine stale Inspect-Prüfung und keine Erhöhung mit neu zugelassenen Allokationen vor gesichertem Commit. Ein abgebrochener HTTP-Waiter darf eine angenommene Veröffentlichung nicht zurückziehen. Gewünschte Settingsrevision, bestätigte wirksame Heaprevision, Heapverbrauch, Start-/Ersatzgrund und optionale Messaktualität anzeigen; keine DB-Schreibvorgänge pro automatischer Messung.

Der Default für `max_duration_secs` beträgt instanzweit 600 Sekunden und gilt bei fehlender expliziter Betreiberkonfiguration in Config, DB, API und WebUI einheitlich. Bestehende explizite Werte beim Upgrade erhalten. Diese Grenze betrifft ausschließlich Rechenzeit pro Aufgabe; die Lebensdauer des globalen Monty bleibt bis zum Shutdown unbegrenzt.

Zusätzliche Grenzen für taskgebundene Ausführungskontexte, Inbox, Fortsetzungen und stdout im Settings-Katalog getrennt ausweisen. Der globale Dienst muss seine begrenzte Kontrollverarbeitung auch nach einem Taskbudgetfehler fortsetzen können; dafür die Trennung von Taskverbrauch und Dienstaufwand technisch nachweisen.

Alle unterstützten Ressourcensettings gelangen ohne Prozess-, Gateway- oder Monty-Neustart in die laufende Runtime. Der Dienst übernimmt den zusammenhängenden, validierten Einstellungssatz mit Revision am nächsten sicheren Prüfpunkt, auch bei aktiven/suspendierten Aufgaben. Rust-seitige Ressourcenprüfung und Monty-Tracker verwenden denselben wirksamen Einstellungssatz samt Revision; getrennte Defaults oder veraltete Kopien dürfen keine abweichenden Grenzen erzwingen. Eine Revision gilt erst als vollständig übernommen, wenn beide Prüfpfade sie verwenden; ausstehende oder fehlgeschlagene Übernahme bleibt sichtbar. Daueränderungen erhalten die bereits verbrauchte Task-Rechenzeit, auch über Schritte und Fortsetzungen hinweg. Maximale Übernahme-/Control-Reaktionszeit und VM-Zeitscheibe vor Produktionsverdrahtung festlegen und messen. Reine Python-Schleifen dürfen die Verwaltungsroute oder den Control-Kanal nicht unbegrenzt blockieren. Persistenz und Runtime-Acknowledge können scheitern: gespeicherte Zielrevision, wirksame Revision und fehlgeschlagene/ausstehende Übernahme anzeigen; Wiederanlauf übernimmt nur gültige Werte. Eine UI-Speicherbestätigung allein beweist keine wirksame Übernahme.

**Verbindliche Tokenbudget-Regel:** `token_budgets_enabled` ist standardmäßig `false`. Bei ausgeschalteten Tokenbudgets keine künstliche Begrenzung von Retrieval, Vorwissen, History oder Taskverbrauch. Tokenzählung zur Beobachtung bleibt möglich. Technische Kontext- und Ausgabelimits des verwendeten Modells bleiben bestehen. Alle Verbraucher beachten denselben globalen Schalter; feste Konstanten wie `RETRIEVAL_TOKEN_BUDGET` oder `PRIOR_KNOWLEDGE_TOKEN_BUDGET` dürfen ihn nicht umgehen. Ressourcenlimits für Rechenzeit, Allokationen und Speicher bleiben eigenständige Regeln.

**Gemeinsame Dauerrevision für Rust und Monty:** Änderungen von `max_duration_secs` im WebUI werden als ein validierter, revisionsgeprüfter Instanzsatz veröffentlicht. Rust-Supervision und Monty-Tracker lesen dieselbe wirksame Revision und denselben taskbezogenen Verbrauch; keine unabhängigen Startzeitkopien, per-Chat-Werte oder nacheinander aktualisierten Grenzwerte. Aktive Aufgaben übernehmen die Änderung am begrenzten Prüfpunkt ohne Verbrauchsreset. Eine Senkung unter den bereits verbrauchten Wert beendet die betroffene Aufgabe nach dem Taskfehlervertrag. Externe Aufrufdeadlines bleiben separat; eine Rust-Wall-Clock-Timeout-Kopie des Monty-Rechenzeitlimits darf Idle-/Auth-/Tool-Waits nicht als Task-Rechenzeit verbuchen. Die API bestätigt die gemeinsame Übernahme erst nach Runtime-Acknowledge, andernfalls zeigt sie ausstehende/fehlgeschlagene Übernahme. Caller-Level-Tests ändern den Wert während aktiver Ausführung und während eines externen Waits und prüfen die Revision und Wirkung auf beiden Seiten.

**Nachgewiesene API-Lücke und Upgrade-Auswirkung:** Im bisherigen Monty v0.0.16 zählt `LimitedTracker` Allokationen über seine Lebensdauer; `set_max_duration` setzt die Startzeit zurück, öffentliche Setter für Speicher/Allokationslimit fehlen. In 1.0.0 gelten stattdessen der konkrete ResourceTracker, Feed-/Turn-Zeit und Allocator-Speicherprüfung gemäß Upgrade-Voraussetzung; auch dort setzen die Zeitsetter Akkumulatoren zurück. Vor dem Cutover einen auf der gewählten 1.0.0-API nachgewiesenen Engine-/Hosting-Vertrag mit erhaltener Verbrauchsrechnung, Taskzuordnung, Live-Grenzen und Zugriff an allen verwendeten Fortschrittszuständen implementieren und testen. Keine erfundenen Reset-/Snapshot-APIs, unbegrenzten Limits oder neue globale VM pro Task als Ersatz verwenden.

**Verbindliche Reihenfolge:** Betroffene Verbraucher, Kontext-/Policy-Verträge und Migrationsentwurf müssen feststehen. Monty-Upgrade samt Ressourcen-Disposition sowie Fortsetzungs-, Fairness- und Live-Limit-Machbarkeit zuerst nachweisen; dann den Produktionsdienst verdrahten. Temporäre alte SQL-/Scope-Typen hinter bestehenden Adaptern halten; produktive Daten erst über Phase 6/8 migrieren. Jeden Schritt mit Caller-Level-Tests abschließen. VM-Lebenszyklus ist Infrastruktur; Aufgaben bleiben Recipe-/PythonCode-gesteuert.

1. **Lebenszyklus und Ownership als neutralen Vertrag festlegen.** In `brassclaw_turns/src/run_profile/driver.rs` den vorhandenen Port zu einer Vorgangsübergabe an den globalen Dienst weiterentwickeln, ohne DB-, Engine- oder HostRuntime-Implementierungen zu importieren. Komposition besitzt einen privaten Service-Handle mit Inbox, Status und Shutdown/Join; genau eine Tokio-Service-Task besitzt und treibt die globale `MontySession`. Keine zweite Instanz in WebUI, REPL, MCP oder Triggern; kein `SessionRegistry<TurnScope, MontySession>` mehr als Orchestrator-Owner. Lebenszyklus mindestens `Starting → Ready → Stopping → Stopped`, zusätzlich `Failed`; Run-Ergebnisse sind separate Zustände. Wenn mehrere Produktadapter in derselben Runtime hängen, teilen sie denselben Handle. Einen zweiten Prozess auf derselben Instanzablage vor Dienststart durch eine Instanz-Lock/Lease mit sauberem Crash-Release verhindern; mehrere unabhängige Instanzablagen bleiben zulässig. Erst vorhandene Instanz-/Embedded-PG-Locks prüfen, keine zweite Lock-Konvention erfinden.

   Instanzidentität auch bei externer PostgreSQL-DB eindeutig festlegen. Mehrere Prozesse mit verschiedenen lokalen Pfaden dürfen denselben logischen Instanzbestand nicht parallel besitzen. Den passenden DB-/Instanzlock wiederverwenden; ein Embedded-PG-Datenverzeichnislock allein beweist keinen exklusiven Orchestratorbesitz.

2. **Boot-Prerequisites aus der WebUI herausziehen.** In `webui.rs` die bestehenden `seed_*`, Queue-Recovery, Content-Integritätschecks und `init_*`-Ladungen zu genau einer gemeinsam genutzten Kompositionsinitialisierung extrahieren. Ihre Abhängigkeiten und Reihenfolge erhalten; `BootedDb` bleibt der Migrationsnachweis. `runtime.rs`, CLI `serve` und `repl` auf explizites Bauen/Initialisieren/Starten umstellen. Ziel: DB/Migrations → Seeder → Integritätsprüfung → Host-/Component-/Kohai-Ports → verifizierter Orchestrator-Code → globale VM → initialer Work-Wait-Handshake → Worker/Produzenten → Ingress/Readiness. Für erforderliche Komponenten Fehler weitergeben statt nur warnen. `build_webui_services` darf keine nachträgliche Voraussetzung für Monty mehr herstellen. Worker, Trigger-Poller und Kanaladapter dürfen vor dem Handshake weder neue Arbeit claimen noch Side Effects ausführen. Jeder Zwischenfehler räumt bereits erworbene Locks/Tasks/DB-Ressourcen auf.

3. **Thread-freien VM-Boot ermöglichen.** `prepare_monty_session`, `MontySession::new` und `build_orchestrator_inputs` in `engine/src/executor/orchestrator.rs` sind heute an `Thread` und dessen Kontext gebunden. Einen neutralen Boot-Input mit Instanz-/Protokollversion und leerem Taskzustand einführen; keinen Dummy-Chat, Dummy-Run, Betreiber-Grant oder fingierten Userprompt anlegen. `basic_mode.py` muss vor der ersten Arbeit ausschließlich zulässige Boot-/Wait-Operationen ausführen; auch das bisher vor `await_next_turn()` stehende `check_signals()` darf keinen fiktiven Run verlangen. Taskgebundene Tools ohne aktiven Kontext verweigern. Die einzige globale VM beim Boot kompilieren/starten und bis zu ihrem ersten Eingabe-Wait treiben. Fehlender/ungültiger Code, Integritätsfehler, Boot-Timeout, frühes `FINAL` oder fehlender Handshake verhindern Ready; die erste Chatnachricht darf diese Fehler nicht erst entdecken.

4. **Work-/Control-Ereignisse und begrenzte Inbox definieren.** Eingaben tragen Protokollversion, Conversation-/Thread-/Turn-/Run-ID, genaue AcceptedMessageRef, Reply-Binding, Worker-Claim-/Attempt-Identität und Deadline. Payloadauflösung erfolgt im Host-Adapter. Control unterscheidet Run-Abbruch, externe Auth-Fortsetzung, Child-/Tool-Ergebnis, Settingsrevision und Shutdown. Python erhält validierte Monty-Werte; unbekannte Versionen/Typen scheitern explizit. Leere Nachricht, geschlossener Sender und Run-Stop bedeuten kein globales `FINAL`. Begrenzte Workqueue, Backpressure und garantierte Control-Kapazität/Priorität unter Last festlegen. Die persistente Turn-Ablage bleibt Arbeitsquelle; die Inbox ist Transport und kein zweiter dauerhafter Scheduler.

   **Recipe-Zustand gehört Monty.** IBS kompiliert das Recipe in ein `BuildInstruction`; Monty führt die Schritte aus und hält benötigte Zwischenwerte im jeweiligen Recipe-Ausführungskontext. Kein verpflichtendes leeres `{}` pro PythonCode-Schritt und kein Zusammenlegen von Schritten als Ersatz für fehlende Zustandsweitergabe. Bei einer Child-VM oder einem Child-Prozess übergibt Monty die benötigten Eingaben und übernimmt zurückgegebene Ergebnisse in denselben Kontext. Laufzeitdaten als validierte Werte transportieren, nicht in Python-Quelltext interpolieren. Unabhängige Vorgänge und Attempts bleiben getrennt; Fortsetzung nach Waits erhält den zugehörigen Kontext, Retry-/Checkpoint-Restore ist ausdrücklich validiert und beendet/abgebrochenes Task-State wird freigegeben. Durch den Produktions-Caller eine Kette A → B mit B als Verbraucher von A nachweisen, einschließlich Child-Ausführung, externer Wartephase, interleavtem fremden Task und veraltetem Attempt. Die bisherige Fresh-Step-Ausführung ist eine zu behebende Implementierungslücke.

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

9. **Signale, Replies und Fehler voneinander isolieren.** Signaladressierung mindestens durch Run-ID + Attempt ersetzen/ergänzen; ein verspätetes `Stop` für Run A darf nicht den nächsten Run desselben Chats oder die globale VM beenden. Reply-Persistenz aus `persist_new_assistant_messages` weiterverwenden, aber am konkreten Task/Message/Reply-Binding festmachen und gegen Doppelschreiben absichern. Vorgang erst nach bestätigter Persistenz abschließen; die bestehende Turn-/Event-Transition genau einmal anwenden. Fehler eines Tasks lässt den Dienst bereit, sofern die VM nachweislich konsistent ist. Ist sie beschädigt, `Failed` melden, Aufnahme/Dispatch sperren und Recovery beaufsichtigen. Den alten `SessionGuard::Drop` mit unüberwachtem asynchronem Re-Park nicht auf eine globale Singleton-VM übertragen.

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

**Pflichttests für Phase 3a (durch den Produktions-Caller):**

| Fall | Nachzuweisendes Verhalten |
|---|---|
| Monty-1.0.0-Upgrade | Beide Dependencies/Lockfile konsistent; unterstützte Build-Toolchains bestehen. Recipe-/Resume-/Hostfehler-Kompatibilität, aktive Rechenzeit über mehrere Feeds, erhaltene Live-Revisionen, Allokations-Disposition und tatsächliche Allocator-Speicherbegrenzung sind durch Caller-Level-Tests belegt; alte Fortsetzungen werden vor Versionswechsel reconciliiert. |
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
7. Task-/Attempt-/Continuation-/Reply-Beziehungen sowie Recipe-UUIDs, Varianten, `step_link`, Komponenten-Includes, `dependency_registry` und Intent-Zuordnungen vollständig erhalten bzw. konsistent remappen. Migrationsprüfung weist jede Referenz auf eine vorhandene passende Revision nach; keine dangling UUIDs oder durch Konflikte überschriebene Varianten. Kohai-Prefix und Caches aus dem migrierten validierten Bestand erneuern. Nur notwendige Felder ergänzen; keine zweite dauerhafte Turnqueue oder persistente `BuildInstruction`-Tabelle. RAM-Sitzungen sind keine DB-migrierbaren Snapshots.
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

   Die neue Generation erst veröffentlichen, wenn die zugehörigen Komponentenreferenzen, Matching-Daten und erforderlichen Prefix-/Cacheartefakte verfügbar sind. Bis dahin bleibt die bisherige Generation aktiv; ein Fehler lässt sie nutzbar. Ein Vorgang erhält eine konsistente Generation, statt neues Matching mit altem Komponenten-/Prefixinhalt zu mischen. Globale Toolsettings werden unabhängig davon vor jedem Dispatch aktuell geprüft.

   **Systemkomponenten:** geprüfte Seeds und Checksums erhalten. Betreiberänderungen als explizite eigene Revision/Kopie mit definiertem Override und Q1/Q2 behandeln; keine stillen Body-/Checksummutationen. Boot/Repair darf Overrides und deaktivierte Intent-Zuordnungen nicht reaktivieren oder überschreiben. Persistente Override-/Deaktivierungsregeln und Verhalten bei Seed-Upgrades vor Implementierung festlegen und testen. Geschützte Root-Orchestratorrevisionen wechseln weiterhin nur nach kontrollierter Reconciliation; gewöhnliche Intents/Recipes werden ohne Dienstneustart aktiv.
5. **Auswahl im Chat-Verlauf sichtbar machen:** für den konkreten Vorgang die tatsächlich verwendete Komponente, das Recipe, die Variante und Tier 0/1 bzw. den No-Match-/Tier-2-Pfad anzeigen. Die Auswahl aus dem ausgeführten Vorgang übernehmen, nicht nachträglich mit dem aktuellen Intent-Bestand neu berechnen. Ein Recipe kann LLM-Schritte enthalten: Nur Tier 0 bedeutet null LLM-Aufrufe; Tier 1 führt die vorgesehenen LLM-Schritte aus.

### 8.3 Abnahme

- Alle Intents sind über die paginierte Liste und ihre Komponenten-/Variantenzuordnung einsehbar; auch Bestände mit mehr als 500 Einträgen bleiben vollständig erreichbar.
- Hinzufügen, Ändern, Löschen und Variantenwechsel eines einzelnen Beispiels aktualisieren Komponenteninhalt und Matching-Bestand konsistent; kein veralteter Ausdruck oder verlorener `step_link` bleibt zurück. Fehlgeschlagene Änderungen erzeugen keine Teilzustände.
- Mehrfachzuordnungen desselben Ausdrucks, konkurrierende Editoränderungen und Draft-/Aktivierungsfehler erzeugen keine still überschriebenen Varianten. Auswahl verwendet nur den vorgesehenen aktiven validierten Stand.
- Der Matching-Test zeigt die gleiche Auswahl und Variablenauflösung wie der produktive Pfad, ohne Tools auszuführen oder produktive Matching-/Lerndaten zu verändern.
- Review-pflichtige Änderungen werden erst nach dem erforderlichen Validierungsweg aktiv; Q2 bleibt menschlich. Freigegebene Änderungen wirken ohne Neustart auf folgende Matching-Vorgänge.
- Der Chat-Verlauf zeigt die tatsächlich verwendete Auswahl auch dann korrekt, wenn Intents oder Recipes später bearbeitet werden. Tier-0-Recipes laufen ohne LLM; Tier-1-Recipes führen ihre vorgesehenen LLM-Schritte aus.
- Bestehende validierte Komponenten, Recipe-Varianten, UUID-Verweise und gelernte Intent-Einträge bleiben beim Umbau erhalten; Betreiberänderungen überstehen Boot/Repair gemäß der festgelegten Systemkomponentenregel.
- Ein suspendierter Vorgang verwendet seine bisherigen Komponentenrevisionen, ein folgender Vorgang den neuen aktiven Matching-/Komponenten-/Prefixstand; Toolberechtigungen folgen unabhängig davon stets der aktuellen globalen Einstellung.

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

**Abnahme:** Frische und bestehende Instanzen verwenden vom ersten Providerzugriff an ausschließlich PostgreSQL. Tests laufen ohne `providers.json` und ohne Provider-Env-/Datei-Fallback: Boot, CLI und WebUI zeigen denselben Bestand; DB-only hinzugefügte Anbieter sind nutzbar; alle initialen, früheren Builtin- und benutzerangelegten Provider lassen sich über UI/API ausblenden/deaktivieren, einschließlich des letzten Providers. Ihre Definitionen bleiben in der DB und lassen sich über „Provider hinzufügen“ mit derselben ID ohne Duplikate wieder aktivieren. Inaktive/ausgeblendete Provider erscheinen nicht in aktiven Listen und werden nicht zur Ausführung ausgewählt. Das Entfernen aktiver Provider löst ihre Auswahlreferenzen konsistent auf; danach bleibt die Betreiberverwaltung erreichbar und LLM-bedürftige Vorgänge melden fehlende Konfiguration. Änderung, Ausblendung/Deaktivierung und ausdrückliches Wiederhinzufügen funktionieren revisionsgeprüft; Neustart/Upgrade erhalten Betreiberentscheidungen ohne automatische Reaktivierung. Wiederhinzufügen prüft Konfiguration und Authreferenzen und stellt alte aktive Auswahlen nicht ungefragt wieder her. Modell-/Auth-/Embeddingpfade sowie Secretauflösung funktionieren weiter. Fehlende Konfiguration und DB-Fehler sind unterscheidbar. Backup/Restore und Migration erhalten alle Provider-/Auswahl-/Secretreferenzen. Der Repository-Scan weist keine produktiven Dateikatalog-/Rust-Seeder-Verbraucher mehr nach; historische Migrationen bleiben unverändert.
