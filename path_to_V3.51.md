# Path to V3.51 — Multiplattformfähigkeit

Stand: 2026-11-16. **Umsetzungsplan; alle Schritte offen.** Dieses Dokument
belegt weder Plattformunterstützung noch Validierung, Aktivierung oder Release.
**Priorität des Nutzers — 2026-11-16:** erst als allerletzten Plan umsetzen.
Aktuelle CI-Reparaturen, `unnecessary_stuff_v3_at_bob1.md` und die bestehenden
v3-/Bob-Arbeiten haben Vorrang. Die Veröffentlichung dieses Plans startet keine
seiner Implementierungsschritte.
Der Auftrag dieses Plans ist die Umsetzung auf Linux, macOS und Windows durch
die vorhandenen Architektur-, Validierungs-, Katalog- und Laufzeitverantwortlichen.

## Ziel und verbindliche Leitplanken

Universelle Komponenten müssen auf allen ausdrücklich unterstützten Zielsystemen
bei denselben fachlichen Eingaben und erfüllten Voraussetzungen dieselben
zugesagten Ergebnisse, Fehlerklassen und Effekt-, Sicherheits- und
Wiederaufnahmegarantien liefern. Native Implementierungen dürfen sich unterscheiden;
ein Adapter darf den gemeinsamen Vertrag nicht stillschweigend abschwächen.
Plattformbedingte Unterschiede müssen im Vertrag ausdrücklich benannt sein;
sie dürfen die zugesagte gemeinsame fachliche Bedeutung und Sicherheitsgarantien
nicht aufheben. Wo das nicht möglich ist, ist die betreffende Nutzung spezifisch
zu deklarieren. Identisches Verhalten meint die vereinbarte Semantik, nicht
identische absolute Pfade, Binärbytes oder native Diagnoseformulierungen.

**Universell** bezeichnet die fachliche Geltung eines Vertrags über die festgelegte
Supportmatrix. Es bedeutet weder ein einziges Binärartefakt noch unbegrenzte
Unterstützung beliebiger Betriebssysteme und ABIs. **Spezifisch** bezeichnet eine
ausdrücklich begrenzte Geltung, etwa für Windows, eine ABI, einen Dateisystemtyp
oder eine bestimmte Shell. Voraussetzungen und Einschränkungen sind prüfbar zu
deklarieren; unbekannte Fähigkeiten gelten nicht als vorhanden. Universelle
Workflows dürfen spezifische Adapter nur mit nachgewiesener gemeinsamer
Vertragserfüllung verwenden. Spezifische Abhängigkeiten dürfen eine universelle
Deklaration nicht verdeckt einschränken.

Maßgeblich bleiben [AGENTS.md](AGENTS.md), die
[Entwicklungspolitik](docs/development-policy.md), [recipe.md](recipe.md),
[skills.md](skills.md), [tools.md](tools.md), [toolskills.md](toolskills.md),
[simplified_v3.md](simplified_v3.md) und der
[Implementierungsstatus](docs/implementation-status.md).
Dieser Plan ergänzt [path_to_v3.5.md](path_to_v3.5.md) und die Bob-Pläne;
er ersetzt keine ihrer offenen Voraussetzungen oder Abnahmen.
Die dort aufgenommenen [Sempai-v3.5-Vorgaben](docs/plans/sempai_v3.5.md) und
[Connections-v3.5-Vorgaben](docs/plans/simplify_connections_v3.5.md) bleiben im
betroffenen Umfang verbindlich. V351 ergänzt deren Owner um Plattformarbeit;
es ersetzt keine offenen V35-Gates und eröffnet keine parallelen Implementierungen.

- Recipes und wiederverwendbare Skills/PythonCode behalten die fachliche
  Ablaufsteuerung. Rust ergänzt nur erforderliche native Primitive und
  Laufzeit-, Persistenz-, Binding-, Lifecycle- oder Kernelunterstützung.
  Vor jedem Paket tatsächliche Caller, wiederverwendete Komponenten und fehlende
  technische Unterstützung festhalten. Ein Recipe-Wrapper um einen weiterhin
  fachlich steuernden Rust-Workflow erfüllt v3.5 nicht. Bestehende Interfaces
  zuerst erweitern. Neue gemeinsame Konsolidierungsabstraktionen brauchen zwei
  echte semantisch passende Consumer; eine nachweislich fehlende Primitive darf
  mit einem erforderlichen Consumer beginnen. Keine künstlichen Consumer erzeugen.
- Ein globaler Monty, das bestehende Hostprotokoll und die bestehenden
  Katalog-, Deployment- und Recovery-Verantwortlichen bleiben erhalten.
- Ein kohärenter aktivierter, freigegebener Katalog wird am Taskstart gewählt.
  Exakte Revisionen, Assoziationen, Exporte und native Artefakte bleiben gepinnt.
  Plattformwechsel erlaubt keine neue Latest-Auswahl während eines Tasks.
- Globale aktuelle Tool-Policy, Sandbox, Secret-/Netzwerkgrenzen und technische
  Ressourcenlimits gelten vor jedem Dispatch auf jeder Plattform.
  Keine zusätzliche Plattform-Toolfreigabe oder run-/attemptbezogene Approval-
  Lease einführen. Neue betriebliche Limits über bestehende revisionsgeprüfte
  Settings mit Actor-ACK und sichtbarem Desired-/Effective-Zustand integrieren;
  keine versteckten Plattformquoten. Task-Rechenzeit, externe Wartezeit,
  Transportdeadline, Claim-Lebensdauer und Heap-/Frame-/Modellgrenzen bleiben
  getrennt. Tokenbudgets bleiben standardmäßig deaktiviert; Plattformwechsel
  aktiviert keine künstlichen Tokenkappen und ersetzt keine technischen Grenzen.
- Nur tatsächliches No-Match erreicht Tier 2. Fehler einer aktiven Auswahl,
  fehlende gepinnte Artefakte oder begonnene Recipe-Fehler sind keine Ausweichroute.
- Ausgelieferte kanonische Komponenten übernehmen ihre vor Auslieferung
  erworbene Validierung. Installation, Neustart, Upgrade und Umzug führen keine
  erneute Q1/Verhaltensprüfung oder zusätzliche installierte Human-Q2 aus.
  Technische Integritäts-, Bindungs-, Lade- und Migrationsprüfungen bleiben nötig.
  Installierte eigene Änderungen behalten ihre authored Q1/Verhalten/Human-Q2.
  Die Übertragung gilt nur im belegten Geltungsbereich; ein neues natives
  Zielartefakt benötigt seine Qualifikation vor Auslieferung. Ein Umzug macht
  vorhandene Nachweise nicht automatisch für ein anderes Ziel gültig.

## Ausgangslage und Zuständigkeiten

Die nachstehenden Beobachtungen sind begrenzte Quellenbefunde vom 2026-11-16
bei HEAD `3fa98f706131f28567fe15924f80b972cc0c6217`, keine Laufzeitabnahme.
Im Arbeitsverzeichnis liegen bereits andere, teils vorgemerkte Änderungen;
deren Inhalt und Nachweise sind vor späterer Implementierung erneut abzugleichen.

| Bereich | Vorhandener Ansatz / konkreter Anschluss |
| --- | --- |
| Plattform | [PlatformInfo](crates/brassclaw_common/src/platform.rs) enthält Version, Modell, Backend, Channels und Agent-Identität; kein technisches Zielprofil für Artefaktauswahl. |
| Pfade | [Gemeinsame Pfadhelfer](crates/brassclaw_common/src/paths.rs) lösen das Basisverzeichnis einschließlich `BRASSCLAW_BASE_DIR` auf; bestehende weitere Adapter und Aufrufer zuerst inventarisieren. |
| Dateisystem | [LocalFilesystem](crates/brassclaw_filesystem/src/local.rs) enthält Mount-/Containment-Auflösung und Ersetzen; gemeinsame Garantien sind gegen native Semantik zu prüfen. |
| Secrets | [Keychain](crates/brassclaw_secrets/src/keychain.rs) hat native macOS-/Linux-Zweige und einen Fallback für andere Systeme, keinen entsprechenden nativen Windows-Zweig. |
| Datenbankpaket | [build.rs](crates/brassclaw_embedded_postgres/build.rs) kennt mehrere PostgreSQL-Ziele einschließlich Windows; pgvector-Quellbau ist auf Linux/macOS begrenzt, leere Ersatzarchive sind möglich. |
| Release | [Release-Workflow](.github/workflows/release.yml) enthält Linux x86_64 GNU und macOS ARM64/x86_64, keine Windows-Zeile; Paketaufbereitung und Installationschecks sind Unix-orientiert. |
| Builds | [Combined Building](combined-building_v3_at_bob3.md) enthält Host-/Target-Trennung und bestehende Modal-Anschlüsse. Linux-Testbetrieb und plattformspezifische Abnahme nur im dort tatsächlich belegten Umfang übernehmen. |

| Verantwortung | Bestehender Owner |
| --- | --- |
| Fachliche Geltung, Review-Subjects, Nachweisumfang und Herkunft | [Bob B1](Repair_bob_1_v3_at_k3.md), bestehender Validator-/Installationsowner |
| Zielkompatible Katalogauswahl, Publikation, Artefaktpins und sämtliche retained Leser | [Bob B2](Repair_bob_2_v3_at_k3.md), bestehende Composition-/IBS-/Retentionsowner |
| Technische Erkennung, native Adapter, globaler Prozessbetrieb, Umzug und WebUI | [Bob B3](Repair_bob_3_v3_at_k3.md) mit den jeweiligen Crate-Ownern |
| Build-/Targetpakete, CI und Modal | B3 und [Combined Building](combined-building_v3_at_bob3.md), vorhandene Build-/Release-Owner |

## Umsetzungsschritte

### V351-01 — Gemeinsamen Plattformvertrag und Supportmatrix festlegen

- [ ] In den vier Komponentenguides und allen drei Bob-Plänen die oben definierte
  Universal-/Spezifisch-Geltung konsistent verbindlich machen und gegenseitig
  verlinken. Den fachlichen Vertrag einer Nutzung, die Kompatibilität ihrer
  nativen Implementierung und die Geltung ihrer Nachweise getrennt definieren.
  Die folgende Dokumentzuordnung ist die vollständige Mindestabdeckung dieses
  Schritts; es entsteht keine zusätzliche Architektur- oder Freigabestelle.

  | Dokument | Erforderliche Vertragsergänzung |
  | --- | --- |
  | [recipe.md](recipe.md) | Geltung des unterstützten Workflows und seiner Varianten, Voraussetzungen und gemeinsame Ergebnis-/Fehler-/Effektsemantik; spezifische Varianten machen andere Varianten nicht automatisch universell. |
  | [skills.md](skills.md) | Geltung einer einzelnen Tool-Nutzung einschließlich zugehöriger PythonCode-Exporte und transitiver Hilfen; fachliche Portabilität unabhängig von identischen nativen Bytes beschreiben. |
  | [tools.md](tools.md) | Gemeinsamer Primitivvertrag und spezifische Implementierungs-/ABI-/Ladeanforderungen; alle nativen Implementierungen derselben Toolidentität bleiben an deren aktuelle globale Policy gebunden. |
  | [toolskills.md](toolskills.md) | Prüffähige Binding-/Adapterkompatibilität und Voraussetzungen; Metadaten führen weder Erkennung noch Code aus und erteilen keine Berechtigung. |
  | [Bob B1](Repair_bob_1_v3_at_k3.md) | Fachliche Nachweisgeltung und konkrete native Prüfherkunft trennen; A-/S-Verträge und historische Bedeutung erhalten. |
  | [Bob B2](Repair_bob_2_v3_at_k3.md) | Geltung und native Kompatibilität aus einem kohärenten Katalog wählen, exakte Artefakte pinnen und sämtliche Publikations-/Retentionsleser berücksichtigen. |
  | [Bob B3](Repair_bob_3_v3_at_k3.md) | Technische Fakten und Adapter-Lifecycle liefern; Umzug, Wiederaufnahme und Anzeige konsumieren B1-/B2-Verträge, ohne selbst Freigabe oder Ersatzpins zu erfinden. |

- [ ] Zielmatrix mindestens für `x86_64-unknown-linux-gnu`,
  `aarch64-apple-darwin`, `x86_64-apple-darwin` und
  `x86_64-pc-windows-msvc` als **geplante Produktziele** ausarbeiten. Die
  gegenwärtige Release-Matrix ist ein Ausgangspunkt, kein Nachweis vollständiger
  Unterstützung. Weitere bekannte Downloadtargets gelten dadurch nicht
  automatisch als unterstützt; ihre Aufnahme erfordert eine eigene Matrixzeile.
  Je Zeile mindestens festlegen:

  - Betriebssystem-/Versionsbereich, Prozessarchitektur und ABI einschließlich
    benötigter System-/C-Laufzeit; Build-Host, Target und native Testumgebung
    getrennt angeben. Emulation ausdrücklich ein- oder ausschließen.
  - Unterstützte Produkt-/Featurekonfiguration, Installations- und Dienst- oder
    Benutzerkontext, erforderliche Fähigkeiten, Dateisystemgarantien,
    Secretstore sowie passende PostgreSQL-/pgvector-/Monty-Workerpakete.
  - Geltungsbereich der universellen Basis, ausdrücklich spezifische Nutzungen,
    Quellen-/Buildstatus, native Test-/Paketnachweise, verbleibende Lücken,
    zuständiger Owner und Freigabegate. Offene Versionsgrenzen sind offen zu
    kennzeichnen und vor Supportzusage zu entscheiden, nicht zu schätzen.

- [ ] Regeln für die Zusammenführung von Geltung und Voraussetzungen festlegen:
  ausgewählte Recipe-Variante, Skill/Code, transitive Hilfen, ToolSkill und Tool
  müssen gemeinsam auf dem Ziel gültig sein. Eine universelle Nutzung verlangt
  für jedes zugesagte Ziel eine vertragsgleiche qualifizierte Implementierung;
  ein nativer Adapter selbst darf spezifisch bleiben. Keine UUID-Duplikate oder
  zusätzlichen Recipes allein wegen eines anderen OS anlegen, wenn bestehende
  Identitäten und Verträge die Implementierungen korrekt abdecken.
  Eigenständige pure PythonCode bleibt eine normal qualifizierbare Komponente
  beziehungsweise explizite Abhängigkeit; weder künstliche Toolbindung noch
  Zero-Tool-Skill oder zwingendes eigenes Recipe dafür schaffen. Allgemeine
  Pure-Approval-Unterstützung bleibt beim vorhandenen Validatorowner.
- [ ] Im vorhandenen Schema-/Katalogowner die Darstellung von Geltung,
  Voraussetzungen, Implementierungskompatibilität und Nachweisbezug entwerfen.
  Tatsächliche Writer, Reader und Migrationen inventarisieren; keine erfundenen
  JSON-Felder als bereits vorhandene Schnittstelle dokumentieren. B1 verantwortet
  Nachweisbedeutung/-herkunft, B2 das gemeinsame Katalog-/Selektionsformat und
  seinen Cutover, B3 die authentischen technischen Fakten und nativen Adapter.
- [ ] Fehlende oder unbekannte Geltung darf nicht als universeller Nachweis gelten.
  Für alte Definitionen/Nachweise explizite Leseregeln festlegen, die deren
  ursprüngliche Bedeutung und gültige Task-Pins bewahren. Neue Bedeutungen in
  neuen unveränderlichen Revisionen beziehungsweise versionierten Formaten
  über den vorhandenen Upgradeowner einführen; freigegebene Bytes und
  historische Records nicht nachträglich annotieren oder umdeuten.
- [ ] Mit B1/B2 die Publikationsgrenze festhalten: neue Geltungs-/Auswahlprofile
  erst nach angeschlossenen Produzenten und allen erforderlichen Lesern
  aktivieren. Fehlende Zielkompatibilität einer aktivierten Auswahl ist ein
  technischer Fehler vor Effekten; kein stiller Downgrade oder Tier-2-Fallback.
  Der Vertragsentwurf verlangt noch keine Implementierung der Folgeschritte.

**Owner:** B2 koordiniert die gemeinsame Vertragsdarstellung; B1 und B3 liefern
ihre oben abgegrenzten Teile. **Abhängigkeit:** Quellen- und Vertragsabgleich mit
den bestehenden Ownern; V351-02 und V351-09/10 konsumieren den Entwurf. Die
native Implementierung und ihre Belege sind keine Voraussetzung, um diesen
Dokumentations-/Entwurfsschritt abzuschließen, wohl aber für spätere Freigaben.

**Abnahme:** alle sieben Dokumente enthalten widerspruchsfreie Ergänzungen;
eine ausgefüllte Sollmatrix unterscheidet geplanten Support und tatsächlich
belegten Iststatus; der gemeinsame Formatentwurf nennt tatsächliche
Producer/Reader, Altbestandsregeln, Owner und spätere Cutovergates. Der
Vertragsreview behandelt mindestens: universelle Nutzung mit verschiedenen
nativen Artefakten; spezifische Shell-Nutzung; fehlende ABI/Fähigkeit;
Altbestand ohne Plattformangabe; unveränderter wartender Task nach OS-Wechsel.
Diese Fälle erhalten eindeutige erwartete Entscheidungen und Folgeschritt-Owner;
hier sind keine neuen Builds oder Laufzeittests erforderlich.

**Schrittaudit — 2026-11-16:** Plantext gegen die vier Komponentenguides, die
B1/B2/B3-Zuständigkeitsverträge und die vorhandene Release-Zielmatrix geprüft.
Korrigiert wurden die Trennung von Geltung/Kompatibilität/Nachweisen, die
unzureichende Matrixdefinition, die bisher offene Altbestands-/Publikationsgrenze
und die nicht konkret belegbare Dokumentationsabnahme. Nur dieser Plan wurde
angepasst. V351-01 bleibt offen: die sieben Vertragsdokumente, ausgefüllte
Supportmatrix und der angeschlossene Formatentwurf sind noch zu bearbeiten.
Dies war das erste Teilaudit; das spätere Gesamtaudit ist am Ende dokumentiert.

### V351-02 — Technische Umgebung im Laufzeitowner erkennen

- [ ] Betriebssystem, Prozess-/Hostarchitektur, Ziel-ABI und relevante native
  Fähigkeiten zuverlässig erfassen. Emulation und Build-Host von der Umgebung
  unterscheiden, in der ein Artefakt tatsächlich geladen oder ausgeführt wird.
  Buildseitige Targetmetadaten mit Runtime-Fakten abgleichen; eine ABI nicht
  allein aus dem OS-Namen ableiten. Betriebssystemprozesse, dynamisch geladene
  Bibliotheken und Datenbankmodule haben jeweils eigene Kompatibilitätsgrenzen.
- [ ] Vorhandene Runtime-/Host-Adapter erweitern; `PlatformInfo` als
  Agent-Identität nicht zur alleinigen Sicherheits- oder Auswahlquelle machen.
  Erkennung ohne frei erfundene Identitätswerte oder LLM-Schätzung durchführen.
- [ ] Fehlende, unbekannte und verfügbare Voraussetzungen unterscheiden.
  Veränderliche Fähigkeiten vor betroffenen Operationen technisch erneut prüfen;
  beobachtete Verfügbarkeit erteilt keine Tool-Berechtigung.
  Bestehende Host-/Kernelprüfungen wiederverwenden; keinen periodischen
  Systemscanner oder neuen Plattform-Policy-Dienst einführen. Prozessglobale
  Fakten von aufrufabhängigem Benutzer-/Dienstkontext und Mountfähigkeiten trennen.

**Owner:** B3/Host-Runtime; B2 konsumiert das technische Profil.
**Abnahme:** native Erkennung auf allen Matrixzielen, einschließlich negativer
Fälle und abweichender Build-/Ausführungsumgebung.

### V351-03 — Verzeichnisse und portable Referenzen vereinheitlichen

- [ ] Daten-, Cache-, Konfigurations-, temporäre und Programm-/Workerverzeichnisse
  über vorhandene Pfadadapter auflösen; Präzedenz bestehender Overrides,
  Defaultpfade und Upgradeverträglichkeit ausdrücklich festhalten.
  Bestehende Defaults nicht allein zur Anpassung an OS-Konventionen verschieben;
  jede notwendige Verlagerung braucht eine verlustfreie Upgradeentscheidung.
  Den `/tmp`-Notfallpfad und relative Overrides im gemeinsamen Pfadhelfer prüfen.
- [ ] Komponenten und transportierbare Konfiguration auf logische Referenzen
  beziehungsweise kontrolliert aufgelöste Pfade umstellen. Maschinengebundene
  absolute Pfade als explizite lokale Bindung kennzeichnen; weder blind kopieren
  noch durch Textsubstitution in ausführbaren Code einsetzen.
- [ ] Leerzeichen, Unicode, Windows-Laufwerke/UNC-Pfade, getrennte Benutzer- und
  Programmverzeichnisse sowie schreibgeschützte Installationsorte behandeln.
  `ScopedPath`/`VirtualPath` behalten ihre virtuelle Grammatik; native
  Laufwerk-/UNC-Pfade nur innerhalb des zuständigen Host-/Mountadapters auflösen.
  Portabilität erweitert keine Mountgrenze und gibt keine rohen Hostpfade an
  Komponenten weiter. Workerauflösung weiterhin aus der vertrauenswürdigen
  Installation, ohne PATH-Suche oder vom Task gewählten Executablepfad.

**Owner:** B3/Pfad- und Installationsowner. **Abnahme:** identische logische
Referenzen werden auf jedem Ziel korrekt aufgelöst; bestehende Installationen
behalten Daten und wirksame Overrides.

### V351-04 — Dateisystemverträge über native Adapter erfüllen

- [ ] Für `LocalFilesystem` und alle genutzten Backends Verträge für Ersetzen,
  Verschieben, Konflikte/CAS, Sperren, Atomarität und zugesagte Dauerhaftigkeit
  präzisieren; Cross-Volume-Verhalten ausdrücklich festlegen.
  **Istgrenze:** `RootFilesystem` hat derzeit keine allgemeinen Move-/Lockmethoden;
  `LocalFilesystem::put` unterstützt für Byteeinträge `Any`/`Absent`, aber keine
  Versions-CAS. `atomic_write_file` nutzt Rename beziehungsweise Hardlink.
  Interne native Sperren von öffentlich zugesagten Operationen unterscheiden.
  Benötigte neue Primitive erst mit echten Aufrufern und dem vorhandenen
  `RootFilesystem`-/Capabilityowner entwerfen; keine parallele Filesystem-API
  oder vorgetäuschte Backendfähigkeit hinzufügen.
- [ ] Sichere Pfadauflösung einschließlich Symlinks, Windows-Reparse-Points,
  Junctions, Groß-/Kleinschreibung und TOCTOU-Risiken im bestehenden Owner
  durchsetzen. Native handlebasierte Mechanismen einsetzen, soweit der Vertrag
  sie benötigt; bloße lexikalische Normalisierung genügt nicht.
- [ ] Windows-Dateifreigaben und offene Handles korrekt behandeln. Fehlende
  Garantien als klassifizierten Fehler melden, keine stille Copy/Delete- oder
  ungesperrte Ersatzoperation unter einem stärkeren Vertrag ausführen.
  Fehler nach bereits erfolgtem Ersetzen/Verlinken dürfen nicht als sicher
  effektlos gelten: bestätigten oder ungeklärten Effekt und Cleanupfehler
  unterscheiden, damit kein automatischer Retry einen abgeschlossenen Effekt
  wiederholt. Atomare Sichtbarkeit ist keine Zusage von Crash-Dauerhaftigkeit.

**Owner:** Filesystem-/Kernelowner mit B3. **Abnahme:** dieselben Vertragstests
auf echten Linux-, macOS- und Windows-Dateisystemen, einschließlich konkurrierender
Operationen, Escape-Versuchen und wahrheitsgemäß klassifizierten Teil-/Cleanup-
fehlern. Fehlende Fähigkeiten bleiben explizit Unsupported, bis implementiert.

### V351-05 — Prozess- und Monty-Lifecycle portieren

- [ ] Bestehende Process-/Monty-Host- und Compositionpfade auf Unix-Annahmen
  prüfen. Programmauflösung, Argumentvektoren, Umgebung, Working Directory,
  Pipes und Handle-Vererbung plattformgerecht implementieren.
- [ ] Kindprozessgruppen, Abbruch, Exit-/Signalabbildung und geordneten Shutdown
  über native Adapter realisieren, etwa Unix-Prozessgruppen und Windows Job
  Objects bei entsprechendem Bedarf. Verwaiste Kinder und offene Ressourcen
  vermeiden; vorhandene Claims und Effect-Custody beibehalten.
  Den vorhandenen Worker-Transportowner für Korrelation, Frames, Credits,
  Control-Lane und Stop-Zustand verwenden. Neue OS-Pipes/-Handles rechtfertigen
  keinen zweiten Transportactor oder universelles RPC-Protokoll. Beobachterverlust,
  Verbindungsverlust, Task-Cancel und Workerdefekt getrennt behandeln; Kanalverlust
  beweist weder Abbruch noch Effektlosigkeit.
- [ ] Genau einen globalen Monty und das bestehende versionierte Protokoll
  erhalten. Workeridentität, Framing, Limits, Startbereitschaft und kontrollierte
  Recovery gegen die jeweils gepinnten nativen Artefakte prüfen.
  Gemeint ist ein supervisierter globaler Root-Owner, kein Verbot vorhandener
  explizit verwalteter Child-Interpreter/abhängiger Worker. Ihre Provenienz,
  Budgets und Claims erhalten; keinen zweiten unabhängigen Orchestrator schaffen.
  Native Ressourcen-/Sandboxmechanismen je Ziel anschließen oder fehlende
  technische Unterstützung vor Freigabe melden. Prozessüberwachung allein
  beweist keine Sandbox. Geordnete Beendigung von protokolliertem erzwungenem
  Abbruch unterscheiden; ein Task-Cancel darf nicht den globalen Root ersetzen.

**Owner:** B3/Processes/Monty-Host. **Abnahme:** tatsächlicher Start, Dispatch,
Cancel, Fehler, Shutdown und belegtes Prozessende auf allen Zielen; kein zweiter
unabhängiger Root-/Fallbackpfad und kein Replay nach unbekanntem Effektausgang.

### V351-06 — Universelle Workflows von impliziten Shells lösen

- [ ] Kanonische Komponenten und produktive Adapter auf Bash, `/bin/sh`, Unix-
  Programme, Shellquoting und pfadabhängige Befehle inventarisieren.
- [ ] Universelle Datei-, Prozess- und andere Primitive über vorhandene typisierte
  Tools und zugehörige Skills/PythonCode ausdrücken. Fehlende Primitive nur im
  zuständigen nativen Owner ergänzen; keine Rust-Workflow-Tools schaffen.
  Vorhandene Unterstützung durch tatsächlichen Dispatch/Adapter belegen.
  ToolSkill-Binding und aufgerufenen Skill-Export getrennt halten: ein UUID pro
  Komponentenschritt, unabhängige Toolaufrufe in getrennten Ausführungsschritten,
  nur die dokumentierte direkte Abhängigkeitsausnahme verwenden. Laufzeitdaten
  bleiben typisierte Eingaben und werden nicht zu Python- oder Shellquelltext.
- [ ] Tatsächliche Shell-Aufgaben als spezifisch deklarieren: Shell/Version,
  Aufrufvertrag, Daten-/Quotinggrenzen und Voraussetzungen. Kein stiller Wechsel
  zwischen Bash, PowerShell und `cmd.exe` mit abweichender Bedeutung.
  Vorhandene Shell-Workflows behalten ihren vorgeschriebenen Tier-1-Pfad;
  technische Plattformdeklaration allein stuft sie nicht auf Tier 0 um.
  Entwicklungs-/CI-Shellskripte von universellen Produktworkflows unterscheiden:
  native Buildadapter dürfen ihre Werkzeuge ausdrücklich voraussetzen.
  Neue/angepasste Komponenten zuerst als Recipe-/Reuseentwurf mit Exports,
  rekursiven Datenverträgen und Ergebnis-/Eingabekanten beschreiben. Fachliche
  Transformation und Abschlussentscheidung in kleinen PythonCode-Exporten
  halten; native Adapter authentisieren, dispatchen und sichern Effekte.
  Tier 0 durch echte Ausführung ohne Modellaufruf belegen, nicht durch ein Flag
  oder allein die Entfernung eines Shellbefehls.

**Owner:** Komponentenowner, B1 für Bedeutung und B2 für Auswahl.
**Abnahme:** universelle Referenzworkflows laufen ohne Unix-Shell; ausdrücklich
spezifische Shell-Aufgaben werden nur mit passenden Voraussetzungen ausgeführt.

### V351-07 — Windows-Secrets und sichere Neubindung ergänzen

- [ ] Nativen Windows-Backendzweig im bestehenden Keychain-Adapter ergänzen;
  geeigneten OS-Secretstore, Benutzer-/Dienstkontext, ACLs sowie Fehler- und
  Lockverhalten mit dem bestehenden Secretvertrag abstimmen.
  Der vorhandene Adapter speichert den Verschlüsselungs-Master-Key; Secretdaten
  bleiben in ihren bestehenden Stores. Die asynchronen Operationen
  `store_master_key`, `get_master_key`, `delete_master_key`, `has_master_key`
  und `resolve_master_key_material` korrekt anschließen, einschließlich
  Dependencies/Featuregates und der bestehenden Vorrangregel für den ausdrücklich
  gesetzten `SECRETS_MASTER_KEY`. Diese Konfiguration nicht mit einem stillen
  Klartext-Fallback verwechseln oder im Zuge der Portierung unbemerkt entfernen.
- [ ] Gespeicherte Referenzen, verschlüsselte Daten und maschinengebundene
  Schlüssel unterscheiden. Umzug über ausdrückliche Wiederanmeldung oder
  autorisierten sicheren Transfer mit Neubindung lösen; keine Klartextablage,
  Logausgabe oder stillen Fallback auf einen schwächeren Store einführen.
  Wiederanmeldung erneuert externe Credentials, entschlüsselt aber keine mit dem
  alten Master-Key geschützten Daten. Für deren Erhalt braucht es den sicher
  verfügbaren Originalschlüssel oder einen autorisierten entschlüsselnden
  Transfer mit geprüfter Neuverschlüsselung. Bei fehlendem Schlüssel nicht
  automatisch einen neuen erzeugen und vorhandene Daten unlesbar machen.

**Owner:** Secrets-/Auth-/Kernelowner mit B3.
**Abnahme:** echter nativer Store auf allen unterstützten Plattformen;
Windows-Roundtrip, fehlender Zugriff und Neubindung ohne Secretverlust/-offenlegung.
Headless-/Dienstkontexte ausdrücklich qualifizieren oder als nicht erfüllt
anzeigen; keine interaktive Desktop-Keychain pauschal als verfügbar annehmen.

### V351-08 — Vollständige PostgreSQL-/pgvector-Zielpakete herstellen

- [ ] `build.rs`, Archivprüfung, Extraktion, Installation und Lifecycle gemeinsam
  prüfen. Für jedes Supportziel PostgreSQL und passendes pgvector einschließlich
  Binärmodul, Control-/SQL-Dateien und benötigter Laufzeitbibliotheken bereitstellen.
  Integrität anhand geprüfter Herkunft, Checksummen und erwarteter Inhalte
  verifizieren; Archivgröße allein genügt nicht. Extraktion bleibt auf das
  Paketverzeichnis begrenzt und muss Links/Pfadtraversal auf jedem Ziel abweisen.
- [ ] Windows-pgvector mit passender PostgreSQL-/Compiler-/ABI-Kombination bauen
  oder aus nachvollziehbar geprüften Zielartefakten paketieren. Niemals Hostcode
  für das Target ausgeben oder fremde Targetprogramme versehentlich ausführen.
- [ ] Leere Ersatzarchive und Warnung-plus-Erfolg für benötigte Bestandteile
  entfernen. Fehlendes Pflichtpaket lässt den betreffenden Produktbuild scheitern.
  Ein bewusst eingeschränkter Build braucht ein ausdrücklich getrenntes Profil
  und darf nicht als vollständiges Release erscheinen.
- [ ] Init, Readiness, Neustart, Port-/Socketwahl, Konfiguration, Migration und
  geordnetes Stoppen unter gemeinsamen Verträgen implementieren.
  Insbesondere tatsächliche Aufrufer prüfen: `PgCtl::new` und `run_initdb`
  bilden bisher suffixlose Programmpfade, während `ensure_pg_extracted` bereits
  einen Windows-Zweig besitzt. `.exe`-Auflösung, `pg_ctl`-Optionen/Quoting,
  DLL-Ladepfade und TCP-/Socketverhalten zusammen am echten Paket prüfen.
  Vorhandene SQL-/pgvector-Anforderungen und Startupfehler nicht durch einen
  optionalen Warnpfad umgehen; bestehende Sicherheitskonfiguration nicht lockern.
  Bestehende gemeinsame PostgreSQL-Verbindungs-/TLS-Policy und Poolinjektion
  auch auf Windows verwenden; keinen OS-spezifischen NoTls-/Reconnectpfad
  schaffen. Notwendige getrennte Credentials/Datenbanken behalten getrennte
  Pools unter derselben Policy; Gesamtbedarf einschließlich Admin-/Migrations-
  und dedizierter Sessions gegen die Instanzkapazität prüfen. Standalone-
  Migrationen müssen dafür keinen Monty starten. Beim Shutdown erst Nutzer und
  eigene Sessions drainen, dann Embedded PostgreSQL stoppen. Ein verlorenes
  Commit-ACK erlaubt Readback unter dauerhafter Identität, kein blindes SQL-Replay.

**Owner:** Embedded-Postgres-/Buildowner mit B3.
**Abnahme:** vollständiges Paket auf jedem Ziel offline installieren;
dedizierte temporäre Instanz starten, `CREATE EXTENSION vector` und echte
Vektorabfrage ausführen, neu starten und Prozessende bestätigen. Sandboxbedingte
PG-Ausnahmen nur nach den bestehenden Freigabe- und Bereinigungsregeln nutzen.

### V351-09 — B2: Zielsystemgerechte Artefaktauswahl und Pins

- [ ] Technisches Laufzeitprofil, deklarierte Geltung und qualifizierte native
  Implementierungen im vorhandenen konsistenten Katalogabzug zusammenführen.
  Mehrdeutigkeit, fehlende Pflichtimplementierung und ABI-Konflikte vor Effekten
  ablehnen. Zielgeltung bei kohärenter Veröffentlichung/Vorbereitung festlegen,
  nicht durch einen zusätzlichen stillen Filter im Matcher. Die Regeln für
  bewusst zielgebundene Kataloge in V351-01 ausdrücklich definieren; eine
  eigentlich verfügbare universelle Nutzung darf bei Adapterfehlern nicht
  verschwinden. Defekte eines aktiven Katalogs als Fehler melden, nicht als
  No-Match oder Anlass für Tier 2.
- [ ] Exakte Implementierungsidentität, Revision, Checksumme, ABI-/Adaptervertrag
  und tatsächlich verfügbare unveränderliche Artefaktbytes pinnen. Ein Pfad oder
  gleicher Toolname genügt nicht. Universelle Verträge dürfen auf jedem Ziel
  andere dafür qualifizierte native Artefakte auswählen.
  Neueste aktive freigegebene Vertragsrevision beibehalten und daraus die
  eindeutig qualifizierte Zielimplementierung bestimmen. Unpassende neueste
  Verträge nicht durch ältere Revisionen ersetzen. Die Zuordnung aller nativen
  Dispatchaliasse zur stabilen Toolidentität und aktuellen Policy erhalten.
- [ ] Alle betroffenen Writer, Serializer, DB-Constraints, Hashes und retained
  Leser gemeinsam erweitern: ordinary, private Validator, Prefix, Child, Wait,
  Resume, MCP, Deployment und Recovery. Alte gültige Pins erhalten.
  MCP-Ankündigung und Taskaufnahme getrennt behandeln: gelisteter Commandvertrag
  wird vor Chat-Capture auf aktuelle Kompatibilität geprüft; nach Capture gilt
  die originale ordinary Taskauswahl. Kein direkter Plattform-MCP-Runner und
  keine Exposition roher Skills/Tools. Server- und requestbezogenen Discovery-/
  Verbindungslebenszyklus nach bestehendem Vertrag erhalten.

**Owner:** B2; hängt von V351-01/02 und passenden B1-Nachweisen ab.
**Abnahme:** kohärente Auswahl und Wiederaufnahme der passenden Originalpins auf
jedem Ziel, keine Zusage beliebiger OS-übergreifender Fortsetzung;
falsches OS/ABI, manipulierte Bytes und fehlende Artefakte scheitern geschlossen.
Keine Veröffentlichung eines neuen Profils vor vollständigem Reader-Cutover.

### V351-10 — B1: Fachliche und native Nachweisgeltung trennen

- [ ] Universelle fachliche Nachweise von spezifischen Adapter-/Artefaktnachweisen
  im bestehenden Review-/Approvalowner unterscheiden. Wiederverwendung nur bei
  belegter Vertrags-, Revisions- und Voraussetzungenkompatibilität zulassen;
  ein erfolgreicher Lauf auf einem OS qualifiziert kein ungeprüftes natives Backend.
- [ ] Tatsächliche Ausführungsplattform, Architektur/ABI, Konfiguration,
  Artefaktidentitäten und Prüfherkunft nachvollziehbar erhalten. Fachliche
  Geltung und beobachtete Ausführungsplattform sind getrennte Aussagen.
  Technische Herkunft im vertrauenswürdigen Host-/Evidenceowner erfassen, nicht
  aus einem vom Kandidaten gelieferten Plattform-/Erfolgsfeld übernehmen.
  Allgemeine fachliche Geltung braucht ein begründetes Abdeckungsmodell und
  passende native Voraussetzungen; reine Deklaration oder ein einzelner
  Beispieltest genügt nicht. Tool und ToolSkill bleiben eigenständig qualifizierte
  Voraussetzungen; S qualifiziert anschließend die exakte Skill-/Code-Nutzung.
- [ ] Historische Records unverändert lassen; fehlende Plattformherkunft nicht
  nachträglich erfinden oder historische Nachweise pauschal universalisieren.
  Neue Formate mit B2-Writern/-Lesern und Migrationsgrenzen verbinden.
- [ ] A-/S-Unterscheidung erhalten: authored own-functional Review mit Human-Q2;
  shipped Qualifikation vor Provisioning und exakte Übertragung bei Installation.
  Plattformprüfung begründet keine pauschale Neuqualifikation unveränderter
  Verbraucher oder installierte zusätzliche Human-Q2.
  Prüfablauf, fachliche Nachweisauswahl und Aggregation bleiben Recipe-/PythonCode-
  Arbeit; Rust authentisiert Subjects, native Beobachtungen und zulässige Writes.
  Kein plattformspezifischer Rust-Validatorworkflow, der fachliche Regelabdeckung
  oder erfolgreichen Abschluss anstelle des qualifizierten Workflows entscheidet.
  Pure Steps erhalten ihre eigene erforderliche Freigabe; Zugehörigkeit zur
  Dependencyclosure einer Skill-Nutzung allein qualifiziert keinen Pure Step.

**Owner:** B1; Vertragsentwurf gemeinsam mit V351-09, keine zweite Approvalstelle.
Neue Zielartefakte über den bestehenden ausdrücklich aufgerufenen Development-/
Releaseworkflow nichtzirkulär qualifizieren. Keine normale Taskausführung
voraussetzen, deren unqualifizierter Graph erst durch sich selbst freigegeben
werden soll. Qualifizierte Originale im belegten Umfang wiederverwenden.
**Abnahme:** positive Wiederverwendung im belegten Umfang; unpassende native
Nachweise, geliehene Herkunft und unqualifizierte Artefakte werden abgewiesen.

### V351-11 — Umzug, Upgrade und Task-Wiederaufnahme absichern

- [ ] Transportierbare Konfiguration, Komponenten und Daten von lokalen Pfaden,
  Secrets, Datenbank-Binärformaten und nativen Artefakten trennen. Für PostgreSQL
  einen unterstützten logischen beziehungsweise ausdrücklich kompatiblen
  Datenumzug festlegen; rohes Kopieren eines Clusters nicht als universell annehmen.
- [ ] Neue Umgebung über V351-02/03/07 auflösen und technische Voraussetzungen
  vor Aufnahme neuer Arbeit prüfen. Überschriebene authored Komponenten und
  unveränderliche Historie erhalten; shipped Validierung mit echter Herkunft übertragen.
- [ ] Laufende/wartende Tasks behalten ihre ursprünglichen Artefaktpins. Ist die
  Zielumgebung inkompatibel, Wiederaufnahme verständlich blockieren. Einen
  ausdrücklichen Migrationsvertrag mit Quellpins, Zielnachweisen, Zustands-/ABI-
  Kompatibilität, Claims, Versuchsständen und Effektabgleich im bestehenden
  Recoveryowner implementieren, bevor solche Migration angeboten wird.
  Vor der Zielaufnahme alte Instanz/Generation und Dispatch-/Replyrechte wirksam
  fencen. Kopierte Daten dürfen nicht zwei aktive Instanzen desselben Tasks
  erzeugen. Externe deduplizierte Effekte und ausstehende Ergebnisse einbeziehen.
  VM-Heap oder serialisierbarer Speicher allein ist kein dauerhafter Checkpoint.
- [ ] Keine stillen Ersatzartefakte, automatische Gesamtreplays oder Rücksetzung
  von Attempts. Ein neuer Task ist keine unbemerkte Fortsetzung eines alten.
  Fachliche Inspektions-/Abgleichschritte als explizite vorhandene Usages/Recipes
  sequenzieren; native Owner behalten ihre eigenen Custody-, Accounting- und
  Releaseprädikate. Eine allgemeine Migrationszeile ersetzt keine fehlenden
  Nativevidence. Log-/Observer-Replay nimmt keine Tasks erneut auf. Einen
  geänderten Artefaktvertrag nicht als bloßes Wiederaufnehmen alter Pins ausgeben.
  Falls Wiederholung/Iteration erforderlich wird, zuerst die vorhandenen
  Flow-/Occurrenceowner anschließen; keine Rustschleife oder Lockerung der
  Duplicate-Step-Sperre als Recoveryabkürzung. Unverändert stop-only belassen,
  solange der konkrete Fortsetzungsvertrag fehlt. Angewandte Migrationen nicht
  umschreiben; neue Migrationen bewahren IDs, Zähler und ungeklärte Zustände.

**Owner:** B3/Installation/Recovery, B2 für Pins, B1 für Nachweisgeltung.
**Abnahme:** Umzug zwischen unterstützten OS/Architekturen mit Daten- und
Overrideerhalt; kompatible Fortsetzung sowie explizite Blockade inkompatibler Pins
und sicherer Umgang mit bereits bestätigten oder ungeklärten Effekten.
Die sichere Blockade inkompatibler Tasks gehört zur Basisabnahme. Native
Artefaktsubstitution ist eine gesonderte, ausdrücklich versionierte Migration
und bleibt deaktiviert, solange Zustands-/Effektkompatibilität nicht belegt ist;
dieser Plan behauptet keine schon vorhandene allgemeine Migrationsschnittstelle.

### V351-12 — Builds, CI, Modal und Releasepakete vervollständigen

- [ ] Vorhandene Buildkonfiguration um vollständige Host-/Target-/ABI-Matrix
  ergänzen. Caches, Compiler, native Bibliotheken und Ergebnisse strikt passend
  zu Source, Features, Toolchain und Target halten.
- [ ] Linux nativ auf Modal bauen **und dort ausführen/testen**; bestehenden
  Controller, Ressourcen-, Aktivierungs-, Quellen- und Lockvertrag verwenden.
  Das ist künftige Implementierungsarbeit, kein Auftrag dieses Dokuments für
  einen Cloudlauf. Vor Ausführung geltende Modal-Anweisungen und lokale
  Testumgebung lesen; Apple-Crossbau nicht als native Linux-Abnahme anrechnen.
- [ ] Gemeinsame Adapter-Vertragssuite auf nativen Linux-, macOS- und
  Windows-Runnern ausführen. Cross-Compilation allein belegt weder Lifecycle
  noch Keychain, Dateisystemgarantien oder PostgreSQL-Funktion.
  Jede freizugebende Architektur-/ABI-/Featurezeile braucht passende Belege;
  ein macOS-ARM64-Lauf qualifiziert nicht automatisch macOS-x86_64. Die gemeinsame
  Suite ist geplante Arbeit aus wiederverwendeten vorhandenen Tests und nötigen
  Ergänzungen, kein bereits vorhandener benannter Testrunner. Vor Produktfreigabe
  auch tatsächlich wirksame Kernel-/Sandbox-/Ressourcengrenzen prüfen.
- [ ] Release-Matrix und Paketaufbereitung um vollständiges Windows-MSVC-Paket
  erweitern: Hauptprogramm, `monty_worker.exe`, PostgreSQL/pgvector, notwendige
  Bibliotheken, Integritätsnachweise und Lizenzhinweise. Windows-gerechte
  Installation, Upgrade und Deinstallation ohne vorausgesetzte Bash ergänzen.
- [ ] Ausgelieferte Pakete auf sauberen Zielsystemen prüfen, einschließlich
  Start ohne Buildtools/Netzzugang, Secret-Voraussetzungen und geordnetem Ende.
  Fehlendes Pflichtartefakt verhindert die Freigabe des Zielpakets.
  Offline bezieht sich auf lokal paketierte Runtime-/Datenbankbestandteile;
  externe Provider brauchen weiterhin ihre deklarierte Authentisierung und
  Erreichbarkeit. Paket-Smoke-/Integritätsprüfungen sind keine erneute
  installierte Komponentenqualifikation. Produktprüfungen nutzen vorab
  qualifizierte kanonische Definitionen; keine zirkuläre Selbstqualifikation.
  Mindestens einen bereits beauftragten qualifizierten universellen Workflow
  durch den tatsächlichen Ordinary-/Kernel-/Montypfad auf jedem Ziel belegen.
  Vorhandenen v3.5-Referenz-/Consumervertrag wiederverwenden; kein zusätzlicher
  Universal-Testservice oder Pflicht-Recipe pro Skill. Adaptertests allein
  beweisen weder Routing, Komponentenbindung noch genau eine korrelierte Antwort.

**Owner:** Build-/Releaseowner/B3; Anschluss an Combined Building.
**Abnahme:** native Vertrags- und Paketbelege für jede freizugebende Matrixzeile;
Build, Test, Paketprüfung und Veröffentlichung getrennt ausweisen. Lange lokale
Prüfungen folgen den bestehenden Screen-, Lock-, Target- und Überwachungsregeln.

### V351-13 — B3: Plattformstatus in WebUI/API sichtbar machen

- [ ] Universal-/Spezifisch-Geltung, erkannte Umgebung, ausgewählte native
  Implementierung und deren Qualifikations-/Integritätsstatus verständlich anzeigen.
  Verfügbare Zielimplementierung für neue Tasks von der tatsächlich gepinnten
  Implementierung eines laufenden Tasks unterscheiden. Unbekannt, fehlt,
  inkompatibel und blockiert nicht als dieselbe Erfolgs-/Fehleranzeige behandeln.
- [ ] Fehlende Voraussetzungen konkret erklären, etwa nicht erreichbarer
  Secretstore, fehlende Shell, unpassende ABI oder nicht vorhandenes gepinntes
  Workerartefakt. Plattformstatus, Freigabe und aktuelle Tool-Policy getrennt halten.
- [ ] Bei Umzug und Wiederaufnahme Ursprungspins, Zielkompatibilität und einen
  nötigen ausdrücklich unterstützten Migrationsschritt zeigen. Geheimnisse und
  unnötige interne Dateipfade nicht in Produktoberflächen ausgeben.

**Owner:** B3/WebUI mit B1-/B2-Daten über die vorhandenen API-/DTO-Owner.
Plattformfelder und nötige Endpunkte sind erst zu entwerfen und durch Backend,
Facade, API, Client und Oberfläche anzuschließen; ihre heutige Existenz wird
nicht vorausgesetzt. Keine Freigaberegeln oder OS-Probes im Frontend duplizieren
und keine neuen Benutzer-/Tenant-/Feature-Rollen als Plattformberechtigung schaffen.
Vorhandene Operatorauthentisierung und externe Protokollgrenzen einschließlich
Origin-/CSRF-Prüfungen erhalten. Observer-Reconnect nutzt vorhandene Cursor-/
Gapmechanismen und verursacht keinen neuen Dispatch. Providerdefinitionen bleiben
DB-owned; Plattformprofile schaffen keine zweite Env-/JSON-Providerregistry.
**Abnahme:** Oberfläche und API zeigen tatsächliche Backendzustände auf allen
Zielen; keine Erfolgsanzeige aus bloßem OS-Namen oder erfolgreichem Build.

## Quellenanschlüsse und vollständiges Schrittaudit — 2026-11-16

Alle 13 Schritte wurden als **Plan** auf Logik, Architektur, Designgrenzen und
Quellenbezug geprüft. Diese Tabelle ergänzt das erste Teilaudit; sie hakt keine
Implementierung ab. Geprüft wurde der lokale Arbeitsstand beim oben genannten
HEAD einschließlich sichtbarer laufender Änderungen. Insbesondere Buildskripte
und Bob B1 werden parallel bearbeitet; diese Änderungen sind hier nicht
abgenommen. Vor Umsetzung den betroffenen Iststand erneut abgleichen.

Verlinkte Dateien und genannte Reposymbole existieren. Sie sind konkrete
Inspektions-/Erweiterungsstellen, **keine Behauptung fertiger Multiplattform-APIs**.
Eine Moduldatei ersetzt keinen Nachweis der tatsächlichen produktiven Verdrahtung.

| Schritt | Geprüfte Quellen / tatsächlicher Anschluss | Auditentscheidung und Grenze |
| --- | --- | --- |
| V351-01 | Vier Komponentenguides, Bob B1/B2/B3 und [Release-Matrix](.github/workflows/release.yml), oben einzeln verlinkt | Geltung, Kompatibilität, Herkunft und Supportzusage getrennt; Altbestands-/Cutoverregeln präzisiert. Die sieben Vertragsdokumente erst bei Umsetzung ändern. |
| V351-02 | [PlatformInfo](crates/brassclaw_common/src/platform.rs), [global_monty_startup.rs](crates/brassclaw_reborn_composition/src/global_monty_startup.rs) | `PlatformInfo::to_prompt_section` dient Promptidentität. Technisches Profil und Übergabe an B2 sind neue Arbeit; pro Artefakt und Ausführungskontext prüfen. |
| V351-03 | [paths.rs](crates/brassclaw_common/src/paths.rs): `compute_brassclaw_base_dir`, `brassclaw_base_dir`; [process.rs](crates/brassclaw_monty_host/src/process.rs): `installed_worker` | Basisverzeichnishelfer ist kein vollständiges OS-Verzeichnislayout. Defaults, virtuelle Pfade und vertrauenswürdige Workerauflösung erhalten. |
| V351-04 | [root.rs](crates/brassclaw_filesystem/src/root.rs): `RootFilesystem`; [local.rs](crates/brassclaw_filesystem/src/local.rs): `put`, `write_file_with_cas`, `atomic_write_file`; [types.rs](crates/brassclaw_filesystem/src/types.rs): `BackendCapabilities` | Move-/Lockmethoden und Versions-CAS nicht als vorhanden voraussetzen. Tatsächlich zugesagte Fähigkeiten und mögliche Effekte trotz Fehlermeldung präzisieren. |
| V351-05 | [process.rs](crates/brassclaw_monty_host/src/process.rs), [service.rs](crates/brassclaw_monty_host/src/service.rs), [global.rs](crates/brassclaw_monty_host/src/global.rs), [Monty-Hostbeschreibung](crates/brassclaw_monty_host/README.md), [ProcessHost](crates/brassclaw_processes/src/host.rs) | ProcessHost verwaltet Poll/Await/Cancel, belegt keinen nativen OS-Spawn. Globalen Root von Childkontexten und Prozessüberwachung von Sandbox unterscheiden. Job Objects sind ein Entwurfskandidat, keine behauptete Repo-Funktion. |
| V351-06 | [shell.rs](crates/brassclaw_host_runtime/src/first_party_tools/shell.rs): `dispatch`; [shell_core.rs](crates/brassclaw_host_runtime/src/first_party_tools/shell_core.rs); [recipe.md](recipe.md) | Shellmodul delegiert Ausführung. Tatsächlichen RuntimeProcessPort-Implementierer verfolgen; Validierung belegt keinen portablen Prozessbackend. Typisierung, Bindingpaare und Tier-1-Grenze erhalten. |
| V351-07 | [keychain.rs](crates/brassclaw_secrets/src/keychain.rs): `resolve_master_key_material` und vier Master-Key-Operationen; [factory.rs](crates/brassclaw_reborn_composition/src/factory.rs); [PgSecretStore](crates/brassclaw_secrets/src/pg_store.rs) | Windows-Master-Key-Backend fehlt; Resolver ist an Composition angeschlossen. Wiederanmeldung und Entschlüsselung trennen, Env-Override erhalten, destruktive Neuschlüsselerzeugung ausschließen. |
| V351-08 | [build.rs](crates/brassclaw_embedded_postgres/build.rs): `needs_pgvector_compile`, `try_build_pgvector`; [extract.rs](crates/brassclaw_embedded_postgres/src/extract.rs): `ensure_pg_extracted`; [initdb.rs](crates/brassclaw_embedded_postgres/src/initdb.rs): `run_initdb`, `install_pgvector`; [pgctl.rs](crates/brassclaw_embedded_postgres/src/pgctl.rs): `PgCtl` | Leere Archive sind Istcode; Apple-Crossbau hat bereits einen strengeren Fehlerzweig. Windows-Paket, Programmpfade, Lade-/Startsemantik und sichere Extraktion gemeinsam bearbeiten. |
| V351-09 | [authored_monty_catalogue.rs](crates/brassclaw_reborn_composition/src/authored_monty_catalogue.rs), [installed_monty_catalogue.rs](crates/brassclaw_reborn_composition/src/installed_monty_catalogue.rs), [retained_instruction.rs](crates/brassclaw_engine/src/memory/retained_instruction.rs), [retained_tools.rs](crates/brassclaw_engine/src/memory/retained_tools.rs), [B2-Leserinventar](docs/reborn/bob2-retained-reader-inventory.md), [tools.md](tools.md) | IBS-Compiler lädt keine nativen Toolartefakte. Allgemeine Zielartefaktregistrierung ist damit nicht belegt; tatsächlichen Registrierungs-/Ladeowner und alle Consumer anschließen. Stillen Matcherfilter und Downgrade ausgeschlossen. |
| V351-10 | [pg_association_review.rs](crates/brassclaw_reborn_composition/src/pg_association_review.rs), [pg_pure_component_approval.rs](crates/brassclaw_reborn_composition/src/pg_pure_component_approval.rs), [bootstrap_validator_approval.rs](crates/brassclaw_reborn_composition/src/bootstrap_validator_approval.rs), [B1](Repair_bob_1_v3_at_k3.md) | Plattformherkunft authentisch erfassen; Formaterweiterungen sind neue Arbeit. Nichtzirkuläre Qualifikation, eigenständige Tool-/ToolSkill-Voraussetzungen und exakte S-Nutzung erhalten. |
| V351-11 | [global_monty_startup.rs](crates/brassclaw_reborn_composition/src/global_monty_startup.rs), [native_tool_answers.rs](crates/brassclaw_reborn_composition/src/native_tool_answers.rs), [retained_recipe.rs](crates/brassclaw_engine/src/executor/retained_recipe.rs), [Recoveryvertrag](simplified_v3.md) | Originale Antwortjournale belegen keinen allgemeinen OS-übergreifenden Checkpointimport. Quellfencing ergänzt; sichere Blockade von einer gesondert qualifizierten Migration getrennt. |
| V351-12 | [release.yml](.github/workflows/release.yml), [modal_build.py](scripts/modal_build.py), [modal_build_worker.py](scripts/modal_build_worker.py): `validate_command`, `command_mode`; [Buildumgebungen](docs/build-environments.md), [Modal-Vertrag](docs/modal-cross-build.md), [Combined Building](combined-building_v3_at_bob3.md) | Windows fehlt in der Release-Matrix. Worker unterscheidet Apple-Testpaket und Linux-Kompiliermodus; nativer Linux-Testbetrieb ist hier kein belegter fertiger Modus. Controller/Worker/Runner/Aktivierung gemeinsam erweitern, keine CLI-Modi erfinden; native Belege je Matrixzeile. |
| V351-13 | [WebUI-Deskriptoren](crates/brassclaw_webui_v2/src/descriptors.rs), [WebUI-Schema](crates/brassclaw_webui_v2/src/schema.rs), [component-graph.js](crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/component-graph.js): `indexComponentGraph`; [live_monty_settings.rs](crates/brassclaw_reborn_composition/src/live_monty_settings.rs) | Bestehende API-/Darstellungsstellen sind Anschlusskandidaten, keine fertige Plattformstatus-API. Passende Facade-/DTO-Kette bestimmen; neue Auswahl und tatsächliche Task-Pins unterscheiden. |

**Prüfgrenze:** lokale Quellen-/Vertragsprüfung und Existenzkontrolle der Links
und explizit genannten Reposymbole. Kein Build, Cloudauftrag, Datenbanktest oder
Produktfreigabe. Native OS-Mechanismen sind Anforderungen beziehungsweise
Entwurfsoptionen; vor Implementierung anhand aktueller offizieller
Plattformdokumentation und der festgelegten Mindestversionen konkretisieren.
Abschließende lokale Kontrolle: 13 eindeutige Schrittüberschriften, 52 eindeutige
lokale Linkziele einschließlich exakter Groß-/Kleinschreibung und 29 genannte
Symboldefinitionen geprüft; keine fehlenden Ziele/Definitionen festgestellt.
Die Prüfung der Verwendungen und Architekturgrenzen ist oben je Schritt
dokumentiert. Alle Umsetzungsschritte bleiben offen.

## Architekturabgleich mit v3 und v3.5 — 2026-11-16

Der folgende zusätzliche Review prüft jeden Schritt gegen die
[v3.5-Verantwortungsmatrix](path_to_v3.5.md#decision-ownership-and-interfaces),
den [Connections-Vertrag](docs/plans/simplify_connections_v3.5.md) und die vier
v3-Komponentenguides. V35-Verweise benennen vorhandene fachliche Owner und
Akzeptanzgates, keine als erledigt behaupteten Voraussetzungen. V351-01 bis
V351-13 bleiben offene Implementierungsschritte.

| Schritt | v3-/v3.5-Vertrag und zugehöriger Owner | Prüfergebnis nach Planpräzisierung |
| --- | --- | --- |
| V351-01 | V35-02/03/04/08: Rollen, Pure Approval, semantische Verantwortungsprüfung | Konform: Plattformgeltung ist Metadaten-/Kompatibilitätsvertrag; pure Logik wird weder Tool noch künstlicher Skill. Fehlende allgemeine Approvalunterstützung bleibt beim Validator. |
| V351-02 | V35-01/23: technische Fakten und aktuelle Kernelautorität | Konform: Rust erkennt technische Voraussetzungen; es entscheidet keinen fachlichen Recipe-Ablauf. Ein Profil erteilt keine Permission. |
| V351-03 | V35-01/27: Wiederverwendung, Konfiguration, Retention | Konform: native Pfade bleiben Adapterdetails, virtuelle Referenzen und alte Pins bleiben stabil. Betriebssystemspezifische Defaults erzwingen keine ungeprüfte Datenmigration. |
| V351-04 | V35-08/23/26: fehlende Primitive, Authority, Effektzustand | Konform: erforderliche technische Filesystemunterstützung darf in Rust liegen. Sie entscheidet keine Fachworkflows, behauptet keine fehlenden Fähigkeiten und erzeugt keine Retryberechtigung aus bloßem Fehler. |
| V351-05 | V35-24/26/27: gemeinsamer Transport, globaler Lifecycle, getrennte Budgets | Präzisiert: ein Transportowner und ein globaler Root; bestehende explizite Childpfade erhalten. Kanalverlust, Cancel und Workerfehler bleiben getrennte Ereignisse. |
| V351-06 | V35-05/06/08 sowie Sempai Steps 2–4: echte wiederverwendete Komponenten | Präzisiert: Recipe-first-Entwurf, kleine Exporte, typisierte Kanten und echte Tier-0-Belege. Kein Shell-zu-Rust-Workflow-Umbau unter neuem Toolnamen. |
| V351-07 | V35-22/23/27: externe Auth, Secrets und technische Durchsetzung | Konform: OS-Keychain ist technische Primitive; Authablauf und Secret-Custody behalten ihre Owner. Weder zusätzliche Plattformrollen noch unkontrollierter neuer Schlüssel umgehen Zugriffsgrenzen. |
| V351-08 | V35-21/29 und Connections Phase B: PG-Policy, Pools, Startup/Shutdown | Präzisiert: gleiche Verbindungs-/TLS-Policy auf allen Zielen, begründete Pooltrennung, aggregierte Kapazität, Drain-before-stop und kein SQL-Replay nach ACKverlust. |
| V351-09 | V35-04/23/25/27: coherent selection, aktuelle Policy, MCP, Upgrades | Präzisiert: ein Katalogowner und unveränderliche Auswahl; MCP-Ankündigung ist noch kein Task-Capture. Kein alternativer Runner, Filter-Fallback oder nachträglicher Artefaktaustausch. |
| V351-10 | V35-03/04/17–19 und B1: vertrauenswürdige Freigabe, A/S, Pure Subjects | Präzisiert: fachlicher Prüfablauf bleibt Recipe/PythonCode; Rust authentisiert Belege/Writes. Pure Approval, exakte S-Nutzung und unveränderte A-Consumer bleiben getrennt. |
| V351-11 | V35-13/14/26/27/28 und Connections Phase E: Occurrences, Custody, Cutover | Präzisiert: explizite fachliche Abgleichusages und echte native Releaseprädikate; stop-only ohne belegten Fortsetzungsvertrag. Keine generische Ledger-/Checkpointmigration oder doppelte Admission. |
| V351-12 | V35-05/06/16/29/30: tatsächliche Caller, Bootstraptransfer, Abschlussreview | Präzisiert: native Adapter-/Pakettests plus wiederverwendeter echter Ordinary-Workflow. Keine unabhängige Testplattform, kein Bootstrap während Startup und keine Aktivierung durch erfolgreiche Compilation. |
| V351-13 | V35-22/25/27: Operator-/Protokollgrenzen, Observer, Livezustand | Präzisiert: UI konsumiert authentische Backendfakten, ACK-/Revisionsstatus und Task-Pins. Keine Frontendautorität, neue Rollen oder zweite Providerquelle; Reconnect ohne Resubmission. |

**Verbleibende Umsetzungsgrenze:** Die Planfassung wahrt damit die geprüften
v3-/v3.5-Verantwortungsgrenzen. Ob eine spätere Implementierung diese einhält,
ist über tatsächliche Methoden, Exporte, Caller, unveränderliche Auswahlen und
Effektspuren im jeweiligen Paket zu prüfen. Insbesondere Pure Approval,
Occurrences und allgemeine Recovery dürfen nicht aus Plantext oder vorhandenen
Hilfstypen als vollständig implementiert abgeleitet werden. Kein umfassender
Abschluss von v3, v3.5, B1/B2/B3 oder dieser Plattformarbeit folgt aus dem Review.

## Abhängigkeiten und Abschluss

1. V351-01 legt Vertrag und Matrix fest; V351-02 liefert technische Fakten.
   B1/B2 entwerfen V351-10/09 gemeinsam, bevor neue Profile veröffentlicht werden.
2. V351-03 bis V351-08 schließen die jeweils benötigten nativen Lücken.
   Bestehende unveränderte Implementierungen und gültige Nachweise wiederverwenden.
3. V351-09/10 verbinden echte Nachweise mit vollständigen Auswahl-/Readerpfaden;
   neue Artefakte erst nach den erforderlichen nativen Qualifikationen freigeben.
4. V351-11 verbindet Installation und Recovery, V351-13 die Produktoberfläche.
   V351-12 liefert währenddessen die erforderlichen nativen Belege; abschließende
   Paketabnahme wartet auf alle Voraussetzungen des jeweiligen Zielsystems.

Die Nummerierung ist keine strikt serielle Ausführungsreihenfolge:

| Paket | Vorarbeit und Abschlussgrenze |
| --- | --- |
| V351-01 | Vertragsabgleich; abschließbarer Entwurf ohne native Freigabe. |
| V351-02 | Profilbedeutung aus V351-01; native Erkennungsbelege mit Runnern aus V351-12. |
| V351-03 | V351-01; Pfad-/Mount-/Workergrenzen unabhängig implementierbar, Umzugsabschluss in V351-11. |
| V351-04 | V351-01/03 und echter RootFilesystem-/Aufrufervertrag; native Abnahme aus V351-12. |
| V351-05 | V351-01/02/03, Kernel-/Ressourcenanbindung; native Abnahme aus V351-12. |
| V351-06 | V351-01 und nötige Primitive aus V351-04/05; Komponentenqualifikation über V351-10. |
| V351-07 | V351-01/02/03, Secret-/Auth-Vertrag; native Belege aus V351-12, Datenumzug unter V351-11. |
| V351-08 | V351-01/03/05, sichere Paketprüfung, Zielbau/-tests aus V351-12; keine Veröffentlichung unqualifizierter Komponenten voraussetzen. |
| V351-09 | Gemeinsamer Entwurf mit V351-10, Profil aus V351-02; Freigabe erst mit echten Artefakten/Nachweisen und vollständigem Reader-Cutover. |
| V351-10 | Gemeinsamer Entwurf mit V351-09; vorhandene nichtzirkuläre Qualifikationsgrenze und native Runner aus V351-12 nutzen. |
| V351-11 | V351-02/03/05/07/08/09/10 im betroffenen Umfang; Quellfencing vor Zielaufnahme, sichere Blockade bei inkompatiblen alten Pins. |
| V351-12 | Runner-/Paketgrundlagen ab V351-01 vorbereiten; Tests folgen implementierten Adaptern, finale Zielpaketfreigabe nach allen betroffenen Gates. |
| V351-13 | Anzeigevertrag ab V351-01; Abschluss erst mit angeschlossenen Fakten aus V351-02/09/10/11 und API-/Backendintegration. |

Jeder Schritt wird erst nach Quellenreview und seinem konkreten Abnahmenachweis
abgehakt. Nachweise nennen tatsächlichen Source-/Artefaktstand, Target,
Ausführungsplattform, Konfiguration und verbleibende Grenzen. Bereits bestandene
Fälle nur bei relevanten Änderungen oder offener Unsicherheit wiederholen.
Den Implementierungsstatus und die zuständigen Bob-/Buildpläne mit den Ergebnissen
aktualisieren, ohne historische Aussagen umzudeuten.

V3.51-Multiplattformfähigkeit ist erreicht, wenn alle zugesagten Matrixziele die
gemeinsamen Verträge mit vollständigen nativen Paketen erfüllen, B1-/B2-Nachweise
und Pins durchgängig erhalten bleiben und Umzug/Wiederaufnahme keine stillen
Vertrags- oder Artefaktwechsel auslösen. Ein einzelner Build, ein Markdownvertrag
oder eine nur auf einem Betriebssystem bestandene Suite schließt diesen Plan nicht.
