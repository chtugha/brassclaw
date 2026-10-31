# Mythbuster – Anthropic OSS Scanner und Befundbehebung

Stand: **2026-10-24**. Ziel: BrassClaw durch Anthropic untersuchen lassen,
eingehende Befunde reproduzieren und daraus priorisierte Reparaturen ableiten.

## Tatsächlicher Status

**Anmeldung als Entwurfs-PR eingereicht; noch kein Scan und keine Befunde.**
Ein Ergebnis-basierter Reparaturplan kann erst nach Eingang des Berichts
vervollständigt werden. Eine ausstehende Untersuchung bedeutet weder null
Schwachstellen noch eine bestandene Sicherheitsprüfung.

- Anmeldung: [anthropics/oss-scanner#1095](https://github.com/anthropics/oss-scanner/pull/1095).
- Anmeldebranch: [chtugha/oss-scanner, codex/enroll-brassclaw](https://github.com/chtugha/oss-scanner/tree/codex/enroll-brassclaw/projects/brassclaw).
- Ziel: `https://github.com/chtugha/brassclaw#main`, öffentliches Repository.
- Berichtskontakt: `chtugha@ganjacru.de`, vom Operator ausdrücklich angegeben.
  Die Adresse ist im öffentlichen Anmelde-PR sichtbar.
- Konfiguration, eigener Build-Dockerfile und Bedrohungsmodell liegen zusammen
  unter `projects/brassclaw/` im Anmeldebranch. BrassClaw benötigt dadurch keine
  Produktänderung und keinen neuen Rust-Tool oder installierten Recipe.
- Zusätzlich liegt das Anmeldepaket jetzt lokal in `.oss-scanner/`, ergänzt um
  die Konfiguration für Repository-eigene Pfade und eine Umstellungsanleitung.
  Der Anmelde-PR verwendet weiterhin seine bisherigen Dateikopien. Erst nach
  Veröffentlichung auf `main` auf die neuen Pfade umstellen und die Kopien im
  PR entfernen; siehe [.oss-scanner/README.md](.oss-scanner/README.md).
- Lokal beobachteter BrassClaw-HEAD bei der Vorbereitung:
  `5799cc0ef0c7fb65afd8ef59a89377810e78c395`.
  Der Scan folgt dem öffentlichen Branch; maßgeblich ist später ausschließlich
  der tatsächlich im Bericht genannte Commit. Nicht veröffentlichte lokale
  Änderungen werden nicht mitgescannt.

## Verifizierte Rahmenbedingungen

Der [OSS Scanner](https://red.anthropic.com/oss-scanner/) ist ein kostenloser,
von Anthropic betriebener Dienst mit geprüfter Projektaufnahme. Ein Anmelde-PR
startet keinen sofortigen Scan. Die Aufnahme ist insbesondere für junge Projekte
und Forks nicht garantiert; belegte Nutzung und eigenständige Sicherheitsrelevanz
müssen bei Bedarf vom Maintainer ergänzt werden.

Die [Anleitung](https://github.com/anthropics/oss-scanner#readme) verlangt einen
online baubaren Dockerfile mit allen später offline benötigten Abhängigkeiten.
Die automatisierten, menschlich nicht vorgeprüften Befunde werden an den
registrierten Kontakt versandt. `tools/validate.py` prüft die Anmeldung;
`tools/check` prüft die Build-Umgebung, nicht die Sicherheit des Codes.
Der Dienst stellt damit keinen lokal ausführbaren vollständigen Scanner bereit.

## Phase 1 – Anmeldung scanbereit machen

- [x] Öffentlichen Ziel-Branch und Berichtskontakt bestimmen.
- [x] Anmeldepaket erstellen und Entwurfs-PR eröffnen.
- [x] Offizielles `python3 tools/validate.py` erfolgreich ausführen.
- [ ] Docker-Build und Offline-Testbarkeit verifizieren.
- [x] Maintainer-Angaben zu Berechtigung und Eignung im PR vervollständigen.
- [x] CLA vom Maintainer unterschrieben; `CLAAssistant` bestätigt erfolgreich
  am 2026-10-24 um 04:53 Uhr (Europe/Berlin).
- [x] [Teilnahmebedingungen](https://red.anthropic.com/oss-scanner/terms/)
  durch den Maintainer bestätigen.
- [ ] Nach bestandenem Build den PR zur Prüfung bereitstellen.
- [ ] Aufnahme/Merge durch Anthropic und tatsächlichen Scan abwarten.

Prüfstand der Anmeldung: offizieller Scanner-Checkout
`627fc2644e03665b53fec91adf148cb808d6ba05`, ergänzt um das Anmeldepaket.
`python3 tools/validate.py` endete erfolgreich. Der Aufruf
`python3 tools/check brassclaw --no-shell --dir
/private/tmp/brassclaw-oss-scanner-build-check` stoppte mit
**`tools/check needs docker`**. Docker ist auf dem aktuellen Rechner nicht
verfügbar. Es gibt deshalb weder einen bestätigten Container-Build noch
Offline-Test-, Produktions- oder Sicherheitsakzeptanz.

Den Build auf einem geeigneten Docker-Host aus dem Anmeldebranch mit
`python3 tools/check brassclaw` nachholen. Die resultierende Offline-Shell muss
die betroffenen Tests mit echten Voraussetzungen ausführen können.
PostgreSQL nicht als root starten; der Dockerfile legt dafür `scanner` an.
Build-Fehler müssen behoben werden, bevor der PR als scanbereit gilt.
Den verbleibenden Aufwand gegen das Build-Zeit-/Ressourcenlimit des Dienstes
prüfen. Keine echten Zugangsdaten, Operator-Konfigurationen oder lokalen
Testverbindungen in das Anmeldepaket aufnehmen.

Auch auf dem vorgesehenen Testhost `brassclaw2` wurden weder Docker noch Podman
oder QEMU im SSH-PATH gefunden. Anthropics PR-CI hat den Job `project-builds`
gestartet; dessen Ergebnis ist noch offen. `CLAAssistant` ist nach der
[Unterschrift des Maintainers](https://github.com/anthropics/oss-scanner/pull/1095#issuecomment-6378977150)
erfolgreich. Der Maintainer hat auch die Teilnahmebedingungen und übrigen
Erklärungen bestätigt. Der Terms-Check erkannte zunächst die abweichende
Formulierung „Maintainer accepts …“ nicht; die PR-Beschreibung wurde auf das
vom Workflow verlangte „I accept …“ berichtigt. Der neu ausgelöste
[Terms-Check](https://github.com/anthropics/oss-scanner/actions/runs/40056275825)
ist erfolgreich. Der Build läuft weiterhin; sein Häkchen bleibt bis
zu tatsächlicher Build-/Offline-Test-Evidenz offen. Der PR bleibt ein Entwurf.

### Erste Rückmeldung aus der Anmeldeprüfung

Der [automatische Triage-Kommentar im PR](https://github.com/anthropics/oss-scanner/pull/1095)
bestätigt Maintainer-Eigenschaft, Lizenz, Dateiumfang und erfolgreiche
Konfigurationsprüfung. Er warnt vor geringem Projektalter und fehlenden
öffentlich sichtbaren Nutzungssignalen (jeweils 0 Stars/Forks/Watcher und keine
externen Teilnehmer in den untersuchten jüngsten Vorgängen). Der gemeldete
OpenSSF-Criticality-Score beträgt 0,41. Eine `SECURITY.md` zur Bestätigung des
Berichtskontakts wurde nicht gefunden. Dies sind automatische Aufnahmeindikatoren,
keine Sicherheitsscan-Ergebnisse und noch keine Ablehnung.

Nächster Schritt: vorhandene tatsächliche Nutzung und die eigenständige
Sicherheitsrelevanz für den Reviewer belegen. Keine Nutzungszahlen erfinden.
Falls Anthropic den Kontakt nicht zuordnen kann, einen vom Maintainer bestätigten
Sicherheitskontakt dokumentieren. Bei Ablehnung diesen Grund hier festhalten;
ein anderer Audit wäre ein gesonderter Auftrag und kein OSS-Scanner-Ergebnis.

## Phase 2 – Bericht übernehmen und Befunde prüfen

Der Bericht kommt per E-Mail. In dieser Sitzung besteht kein Zugriff auf das
Postfach. Nach Eingang muss der Maintainer die relevanten Berichte für die
Bearbeitung bereitstellen. Vertrauliche Berichte und Reproducer zunächst nur
mit berechtigten Maintainern teilen; vor einem öffentlichen Commit dieses Plans
keine ungepatchten Exploit-Details ergänzen.

Für jeden Befund erfassen: Anthropic-ID, Scan-Commit, betroffene Konfiguration
und Datei/Zeile, Angreifervoraussetzungen, behaupteter Grenzübertritt,
Reproducer, beobachtetes Verhalten, Schweregrad und vorgeschlagene Reparatur.
Den alten Scan-Stand und die aktuelle Implementierung vergleichen: behoben,
weiter vorhanden, nicht reproduzierbar oder noch ungeprüft.

| Bericht | Scan-Commit | Ergebnis der Reproduktion | Priorität | Zuständiger Plan | Reparatur / Nachweis |
| --- | --- | --- | --- | --- | --- |
| Noch kein Bericht eingegangen | offen | nicht untersucht | offen | nach Befund bestimmen | offen |

Diese Zeile ist ein Statusplatzhalter und kein Scanner-Befund. Prüfschwerpunkte
aus dem Bedrohungsmodell sind ebenfalls keine nachgewiesenen Schwachstellen.

## Phase 3 – Befunde in konkrete Arbeitspakete überführen

1. **Reproduzieren:** isolierte Umgebung, synthetische Geheimnisse, exakter
   Commit und tatsächlich betroffener Produktionspfad. Fehlende Voraussetzungen
   oder übersprungene Tests als offen dokumentieren.
2. **Priorisieren:** zuerst erreichbare unautorisierte Codeausführung,
   Geheimnisabfluss und Sandbox-/Policy-Umgehung; danach übrige bestätigte
   Grenzverletzungen. Authentifizierter, absichtlich erlaubter Operatorzugriff
   allein begründet keinen Befund.
3. **Zuordnen:** den bestehenden Verantwortlichen aus
   [implementation-status.md](docs/implementation-status.md) verwenden.
   Authority/Lifecycle/Recovery gehört in `simplified_v3.md`, Q1/Q2 und
   Komponentenfreigabe in `validator_v3.md`, Bindings/Komponentenmigration in
   `docs/plans/monty.composition.md`, finale Modell-/Prefix-Grenzen in
   `docs/plans/prefix_v3_upgrade.md`. Andere Befunde im tatsächlich betroffenen
   Subsystem beheben. Dieser Plan verfolgt Befunde, keine parallele Architektur.
4. **Beheben:** kleinstes zusammenhängendes Arbeitspaket mit Regressionstest.
   Bei Komponenten zuerst Recipe und vorhandene Usages prüfen; die vier
   Komponentenguides gelten. Neue Infrastruktur nur an ihrer zuständigen Grenze.
5. **Validieren:** ursprünglichen Reproducer und passende Negativfälle auf der
   reparierten Version ausführen. Für Sicherheits-/DB-/Runtime-Änderungen echten
   Integrationspfad nach [Entwicklungspolicy](docs/development-policy.md)
   nachweisen. Ein Compilerlauf oder Source-Review allein schließt keinen Befund.
6. **Abschließen:** Fix-Commit, Testnachweis, Restumfang und gegebenenfalls
   Release verknüpfen. Bericht-ID zur Attribution erhalten. Unbestätigte Fälle
   mit Begründung offenlassen oder als Fehlalarm schließen.

## Abschlusskriterium

Die Scan-Aufgabe ist erst erfüllt, wenn Anthropic tatsächlich gescannt hat und
die empfangenen Ergebnisse hier nachvollziehbar bewertet und in konkrete
Arbeitspakete überführt wurden. Eine fertige Anmeldung oder ein Bericht ohne
Befunde ist kein Nachweis vollständiger Sicherheit oder vollständiger v3-Umsetzung.
