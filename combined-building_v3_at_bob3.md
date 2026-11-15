# Combined Building v3 at Bob3 — Build-Skripte angleichen

Stand: 2026-11-15. Die unten beschriebenen Apple-Anschlüsse sind implementiert.
68 lokale Build-/Controllerprüfungen und sechs Source-Sync-Prüfungen bestehen.
Run88 kompiliert neun aktuelle native Testprogramme warnungsfrei; drei tatsächliche
Compiler-/PG-/Modell-/Monty-Fälle bestehen aus dem vorhandenen Programm, mit
explizitem Ressourcenpfad und normalem Worker neben `deps/`. Neue Remote-Wege sind damit noch nicht image-/laufzeitqualifiziert:
das bisherige Image enthält den geänderten Worker nicht. Kein Modal-Image wurde
für diese Skriptänderung neu gebaut oder aktiviert; kein bezahlter Compilejob läuft.
Linux-Testarbeit bleibt ohne konkreten passenden Prüfauftrag ausdrücklich offen.
Häkchen bezeichnen implementierten Code, keine pauschale Plattformabnahme.

## Ziel und logisch begründbarer Nutzen

Die vorhandenen Skripte sollen benötigte Produkt- und Testbuilds lokal oder
remote mit passenden Einstellungen ausführen können. Ihr Ausbau beschränkt sich
auf Aufrufkompatibilität, vorhandene Caches und die Übergabe benötigter Artefakte.

Drei Vereinfachungen sparen Arbeit, ohne eine neue Testorganisation einzuführen:

1. Ein benötigtes Testziel einmal kompilieren und daraus mehrere benötigte Fälle
   ausführen; nicht für jeden Fall erneut denselben Build vorbereiten.
2. Bereits passende Programme, Begleitworker und gültige Ergebnisse weiterverwenden,
   statt vorsorglich Produkt-, Check-, Dev-, Release- und Testbuilds zu stapeln.
3. Unabhängige lokale und Remote-Arbeit mit getrennten Caches überlappen, statt
   beide durch eine gemeinsame Build-Sperre zu serialisieren.

Daraus folgt weniger vermeidbare Arbeit. Daraus folgt **nicht**, dass jeder
Remote-Build schneller ist: Upload, Containerstart, Cross-Linking und Download
kosten Zeit. Remote nur verwenden, wenn diese Zusatzzeit durch kürzere
Kompilierung oder vermiedene lokale Wartezeit voraussichtlich ausgeglichen wird.
Ein warmer lokaler Inkrementalbuild bleibt lokal, wenn Auslagern keinen Vorteil
erwarten lässt. Keine neuen Benchmarks oder Speedup-Zertifikate verlangen.

Die Angleichung beschleunigt die Organisation und gegebenenfalls Kompilierung
von Produkt- und Testcode. Sie verkürzt keine fachlich notwendigen Wartezeiten
innerhalb eines Tests. Ein erneuter Cargo-Aufruf kompiliert dank Cargo-Caching
auch heute nicht zwangsläufig neu; eingespart werden nur tatsächlich redundante
Aufrufe, Vorbereitungen und Artefaktkopien. Zusätzliche Hashprüfungen/Transfers
nicht als kostenlose Beschleunigung behandeln.

## Aktueller Code: wiederverwenden statt neu bauen

| Aufgabe | Vorhandener Eigentümer / aktueller Anschluss |
| --- | --- |
| Native Cargo-Ausführung | `scripts/local_build.py`: globales Cargo-Parsing, Toolchain-Auswahl, Jobs, lokale Sperre und Disk-Prüfung bereits vorhanden |
| Remote-Aufnahme und Abholung | `scripts/modal_build.py`: Jobbelege, Workeridentität, gemeinsames `collect_result` und atomarer Download bereits vorhanden |
| Remote-Compile/Export | `scripts/modal_build_worker.py`: Produktbuild sowie eng begrenzter Apple-Composition-Libtest-Compile-only-Pfad vorhanden |
| Buildparameter | `scripts/build_settings.py`, `scripts/build-environments.toml` |
| Einseitiger Quellabgleich | `scripts/modal_source_sync.py`: geprüfter Snapshot, Delta und getrennte Remote-Sperre vorhanden |
| Apple-Werkzeuge | `scripts/modal_apple_toolchain.py`: OSXCross-/SDK-/Rust-Image |

### Geprüfter Umsetzungsstand

**Implementiert** bezeichnet den verbundenen Quellstand mit den genannten
lokalen Skript- und nativen Prüfungen. Transport-/Header-Fixtures in Skripttests
sind keine echte Remote-Kompilierung. Häkchen bezeichnen fertigen Code;
Image-/Remoteaktivierung und konkrete Plattformabnahme bleiben eigene Aussagen.
Die erhaltenen älteren lokalen Imagebelege unten aktivieren den neuen Worker nicht.

| Teil | Status | Konkreter Befund / verbleibende Arbeit |
| --- | --- | --- |
| Nativer Cargo-Aufruf | Vorhanden | `local_build._cargo_subcommand`, `run_local`: Toolchain-/Optionsparsing, Jobs, kanonischer Targetpfad, Cleanup-Schutz, zwei Disk-Gates und Sperre |
| Gemeinsamer lokal/remote-Einstieg | Implementiert | `local_build.main --host local|modal --platform apple|linux` delegiert ohne neue Queue; lokale Aufrufe bleiben Standard und ohne Cloudabfrage |
| Hostbezogene Einstellungen | Vorhanden | `load_settings`/`modal_resources` trennen lokale Jobs von Modal-Ressourcen und Apple-/Linux-Overrides |
| Source-Sync, Caches und parallele Hosts | Vorhanden | `capture`/`sync_source`, getrennte `cargo_lock`/`modal_lock`, `materialize`/`update_cache`, Image-spezifischer Remote-Targetcache |
| Apple-CLI-Produktbuild | Implementiert; Imageabnahme offen | CLI Dev/Test/Release bleibt ein normales Programm; ein eigener normaler `monty_worker` wird über denselben vorhandenen Cache gebaut |
| Apple-Composition-Testkompilierung | Angeschlossen; Imageabnahme offen | Exakter `--lib --no-run`-Collector plus normaler Worker und lokale exakte Testauswahl; kein beliebiger Harness |
| Testfeatures des benötigten Aufrufs | Implementiert | `brassclaw_pg/test-support` wird am tatsächlichen nichtoptionalen Dependency-Manifest geprüft und getrennt von Composition-Features im Subject behalten |
| Exakter Testbinary-Export | Vorhanden, begrenzter Testmodus | `collect_test_artifacts` prüft aktuellen Cargo-Erfolg, Package/Target/Features, `profile.test` und auch `fresh`; `snapshot_executable` kopiert genau ein verifiziertes Mach-O-Executable |
| Abholung und Wiederaufnahme | Vorhanden | `collect_result`, `atomic_download`, `status`: Request-/Receiptprüfung, temporärer Hashdownload, Wiederverwendung passender lokaler Bytes, Nachholen ohne neuen Build |
| Vollständiges Runtimepaket | Implementiert; echte Remoteabnahme offen | `deps/<Testbinary>`, normaler Worker daneben; aktuelle Cargo-Meldungen wählen PG-Ausgabe; Archive und System-Dylib-Abhängigkeiten werden geprüft, nicht benötigte PG-Laufzeitdaten bleiben eingebettet |
| Automatische lokale Testausführung | Implementiert; echte Remoteabnahme offen | `run --native-test <exakter Name>` / `accept-native --test` nutzen direkt das gesammelte Programm und bestehenden Native-Lock; echte Liste, genau ein erfolgreicher/nicht ignorierter Fall je Name, keine Wiederholung |
| Linux-Testkompilierung/-ausführung | Offen | `command_options` lehnt Linux-`cargo test` ab; Worker führt Cargo aus, aber keinen Testlauf danach |
| Policy-Gate | Korrigiert | Kein technisches 60-Sekunden-Minimum mehr; explizite Modalwahl und Bildaktivierung bleiben nötig; freiwillige Laufzeitschätzung muss endlich/nichtnegativ sein |
| Compilerwahl | Explizit | Apple bleibt auf den bestehenden Image-Compiler 1.98.1 gepinnt; Linux erhält `modal.linux.compiler = "1.98.1"`, unabhängig vom MSRV |
| Schutz nativer lokaler Ausgaben | Vorhandene Trennung | Downloads landen im eigenen Jobverzeichnis, nicht im nativen Cargo-Ausgabepfad; kein `current`-Mechanismus vorhanden oder erforderlich |
| Konfliktregel innerhalb eines Jobverzeichnisses | Implementiert | Identische Bytes werden wiederverwendet, fehlende Kopien nachgeholt; abweichende veröffentlichte Bytes/Symlinks bleiben als Konflikt erhalten. Private Downloadreste werden verworfen, nie über fremde Bytes veröffentlicht |
| Regulärer Apple-Testbetrieb | Aktivierungsanschluss implementiert, derzeit nicht qualifiziert | Eigene `--test-mode`-Aktivierung benötigt aktuelle Worker-/Imageidentität, vollständiges Paket und echte erfolgreiche Native-Fälle; alte Produktaktivierung genügt nicht |

**Lokale Imagebelege:** Unter
`/Users/ollama/brassclaw-target/modal-build/` referenziert `image-apple.json`
das vorbereitete Rust-1.98.1-Image `im-Wxm74Dwhp6v20AWvy6T6aj`, enthält aber
keine Identität/Modusdeklaration des jetzigen Testworkers. Der aktuelle
Testcontroller lehnt dieses alte Receipt deshalb vor Aufnahme ab.
`activation-apple.json` verweist weiterhin auf das andere, frühere Image
`im-S1cpT9G1fsMPpwjISvBSlK`; es aktiviert weder das vorbereitete neue Image noch
den Testmodus. Linux-Image-/Aktivierungsreceipts sind an diesem lokalen
Records-Pfad nicht vorhanden. Das ist eine lesende lokale Bestandsaufnahme,
keine Abfrage des aktuellen Modal-Kontostands.

Der Testmodus `apple-composition-lib-test-build/1` akzeptiert bereits
`cargo test --no-run`, das Apple-Target, Composition `--lib`, das Testprofil und
JSON-Artefaktmeldungen. Der lokale Ausführungspfad und die separate Aktivierung sind angeschlossen.
Normaler Testbetrieb bleibt ohne tatsächliche Image- und Native-Abnahme geschlossen. Er ist kein allgemeiner Linux-Testmodus.

`--profile test` ist gültige Cargo-Syntax und im begrenzten Testmodus bereits
vorgesehen. Dagegen bleibt der Apple-Produktmodus derzeit auf CLI Dev/Release
beschränkt. Diese verschiedenen Grenzen nicht zu einer pauschalen Aussage
„Tests unterstützt“ oder „Testprofil unmöglich“ zusammenziehen.

[Entwicklungspolicy](docs/development-policy.md),
[Build-Umgebungsvertrag](docs/build-environments.md) und
[Modal-Vertrag](docs/modal-cross-build.md) bleiben maßgeblich. Die bestehende
Nutzerfreigabe erlaubt zeitersparende Remote-Testarbeit ohne erneute Standortfrage,
ersetzt aber weder fehlende technische Unterstützung noch Image-Aktivierung.
Andere Entwicklungs-/Retention-/Installerpläne erhalten hier keinen zweiten Owner.

## 1. Bestehende Aufrufe und Einstellungen verbinden

Code angeschlossen: gemeinsamer Einstieg, geltende Policy und explizite Linux-Compilerwahl.
Die tatsächliche Remote-Abnahme bleibt an das neue Image gebunden.

- [x] Vorhandene Basis: lokaler direkter Cargo-Modus einschließlich `+toolchain`,
  unterstützter globaler Optionen, Package-Cleanup-Schutz und beider Disk-Gates.
  Remote bleibt eine bewusst begrenzte Cargo-Grammatik; lokale Freiheiten nicht
  ungeprüft auf den Remote-Worker übertragen.
- [x] Einen dünnen gemeinsamen Einstieg nur für die Auswahl lokal/remote,
  Produkt-/Testbuild und Target ergänzen. Er delegiert an bestehende Funktionen;
  keine neue Scheduler-, Auftragsdatenbank- oder Prognoseschicht. Keine neue
  `auto`-Heuristik als Voraussetzung: Ort nach vorhandenem Arbeitskontext und
  vorhandenen Zeiten wählen; unbekannte Vorteile nicht erfinden.
- [x] Package, konkretes Cargo-Ziel, Profil, Features und Target unverändert
  passend weiterreichen. Keine Default-Features hinzuerfinden. Das alte
  technische 60-Sekunden-Gate an die geltende Policy anpassen, nicht durch einen
  fingierten Schätzwert umgehen. Ressourcen zunächst unverändert lassen.
- [x] Die tatsächlich verwendete Compiler-Version pro Host prüfen/auswählen;
  MSRV aus `Cargo.toml` ist nicht automatisch der gewünschte Compiler. Kein
  Versionswechsel oder Image-Neubau pro Auftrag, kein unnötiges Cache-Warming.

## 2. Produkt- und Testkompilierung kompatibel machen

Code angeschlossen: Produktprofile, normaler Worker, Dependency-Feature und verbundener
Apple-Testablauf. Linux-Tests bleiben ohne konkreten Auftrag offen.

- [x] CLI-Produktbuild mit benötigtem normalem `monty_worker` und den wirklich
  verlangten Profilen anschließen. Das Produktprogramm im Profil `test` ist
  weiterhin ein normales Programm, kein Libtest-Harness. Nur diese konkret
  benötigte Produkt-Profil-Erweiterung ergänzen, nicht beliebige Cargo-Targets.
- [x] Den vorhandenen Apple-Composition-Testexport fertig anschließen, statt
  ihn erneut zu implementieren. Der benötigte Aufruf bestimmt Features und
  Profil; der Validator darf erlaubte Cargo-Syntax nicht irrtümlich ablehnen.
  Konkret: Der bisherige `test_subject()` akzeptiert nur benannte Composition-
  Features; der bestehende Prüfauftrag nutzt auch `brassclaw_pg/test-support`.
  Diese tatsächlich benötigte Dependency-Featureform gezielt unterstützen und
  korrekt dem Package zuordnen, ohne einen neuen allgemeinen Feature-Resolver.
- [x] Im begrenzten Apple-Testmodus: aufgenommene Auswahl und von Cargo gemeldete Artefakte abgleichen.
  `fresh=true` ist zulässig; das gemeldete aktuelle Executable verwenden.
  Unit-Test-Targetkind kann `lib` sein. Nicht im gesamten Cache nach irgendeinem
  gleichnamigen/neuesten Programm suchen.
- [ ] Linux-Testkompilierung/-ausführung nur für einen ohnehin benötigten,
  kompatiblen Prüfauftrag ergänzen. Sie ist keine Vorbedingung für den
  Apple-Testbuild und kein verpflichtender zusätzlicher Pilotbatch. Wenn der
  Test auf Linux im Compile-Job läuft, nur Ergebnisse zurückholen.
- [x] Einen Testharness pro benötigtem Eingabestand bauen und direkt mit den
  benötigten Filtern verwenden. Produktprogramme aus demselben Build nur dann
  mitverwenden, wenn Cargo sie tatsächlich erzeugt und Profil/Features passen.
  Composition `--lib` erzeugt nicht automatisch die CLI. Unveränderten normalen
  Worker nicht vorsorglich zusätzlich in jedem Profil bauen.

## 3. Host und Target getrennt behandeln; Caches erhalten

Stand: Source-Snapshot, getrennte Caches/Sperren und der Apple-/PG-Cross-Zweig
sind vorhandene Basis. Kein Neubau dieser Infrastruktur erforderlich; bei neuen
Aufrufarten nur deren Anschluss an die bestehenden Verträge ergänzen.

- [x] `HOST` bezeichnet die Ausführungsplattform der Build-Helfer, `TARGET` das
  Produktziel. Build-Skripte wählen danach ihre vorhandenen Compiler-/SDK-Pfade;
  `cfg!(target_os=...)` im Build-Skript nicht als Produkt-Target auslegen.
  Den funktionierenden PostgreSQL-/Apple-Cross-Zweig wiederverwenden.
- [x] Für remote macOS OSXCross/Apple-SDK, für remote Linux die Linux-Werkzeuge
  verwenden. Keine Apple-Variablen in Linux-Aufträge übernehmen. Linux-Programme
  und macOS-Programme ausschließlich in der passenden Umgebung ausführen.
- [x] Native Helfer/Proc-Macros werden von Cargo je Compiler-Host gebaut.
  Deren Quellcode nicht pauschal ändern und ihre Binärdateien nicht zwischen
  Linux und macOS kopieren. Gleicher Rust-Stand macht diese Caches nicht identisch.
- [x] Vorhandene Basis: kanonischen lokalen Targetpfad und bestehenden Image-spezifischen
  Remote-Cache erhalten. Kein Target-Sync, kein neuer nativer Buildbaum, kein
  allgemeiner Cache-Dienst. Unveränderte Remote-Source-Dateien behalten wie
  bisher ihre Zeitstempel; keine unnötige Invalidierung durch Touching/Clean.
- [x] Bestehende getrennte Sperren beibehalten: ein lokaler und ein Remote-Job
  können parallel laufen, derselbe mutable Cache bleibt seriell. Quellabgleich
  nur lokal → remote. Native Compilerinputs während des Builds nicht verändern.
  Der Worker im Image muss dem verwendeten Controllerprotokoll entsprechen.

## 4. Genau benötigte Artefakte übernehmen und verwenden

Code angeschlossen: Runtimepaket, Runner und Konfliktunterscheidung.
Eine erfolgreiche Skriptprüfung ersetzt keine echte neue Image-/Native-Abnahme.

- [x] Vorhandenen Export/Collector für Tests weiterverwenden. Nur fehlende
  Produkt-/Worker-Übergaben ergänzen. Benötigte Dateien vor Cachefreigabe in ein
  eigenes Jobpaket kopieren; keine Hardlinks auf später überschreibbare Ausgaben.
- [x] Das Paketlayout erhalten: Produktprogramme/`monty_worker` nebeneinander,
  Testbinaries in `deps/`. Damit funktioniert die vorhandene Worker-Nachbarsuche
  ohne neue Produktions-Overrides. Passende Bibliotheken und erforderliche
  PG-/pgvector-Bestandteile mitberücksichtigen; keine leeren Platzhalter als
  funktionsfähige Laufzeit ausgeben.
- [x] macOS-Testpakete automatisch lokal bereitstellen und den vorhandenen
  Execution-Runner auf den exakten Testbinary-Pfad richten. Nicht erneut lokal
  `cargo test` starten, um das gerade gelieferte Programm wieder zu bauen.
  Bestehende Backend-/Fixture-Locks und Testfilter beibehalten.
- [x] Nur bei tatsächlich ausgewählten Tests nötige Laufzeitpfade anpassen:
  `env!("CARGO_MANIFEST_DIR")` und `env!("CARGO_BIN_EXE_...")` enthalten ggf.
  tote Remote-Pfade. Ein anderer Arbeitsordner repariert sie nicht. Bestehenden
  Testhelper gezielt um einen expliziten Paket-/Ressourcenpfad ergänzen; keine
  pauschale Suite-Migration oder neuen Testframeworks. Fehlt ein tragfähiger
  Laufzeitanschluss, diesen Auftrag lokal belassen bzw. vor Remote-Start ablehnen.
- [x] Rücktransfer ausschließlich in den vorhandenen eigenen Jobbereich unter
  `$CARGO_TARGET_DIR/modal-build/jobs/<job-id>/`. Native Cargo-Ausgaben, lokale
  Quellen und fremde Jobs niemals überschreiben. Identische Job-/Hashdaten
  wiederverwenden; abweichende veröffentlichte Bytes als Konflikt behandeln.
  Neue Versionen erhalten neue Pakete. Ein gemeinsamer `current`-Verweis ist
  nicht erforderlich. Falls später ergänzt: nur nachgewiesenen Nachfolger
  derselben Artefaktidentität atomar veröffentlichen, nie einen verspäteten
  Remote-Stand über einen inzwischen neueren lokalen Stand setzen.

## 5. Keine zusätzliche Validierungsbürokratie

Jobreceipts, atomarer Collector und Status-Recovery bleiben erhalten.
Neue Laufzeit-Anschlüsse sind im Code vorhanden, ihre tatsächliche Remote-Abnahme
bleibt sichtbar getrennt von Collector- und Skriptprüfungen.

- [x] Vorhandene Request-/Source-/Completion-/Artefaktbelege weiterverwenden.
  Keine neue Zertifizierung, vollständige Prüfmatrix, Pflichtbenchmarks,
  zusätzliche Testserie oder separates Speedup-Abnahmeprojekt einführen.
- [x] Bestehende Sicherheits-, Disk-, Hash-, Aktivierungs- und Ressourcenprüfungen
  erhalten. Ein Test mit falschem Binary oder null verlangten ausgeführten
  Fällen zählt nicht als erfolgreich; dafür kein neues Prüfverfahren erfinden.
- [ ] Normale notwendige Arbeit genügt zur praktischen Bestätigung: den nächsten
  tatsächlich benötigten Build über den geänderten Pfad ausführen und die
  ohnehin verlangten Tests aus genau dessen Artefakten laufen lassen. Keine
  zusätzliche große Neukompilierung ausschließlich für diesen Skriptumbau.
  Die nächsten normalen Logs können die Standortentscheidung verbessern;
  eine Vorher-/Nachher-Benchmarkserie ist kein Abschlusskriterium.
- [x] Nur wenn tatsächlich geänderte Parsing-/Collectorlogik eine vorhandene
  lokale Skriptregression betrifft, diese passende Prüfung wiederverwenden
  oder gezielt ergänzen. Keine Rust-/Datenbanktests für bloße Argumentangleichung
  hinzufügen. Neue Ausführungsmöglichkeiten trotzdem nicht allein aus
  Quellinspektion als bereits funktionsfähig/qualifiziert behaupten.
- [x] Vorhanden: Unterbrochene Übernahme mit `status`/`collect_result` fortsetzen,
  ohne neuen Compilejob. Client-/Transportfehler und Remote-Ende werden getrennt
  behandelt; `collection_pending` bewahrt den bekannten Remote-Abschluss.
- [x] Für die angeschlossenen Ausführungswege keine automatische Testwiederholung
  hinzufügen. Deadline einschließlich Vorbereitung und Shutdown-/Commitreserve
  korrekt weiterverwenden: der Watchdog erhält nur das um
  tatsächliche Vorbereitung und Abschlussreserve verminderte Restbudget, keine
  neue volle Frist pro Buildphase.

Für Start, Begleitung und Abschluss von Screen-Läufen gilt verbindlich die
jeweils aktuelle globale Agent-Regel **„Screen-Builds und offene Chat-Turns“**.
Die dort vorgesehenen Skripte umschließen bei Bedarf den bestehenden lokalen
Buildaufruf oder Modal-Controller; Build-Locks und Ressourcenregeln bleiben
wirksam. Keine zweite Screen-Überwachungslogik und keine zusätzlichen Tests
für diese Einbindung einführen.

## Reihenfolge und Abschluss

1. Vorhandene Produkt-/Testaufrufe und Grenzen angleichen; native Kompatibilität
   erhalten. Bereits vorhandene Testcollector-/Statuslogik nicht neu schreiben.
2. Benötigten Apple-Testbuild mit sicherer lokaler Übernahme verbinden; danach
   genau die fachlich ohnehin benötigten Fälle ausführen.
3. Nur bei konkretem Arbeitsbedarf Linux-Tests remote ergänzen und unabhängige
   Arbeit parallel legen. Kein zusätzlicher Testbatch nur zur Demonstration.

Fertig ist die Skriptangleichung, wenn die benötigten Aufrufe dieselbe Auswahl
korrekt an den passenden Host weiterreichen, dort vorhandene Caches nutzen und
vorhandene Runner die richtigen Artefakte erhalten. Erwartbare Beschleunigung
folgt aus vermiedener Doppelarbeit und günstigerer Verteilung; universell kürzere
Laufzeiten sind ohne Hardware-/Cache-/Transferbedingungen logisch nicht beweisbar.
Ein nicht vorteilhafter Remote-Auftrag bleibt lokal. Es gibt kein Geschwindigkeits-
versprechen, das durch zusätzliche Builds erst erzwungen werden müsste.

## Konkreter Anschluss und verbleibende Abnahme — 2026-11-15

- Gemeinsamer Einstieg: `python3 scripts/local_build.py --host modal --platform apple --confirmed-modal --qualification --native-test <exakter Libtestname> --test-timeout-secs <bestehende Falldeadline> -- cargo test --locked --no-run --lib -p brassclaw_reborn_composition --features skills-db,brassclaw_pg/test-support --target aarch64-apple-darwin`.
  `--qualification` darf nur für den tatsächlich nächsten benötigten Image-Abnahmelauf verwendet werden; es aktiviert nichts. Ohne diese Option gilt die bestehende Aktivierung. Lokale Aufrufe ohne `--host modal` fragen weder nach Standort noch nach Modalzugang.
- `accept-native <Job-ID> --test <exakter Name> --test-timeout-secs <Deadline>` nimmt ein bereits abgeholtes Testpaket an. `--test-env <lokales JSON>` liefert bestehende Fixturepfade, nicht Modell-/Produktkonfiguration. Der Runner verifiziert die Original-Quellen und setzt `BRASSCLAW_TEST_SOURCE_ROOT` für die betroffenen vorhandenen Testhelfer. Er startet kein Cargo und wiederholt nach Fehlern keine Fälle automatisch.
- `activate --test-mode <Job-ID>` schaltet ausschließlich den unabhängig qualifizierten Testmodus dieser Worker-/Imageidentität frei. Produkt- und Testaktivierung bleiben getrennt. Eine neue Workerdatei im Source-Sync ersetzt keinen unveränderlichen Image-Worker.
- Source-Vorbereitung, Produkt und notwendiger normaler Worker teilen sich eine monotone Gesamtdeadline mit der vorhandenen 120-Sekunden-Abschlussreserve. Ein abgelaufener Vorbereitungsanteil startet kein Cargo mehr.
- Noch auszuführen ist die echte neue Image-/Remote-/Native-Verbindung beim nächsten ohnehin benötigten, vorteilhaften Apple-Auftrag. Bisherige Produktbelege werden nicht als neue Testabnahme verwendet. Keine zusätzliche große Kompilierung oder Linux-Testserie nur zur Demonstration starten.

### Aktuelle lokale Belege

- Run87: `cargo clippy --all-targets -- -D warnings` unter Rust 1.100 für die
  tatsächlichen Production-/Testfeatures endet mit 0 und ohne Warnungen. Der
  breite Source-Guard schlägt wegen paralleler Dokumentationsbearbeitung fehl;
  dieser Beleg bleibt erhalten. Die separate Review beansprucht nur unveränderte
  Rust-/Lintlogik. Geänderte eingebettete Texte sind erst von Run88 erfasst.
- Run88: Rust 1.98.1, neun vorhandene Testprogramme im bisherigen gemeinsamen
  Featuregraph, keine Warnungen, vollständige native Unwind-Sektionen. Der
  unveränderte normale Worker wird nur nach Source-/Compiler-/Umgebungs-/Hash-
  Nachweis wiederverwendet. Keine zweite Kompilierung für die drei Laufzeitfälle.
- Echter Sandbox-Start mit privatem `SCREENDIR` und Desktop-IPC funktioniert.
  Die PG-Initialisierung benötigt nachgewiesenermaßen den dort gesperrten
  `shmget`-Aufruf; nur dieser native DB-Test braucht eine passende Ausführung.
- Der vorbestehende HF-Fixture lädt jetzt seine echte `AutoConfig`; die bisherige
  Oracle-Umgebung hatte nur noch Python-Verknüpfungen. Eine isolierte Umgebung
  unter `target/test-tools/oracle-20261115` auf der SSDE nutzt die bereits
  vorhandenen `requirements-oracle.txt`. Originale Modell-/Templatebytes und die
  Warnungsprüfung bleiben erhalten. Der pip-Cache ist dort ebenfalls tatsächlich
  beschreibbar, ohne globale Installationen zu ändern.
- Details und Fehlerbelege: [Abnahme und Branchbereinigung](docs/validation/2026-11-15-combined-build-and-branch-cleanup.md).
