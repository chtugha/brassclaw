# Combined Building v3 at Bob3 — einfacher Kern

Stand: 2026-11-12. Status: Plan, nicht implementiert. Dieser Auftrag ändert nur
dieses Dokument; er startet keine Builds, Tests, Cloud-Worker oder Aktivierung.

## Ziel

Ein gemeinsamer Einstieg verwendet die vorhandenen Skripte, um lokal oder remote
zu bauen. Remote erzeugte macOS-Programme werden automatisch lokal übernommen
und können dort ausgeführt werden. Linux-Programme können remote gebaut und
remote ausgeführt werden. Lokaler und Remote-Build dürfen parallel laufen;
Aufträge mit demselben veränderlichen Cache bleiben seriell.

Die Geschwindigkeit kommt aus Remote-Rechenleistung, warmen getrennten Caches
und paralleler Arbeit. Ein universeller Cargo-Cache, ein allgemeiner Scheduler
und die automatische Migration beliebiger Tests gehören nicht zum Kern.

### Erwarteter Nutzen und Grenzen

| Ablauf | Wirkung dieses Kerns |
| --- | --- |
| Produktprogramm bauen | Kann mit warmem Remote-Cache und geeigneten Ressourcen schneller sein; automatische Übernahme spart manuelle Schritte |
| Produktbuild parallel zu lokalem Testbuild | Kann die gesamte Arbeitszeit verkürzen; beide Compiler-Caches bleiben unabhängig |
| Unit-/Integrationstestprogramme kompilieren | Zunächst weiterhin nativ; keine direkte Remote-Beschleunigung durch diesen Kern |
| Bestehenden passenden Test gegen das heruntergeladene Produkt ausführen | Automatisierbar, wenn der vorhandene Runner den exakten Programmpfad akzeptiert |

Entscheidend ist die komplette Zeit aus Quellabgleich, Containerstart,
Kompilierung, Paketierung, Download und gewählter Ausführung. Ein warmer lokaler
Inkrementalbuild kann schneller als ein kalter Remote-Build sein. Neue Images
haben zunächst eigene kalte Kompilierungscaches. Lokale und Remote-Arbeit
überlappen bereits grundsätzlich; der gemeinsame Einstieg vereinfacht sie,
erzeugt aber allein keinen zusätzlichen Compiler-Speedup. Parallelität kann
lokal durch Quellhashing, Transfers und Plattenzugriffe auch bremsen.

Der Kern verspricht daher keinen pauschalen Speedup für Produkt- und Testcode.
Eine spätere gezielte Testbuild-Erweiterung muss ihren Zeitgewinn separat zeigen.

## Vorhandenes weiterverwenden

| Aufgabe | Bestehender Eigentümer |
| --- | --- |
| Gemeinsamer Einstieg / lokale Ausführung | `scripts/local_build.py` erweitern |
| Remote-Setup, Build, Status, Download | `scripts/modal_build.py` erweitern |
| Einstellungen | `scripts/build_settings.py`, `scripts/build-environments.toml` |
| Exakter Quellabgleich | `scripts/modal_source_sync.py` unverändert wiederverwenden, soweit passend |
| Remote-Cargo und Artefaktexport | `scripts/modal_build_worker.py` gezielt erweitern |
| Apple-Cross-Toolchain | `scripts/modal_apple_toolchain.py` |

Es gelten die [Entwicklungspolicy](docs/development-policy.md), der
[Build-Umgebungsvertrag](docs/build-environments.md) und der
[Modal-Image-Vertrag](docs/modal-cross-build.md). Erweiterte Remote-Ausführung
muss vor ihrer Einführung dort konsistent beschrieben werden. Bestehende
Bild-/Jobbelege bleiben an ihren tatsächlichen Geltungsbereich gebunden.

Die Nutzerfreigabe vom 2026-11-12 erlaubt auch Testbuilds, Tests und andere
notwendige Modal-Arbeit ohne erneute Standortfrage, wenn die Verteilung insgesamt
Zeit spart. Sie ersetzt die frühere 60-Sekunden- und Nur-lokale-Tests-Policy,
implementiert aber keine fehlenden Controller-/Runner-Modi. Keine Schätzwerte
erfinden, um die noch vorhandene technische Controller-Grenze zu umgehen.
Produktabnahme bleibt beim bestehenden B3-12-Owner; dieser Entwicklungsplan
ersetzt weder dessen Tests noch die drei Bob-Reparaturpläne.

## Die drei Betriebsarten

| Auswahl | Build-Ort | Produkt-Target | Übernahme und Ausführung |
| --- | --- | --- | --- |
| Lokal/macOS | macOS/ARM64 | `aarch64-apple-darwin` | bestehender nativer Ablauf |
| Remote/macOS | Linux/x86_64 mit OSXCross | `aarch64-apple-darwin` | automatischer Download; Ausführung lokal auf dem Mac |
| Remote/Linux | Linux/x86_64 | `x86_64-unknown-linux-gnu` | Ausführung remote; Logs und Ergebnis lokal übernehmen |

Ein lokaler Linux-Cross-Compiler, andere Architekturen und andere libc-Varianten
sind keine Voraussetzung dieses Ausbaus. Eine inkompatible Kombination wird
klar abgelehnt; es gibt keinen stillen Wechsel von Plattform oder Build-Ort.

## 1. Einen dünnen gemeinsamen Einstieg ergänzen

- [ ] In `scripts/local_build.py` einen neuen Unterbefehl `combined` vor dem
  bisherigen Argumentparser erkennen. Bestehende lokale Aufrufe einschließlich
  `--jobs`, `--config`, `--show-settings` und `-- cargo ...` bleiben kompatibel.
- [ ] Im neuen Modus Build-Ort, Target, Package, Binärziel, Profil und Features
  auswählen. Er delegiert an die vorhandene lokale oder Remote-Ausführung und
  besitzt keinen zweiten Build-Loop. Beispielsyntax, erst zu implementieren:

  ```text
  python scripts/local_build.py combined --where local --target macos-arm64 --package brassclaw_reborn_cli --bin brassclaw-reborn
  python scripts/local_build.py combined --where remote --target macos-arm64 --package brassclaw_reborn_cli --bin brassclaw-reborn
  python scripts/local_build.py combined --where remote --target linux-x64 --package brassclaw_reborn_cli --bin brassclaw-reborn
  ```

- [ ] Eine optionale Ausführungswahl mit expliziter Programmauswahl und
  Argumentliste vorsehen: macOS lokal, Linux remote. Ohne diese Wahl wird nur
  gebaut und bereitgestellt. Keine Shellstrings und kein unbegrenzter Dienststart.
- [ ] Den ersten Umfang auf die CLI `brassclaw-reborn` und ihren benötigten
  `monty_worker`, mit den vorhandenen Dev-/Release-Profilen, begrenzen. Andere
  Packages oder Profile erst freigeben, wenn deren konkreter Export-/Runtime-
  Pfad geprüft ist. Die Eingabefelder bedeuten keine pauschale Unterstützung
  beliebiger Cargo-Ziele; der heutige Apple-Controller ist CLI-only.
- [ ] Die tatsächliche Package-/Target-/Featureauswahl prüfen und `--locked`
  verwenden. Remote das exakte Target setzen; lokal den bestehenden nativen
  Cargo-Aufruf und das vorhandene Artefaktlayout beibehalten.
- [ ] Ungültige Optionen vor Compute ablehnen. Settings-/Planansicht bleiben
  lokal ohne Modal-Import oder Cloudkontakt. Ressourcen und Standortentscheidung
  aus bestehenden Einstellungen und realen Zeitbelegen übernehmen, nicht aus
  erfundenen Schätzwerten. Der neue Einstieg umgeht keine Aktivierungsprüfung.

## 2. Quellabgleich und getrennte Umgebungen erhalten

- [ ] Quellen nur lokal → remote synchronisieren: vorhandene Baseline-/Delta-
  Übertragung mit Hashprüfung, Löschungen und festem aufgenommenem Snapshot.
  Kein Remote-Quellmerge zurück in den lokalen Checkout.
- [ ] SDKs, Compiler und native Bibliotheken über die bestehenden vorbereiteten
  Images bereitstellen. Sie müssen vorhanden und eindeutig festgelegt sein,
  aber nicht bei jedem Quellabgleich erneut übertragen werden.
- [ ] Lokales `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` beibehalten.
  Remote weiter `/build/cache/images/<image_id>/target` sowie den vorhandenen
  Downloadcache `/build/cache/cargo` verwenden. Keine Synchronisierung der
  `target/`-Verzeichnisse, Proc-Macro-Bibliotheken oder Cargo-Fingerprints.
- [ ] Build-Skripte verwenden `HOST`/`TARGET` und die passenden Werkzeuge.
  Den vorhandenen Apple-/PG-Cross-Pfad wiederverwenden. Nur konkrete fehlende
  Linux-/Apple-Anschlüsse reparieren; keine pauschale Überarbeitung aller Helfer.
- [ ] Vorhandene lokale und Remote-Locks weiterverwenden. Ein Build je Ort;
  lokal und remote dürfen gleichzeitig laufen. Source-Sync und Imagewechsel
  überholen keinen laufenden oder ungeklärten Remote-Job. Lokale Compilerinputs
  während eines nativen Builds unverändert halten.
- [ ] Im Remote-Job läuft der im Image eingebettete Worker. Nach Änderungen an
  ihm ein passendes Image vorbereiten und dessen tatsächliche Version prüfen;
  die hochgeladene neuere Source-Datei ersetzt ihn nicht automatisch.

## 3. Fertige Programme gezielt exportieren und herunterladen

- [ ] Im bestehenden Worker ausschließlich die angeforderten Produktprogramme
  erfassen. Dafür die aktuellen Cargo-Artefaktmeldungen verwenden, einschließlich
  gültiger warmer `fresh`-Ergebnisse. Keine Auswahl nach dem neuesten Cache-Glob.
- [ ] Die dazu erforderliche strukturierte Cargo-Ausgabe gezielt im bestehenden
  Commandvalidator zulassen. Cargo-Exitcode und erwartete ausführbare Artefakte
  prüfen; ein fehlendes Programm darf kein erfolgreicher leerer Export sein.
- [ ] Nur tatsächlich nötige Begleitprogramme und Laufzeitdateien mitnehmen.
  Besonders `monty_worker` aus `brassclaw_monty_host` muss bei Monty-Nutzung für
  dasselbe Target gebaut und neben das Produktprogramm gepackt werden:
  `installed_worker()` sucht dort. Dessen Produktionsauflösung nicht lockern.
- [ ] Die ausgewählten Dateien noch unter dem Compile-Lock in ein eigenes
  Jobpaket kopieren, bevor spätere Builds den Cache ändern können. Bestehende
  Jobbelege um Artefaktidentität, Dateipfad, Größe und SHA-256 ergänzen.
  Für benötigtes PostgreSQL/pgvector die tatsächlichen ausgewählten Bytes prüfen;
  ein vorhandener leerer Linux-Platzhalter genügt nicht für diese Fähigkeit.
- [ ] Remote-macOS-Pakete automatisch nach
  `$CARGO_TARGET_DIR/modal-build/jobs/<job-id>/` herunterladen. Temporär schreiben,
  Dateipfade/Hashes prüfen und erst vollständig als bereit markieren. Dabei das
  Paketlayout mit benachbartem `monty_worker` erhalten.
- [ ] Remote-Linux-Programme im passenden Remote-Job ausführen; deren Binary muss
  nicht zum Ausführen lokal kopiert werden. Logs, Exitstatus und Hashbelege
  werden automatisch lokal übernommen. Ein zusätzlicher Paketdownload bleibt
  eine ausdrückliche Archivierungsoption.

### Rücksync: lokale Dateien und neuere Ergebnisse schützen

**Remote darf keine nativen Cargo-Ausgaben, lokalen Quellen oder fremden
Jobdateien überschreiben.** Derselbe Dateiname allein begründet keine Identität.

Der einfache Standard vermeidet Binärdatei-Überschreibungen vollständig:
Jeder Build erhält sein eigenes unveränderliches Jobpaket. Neue Versionen landen
in einem neuen Paket. Auch lokal gebaute Programme bleiben an ihrem bisherigen
Cargo-Pfad; ein Remote-Download ersetzt sie dort niemals.

- [ ] Wiederholter Download desselben Jobs: Sind Zielpfad und SHA-256 identisch,
  nichts ändern. Sind vorhandene Bytes abweichend, einen Konflikt melden und
  getrennt herunterladen; keine vorhandene Datei still überschreiben.
- [ ] Unvollständige temporäre Downloads dürfen nur innerhalb des eigenen Jobs
  aus demselben verifizierten Remote-Paket erneut erstellt werden. Sie sind
  weder lokale Buildausgaben noch veröffentlichte Programmversionen.
- [ ] Für einen bequemen gemeinsamen Aufruf optional nur einen kleinen
  `current`-Verweis pro logischem Artefakt führen. Er zeigt auf ein vollständiges
  Jobpaket; die Programme selbst werden nicht gegenseitig überschrieben.
  Lokale Builds können dafür eine Kopie ihres Programmpakets veröffentlichen,
  ohne ihre originalen Cargo-Ausgaben zu verändern.
- [ ] Die logische Artefaktidentität umfasst Package, Binärziel, Produkt-Target,
  Profil und Featureauswahl. Linux/macOS, Debug/Release oder andere Programme
  haben eigene Identitäten und dürfen niemals dieselbe Ausgabe ersetzen.
- [ ] Den gemeinsamen Verweis nur dann ersetzen, wenn eine **nachgewiesene neue
  Version desselben Artefakts** vorliegt: neuer aufgenommener Eingabestand,
  erfolgreicher passender Build, vollständiges geprüftes Paket und eindeutig
  dokumentierter Bezug zur bisher veröffentlichten Version. Compiler-/SDK-
  Wechsel als Build-Eingaben und Provenienz festhalten. Ein neuer Job mit
  unveränderten Eingaben oder nur abweichenden Bytes beweist keinen Nachfolger.
- [ ] Beim Start eines solchen Veröffentlichungsauftrags die erwartete aktuelle
  Version notieren. Beim Abschluss unter einem kurzen Veröffentlichungs-Lock
  vergleichen: Hat inzwischen ein lokaler oder anderer Remote-Build den Verweis
  weitergesetzt, bleibt das neue Paket separat erhalten. Der ältere/später
  fertig gewordene Auftrag darf den Verweis nicht zurücksetzen.
- [ ] Zusätzlich vor Veröffentlichung prüfen, dass der aufgezeichnete Quellstand
  dem aktuell vorgesehenen lokalen Eingabestand entspricht. Unterschiedliche
  Snapshot-Hashes sind nicht automatisch zeitlich geordnet. Bei alten, parallelen
  oder unklaren Ständen keinen automatischen Austausch vornehmen.
- [ ] Dateizeit, Downloadzeit und Versionsnummer aus `Cargo.toml` allein sind
  kein Nachweis. Unbekannte vorhandene Dateien ohne Herkunftsbeleg nicht ersetzen.
  Den neuen Verweis erst nach allen Prüfungen atomar austauschen; frühere Pakete
  bleiben für laufende Prozesse und vorhandene Nachweise erhalten.

Der `current`-Verweis ist Komfort, keine Voraussetzung für den ersten nutzbaren
Ablauf. Ausführung über den expliziten Jobpfad funktioniert ohne ihn und ohne
allgemeines Versionsverwaltungssystem für Builddateien.

## 4. Gewählte Programme ausführen; Tests gezielt anschließen

- [ ] Vor Ausführung das tatsächliche Binaryformat, die Architektur und benötigte
  Laufzeitbibliotheken prüfen. macOS-Programme nur auf dem Mac, Linux-Programme
  in der passenden Linux-Umgebung starten.
- [ ] Einen ausdrücklich gewählten endlichen Programmbefehl mit Timeout,
  Arbeitsverzeichnis und notwendigen Runtime-Einstellungen ausführen. Ausgabe,
  Exitstatus und Programmhash im vorhandenen Jobnachweis festhalten. Produkt-
  Smokes wie `--version`/`--help` sind keine umfassende Testabdeckung.
- [ ] Benötigt ein Programmlauf PostgreSQL oder weitere Dienste, deren echte
  Runtime-Voraussetzungen vor Start prüfen. Insbesondere muss Linux-`initdb`
  unter einem geeigneten Nicht-Root-Benutzer mit Schreib-/Shared-Memory-Rechten
  laufen können. Bei fehlender Voraussetzung sichtbar abbrechen; kein stiller
  Hostwechsel und keine Behauptung, erfolgreicher Cross-Build beweise Laufbarkeit.
- [ ] Bestehende lokale Tests zunächst weiter über ihren vorhandenen nativen
  Ablauf ausführen. Ein Remote-Produktbuild ersetzt weder Testkompilierung noch
  deren Features/Profil. Keine zusätzliche lokale Neukompilierung auslösen,
  wenn lediglich das übernommene Produktprogramm ausgeführt werden soll.
- [ ] Einen konkreten Test nur dann automatisch anschließen, wenn er bereits
  einen passenden vorhandenen Runner und belegte Voraussetzungen besitzt.
  Dessen exakten Programm-/Artefaktpfad übergeben; keine zufällige lokale Binary
  über PATH oder aktuelle Cargo-Dateien auswählen. Fehlende Dienste/Fixtures
  sichtbar melden, statt eine nicht ausgeführte Prüfung als Erfolg zu zählen.

## 5. Kleine notwendige Robustheit im bestehenden Ablauf

- [ ] Bestehende Job-UUIDs, Request-/Source-/Completion-Belege, Hashprüfungen und
  Locks behalten. Keine neue allgemeine Phasendatenbank oder Schedulerpipeline.
- [ ] Compilerergebnis, Paketübernahme und optionale Ausführung im vorhandenen
  Jobnachweis unterscheidbar festhalten. Ein Downloadfehler startet keinen neuen
  Build; ein Test-/Programmlauf wird nach verlorenem Ergebnis nicht automatisch
  wiederholt. Teilpakete werden niemals ausgeführt.
- [ ] `status` und normalen Abschluss denselben geprüften Downloadpfad verwenden
  lassen. Herkunft aus dem ursprünglichen Job übernehmen, nicht aus heutigen
  Defaults. So lässt sich ein unterbrochener Download ohne Neubuild nachholen.
- [ ] Remote-Terminalität weiterhin tatsächlich feststellen. Ein Verbindungsabbruch
  ist kein Ende des Workers; ein belegter Remote-Abschluss wird durch einen
  späteren lokalen Downloadfehler nicht wieder unbekannt. Erst nach beendetem
  Remote-Build/-Programmlauf neue Source-/Cacheänderungen zulassen.
- [ ] Die bestehende Deadline einschließlich Sourcevorbereitung und ggf.
  Remote-Ausführung einhalten, genügend Zeit für Beendigung und Commit lassen.
  Abbrüche an eigene Prozesse weiterreichen, Logs/Ergebnisse dauerhaft speichern
  und bezahlte Worker nach der Aufgabe beenden. Bestehende Disk-Regeln beachten.

## Umsetzung und minimale Abnahme

1. Gemeinsamen Einstieg mit unverändertem lokalen Ablauf verbinden; Einstellungen
   und Ablehnung unpassender Kombinationen durch vorhandene Skripttests prüfen.
2. Remote-macOS-Produktpaket samt nötigem Worker automatisch lokal übernehmen.
   An einem ohnehin notwendigen qualifizierten Build Hashes, Paketlayout und
   tatsächliche lokale Programmausführung nachweisen.
3. Linux-Produktbuild und ausdrücklich gewählte Remote-Ausführung im vorhandenen
   Worker ergänzen. Linux-Image, Runtime und Export separat passend prüfen;
   alte Apple-CLI-Qualifikation gilt dafür nicht automatisch.
4. Lokal/remote parallele Builds, wiederholten Download, Übertragungsabbruch und
   Dateikonflikt prüfen. Optionalen `current`-Verweis zusätzlich gegen einen
   inzwischen neueren lokalen Build und einen verspäteten alten Remote-Build
   prüfen: Beide dürfen keine falsche Ersetzung verursachen.
5. Nur die tatsächlich angenommenen Modi dokumentieren/aktivieren. Vorhandene
   Logs und Evidenz wiederverwenden; keine teuren reinen Timing-Wiederholungen.

Abnahme: Die drei Betriebsarten funktionieren für die ausdrücklich ausgewählten
Produktprogramme. Quellen und Cargo-Caches bleiben getrennt, Pakete kommen
vollständig an, lokale Dateien bleiben geschützt und gewählte Programme laufen
auf der passenden Plattform. Keine allgemeine Testunterstützung behaupten.

## Später: allgemeine Testmigration

Erst für einen konkreten Bedarf erweitern. Der heutige Apple-Controller ist
CLI-Dev-/Release-only; das ist durch diesen Plan noch nicht geändert.
`cargo test --no-run`, zuverlässiger Testartefaktexport und Testausführung brauchen
zusammenhängende Anpassungen in den bestehenden Validator-/Worker-/Runnerpfaden.

Insbesondere enthalten CLI-Tests `env!("CARGO_BIN_EXE_brassclaw-reborn")` und
Composition-Tests `env!("CARGO_MANIFEST_DIR")`. Andere benötigen Python-/Node-
Dateien, Datenbanken oder den benachbarten Monty-Worker. Solche Tests müssen ihre
konkreten Runtime-Pfade und Abhängigkeiten portabel auflösen, bevor man sie
zwischen Hosts verschiebt. Ein anderer Arbeitsordner oder Binary-Download allein
repariert diese Pfade nicht. Diese Arbeit ist **kein Blocker des einfachen
Produktbuild-Kerns** und wird nicht vorsorglich für die gesamte Suite umgesetzt.
