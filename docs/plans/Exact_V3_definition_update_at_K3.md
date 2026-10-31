# Exact V3 definition update at K3

**Stand:** 2026-10-28. **Status:** Dokument-/Contractabgleich umgesetzt; keine
Runtimeaktivierung oder installierte Paketübernahme behauptet. Der anschließende
Betreiberauftrag „implement the exact plan“ autorisiert die begrenzten D-Pakete.
Die Disposition jedes Inventareintrags und die Ergebnisse stehen in §12 und im
[Umsetzungsnachweis](../validation/2026-10-28-exact-definition-update.md).
Die frühere Skill-/Recipe-Textkorrektur in 67 Dateien ist Ausgangsmaterial, kein
Nachweis vollständiger Architekturkonformität oder installierter Übernahme.

**Historische Auftragsgrenze:** Erstellung und Audit dieses Plans waren reine
Markdownarbeit. Der spätere Implementierungsauftrag liefert jetzt die nachfolgend
beschriebenen Dokument-/Beispielkorrekturen; zukünftige Aktivierungs-/Runtimegates
werden dadurch nicht als umgesetzt markiert. Bereits separat autorisierte Arbeiten
anderer Chats laufen in ihrem bisherigen Umfang weiter. Dieser Plan widerruft
weder deren Auftrag noch ihre vorhandenen Nachweis- und Integrationsvereinbarungen.

**Korrigierte Einordnung:** K0–K3 sind nach Betreiberangabe bereits durchgelaufen
und bleiben abgeschlossen und unverändert. Sämtliche neuen Änderungen und
Nachweise dieses Plans beginnen erst ab K4. Der angeforderte Dateiname „at_K3“
bezeichnet den Entstehungskontext, keine neue Arbeit oder Voraussetzung für K3.

**Zusätzliche Abschnittsgrenze:** Auch innerhalb der fünf maßgeblichen v3-Pläne
werden bereits umgesetzte oder abgenommene Abschnitte nicht rückwirkend geändert,
gelöscht, verschoben oder neu beschriftet. Deren Fundstellen sind ausschließlich
Lesereferenzen. Neue Präzisierungen stehen in ausdrücklich offenen Folgeabschnitten
ab K4. Diese Regel hat Vorrang vor jedem verkürzten „ersetzen“, „löschen“ oder
„historisch kennzeichnen“ weiter unten; §1.1 bestimmt das zulässige Änderungsziel.

## 1. Zweck, Autorität und Arbeitsgrenze

Agents sollen aus den Guides, Plänen und Beispielen dieselbe Architektur ableiten
und tatsächlich unterstützte Konstruktoren, Datenverträge und Ausführungspfade
benutzen. Dazu aktuelle Anweisungen, Zielentwürfe, Kompatibilität und historische
Beobachtungen lokal unterscheidbar machen. Vorhandene Unterstützung nicht neu
implementieren; fehlende Unterstützung nicht durch angenommene Felder ersetzen.

Verbindliche Komponentenverträge bleiben [recipe.md](../../recipe.md),
[skills.md](../../skills.md), [tools.md](../../tools.md) und
[toolskills.md](../../toolskills.md). Die übergreifende Zielarchitektur bleibt
[simplified_v3.md](../../simplified_v3.md). Quellen-/Statusbehauptungen mit dem
[Implementierungsindex](../implementation-status.md) und tatsächlichen Callern
abgleichen. [Entwicklungspolicy](../development-policy.md) bestimmt Checkauswahl,
Evidenzwiederverwendung und Abschluss; dieser Plan schafft keine zweite Policy.

Mit folgenden sieben Plänen zusammenführen, nicht durch einen achten allgemeinen
Architekturtext ersetzen:

1. [Simplified](../../simplified_v3.md): K0–K7, Phasen 0a/3a/6–8, §§8.2/9/10.
2. [Validator](../../validator_v3.md): Steps 1–12 und Coding-Anweisungen §7.
3. [Composition](monty.composition.md): §§4–7/10/16/20/23–26 und aktuelle Nachweise.
4. [Komponentenimplementierung](monty.composition.implementation.md): gemeinsame
   Verträge, einzelne Nutzungen, Pure-Beispiele und R01–R10.
5. [Prefix](prefix_v3_upgrade.md): Quellen/Lehrmaterial, Jobs/Serving, Gates A1/C2
   und gemeinsame Upgrade-/Recovery-Anbindung.
6. [Sempai v3.5](sempai_v3.5.md): beide Review-Zwecke, Steps 2/6/10–12.
7. [Connections v3.5](simplify_connections_v3.5.md): Phasen D–F und Abnahme.

Fundstellen sind Suchhinweise, keine Patchkoordinaten. Vor jeder Änderung Pfad,
vollständige Überschriftenkette, charakteristisches Zitat und aktuellen Consumer
gemeinsam auflösen. Zeilennummern, alte Überschriftennummern, Dateidatum und ein
früherer Git-HEAD reichen nicht zur Identifikation. §1.2 regelt parallele Edits.

### 1.1 Fundstelle ist nicht Änderungsziel: abgeschlossene Abschnitte schützen

**Erneuter Abschnittsabgleich — 2026-10-29:** Die fünf Definitionsnachfolger sind
bereits als Dokumentlieferung umgesetzt (§12). Ihre Überschrift „Open“/„Offene“
bezeichnet die dort beschriebenen zukünftigen Consumer-/Runtimeaufgaben, nicht
eine noch ausstehende Erstellung des Definitionsblocks. Weder diese Lieferung
erneut durchführen noch den ganzen Block als unbearbeitet behandeln. Nur eine
nach aktuellem Owner-/Statusabgleich tatsächlich noch unbearbeitete Einzelaufgabe
darf neue Anweisungen erhalten. Ist auch diese inzwischen umgesetzt, bleibt sie
Lesereferenz; ein bestätigtes neues Delta gehört in einen getrennten, ausdrücklich
offenen Folgeabschnitt. Eine offene Oberphase oder K-Nummer allein genügt nicht.

Diese Regel gilt ausdrücklich für `simplified_v3.md`, `validator_v3.md`,
`monty.composition.md`, `monty.composition.implementation.md` und
`prefix_v3_upgrade.md`, einschließlich ihrer implementierten Teilabschnitte.

1. Vor jeder Änderung den **einzelnen Teilabschnitt** mit aktuellem Statusindex,
   zugehörigem Nachweis und tatsächlichem Consumer abgleichen. Eine teilweise
   implementierte Phase ist weder insgesamt abgeschlossen noch insgesamt frei
   editierbar. Ein offener Gesamtplan erlaubt keine Änderungen an fertigen Teilen.
2. Bereits umgesetzte Schritte, Abnahme-/Implementierungsjournale, konkrete
   qualifizierte Beispiele und historische Ausgangsbeschreibungen als schreibgeschützte
   Referenz behandeln. Keine nachträgliche Textkorrektur, Löschung, Verschiebung,
   neue Historieüberschrift oder Änderung ihrer damaligen Abschlussanforderungen.
3. Bei unklarem Status den vorhandenen Abschnitt ebenfalls unverändert lassen.
   Im passenden offenen Bereich zunächst eine Prüfaufgabe mit bisherigem Bezug
   und konkreter Unsicherheit eintragen. Erst nach Quellen-/Ownerabgleich eine
   neue normative Nachfolgeranweisung formulieren; Unklarheit beweist weder eine
   alte Abnahme noch einen Architekturfehler. Keine Fertigstellung aufgrund eines
   Datums oder einer Überschrift erraten.
4. Auch bei eindeutig noch unbearbeiteten Abschnitten bevorzugt einen klar
   bezeichneten offenen Unterabschnitt ergänzen. Neue Änderungen an Entwürfen
   dort ausarbeiten; die ursprüngliche Bezugspassage muss dazu nicht umgeschrieben werden.
5. Bestätigte neue Folgeanweisungen erklären lokal: „Offene Nachfolgeranweisung ab
   K4; gilt für die hier benannten zukünftigen Consumer/Änderungen. Die frühere
   Passage und ihre damalige Evidenz bleiben unverändert. Für das neue Delta
   gelten der benannte Owner und dessen neue oder exakt anwendbare Nachweise.“
   Damit wird keine pauschale neue Architekturautorität über die vier Guides gelegt.
6. Eine neue echte Implementierungslücke als neue Folgearbeit unter K4–K6 bzw.
   beim späteren bestehenden Owner erfassen. Keinen alten Schritt auf offen setzen,
   keine Wiederimplementierung verlangen und nicht behaupten, die frühere scoped
   Abnahme sei durch den neuen Entwurf falsch geworden. Laufende Tasks behalten ihre Auswahl.

**Prüfergebnis der bisherigen Planfassung:** Die Anweisungen zu Phase-3a-Einleitung,
Composition §§4/5/16 und Referenz-/Lehrbeispielen waren zu pauschal. Sie konnten
in-place Änderungen an bereits umgesetzten Teilen auslösen. Daher sind diese
Fundstellen künftig nur Quellen für offene Nachfolgeranweisungen. Bei Validator
und Prefix sind vorhandene Facts-/Preload-/Source-/Job-Prerequisites getrennt von
noch offenen Klassenrouten, Pure-Approval und Serving-Verbrauchern zu behandeln.

| Datei | Vorhandene Abschnitte nur lesen/erhalten | Neues zulässiges Änderungsziel |
| --- | --- | --- |
| `simplified_v3.md` | K0–K3; abgeschlossene Teilumfänge von Phasen 0a/3a und §§8.2/10.1; frühere Pin-/Trackerbefunde und deren Receipts. | Vorhandenen K4-Nachfolger „Offene Definitions- und Consumerpräzisierungen nach K3“ referenzieren. Nur nach aktuellem Einzelstatus noch unbearbeitete K4–K7-/Consumerfolgearbeit ergänzen; keinen gelieferten oder abgeschlossenen Teil neu anlegen oder erweitern. |
| `validator_v3.md` | Umgesetzte Source-/Definition-/Graph-/Subject-/Preload-Voraussetzungen, deren dated Follow-ups, vorhandene Q2-/Hostingbelege; keinen gesamten Step 1–12 pauschal für offen erklären. | Vorhandenen Nachfolger `Open definition and example successors from K4` im `Remaining implementation` verwenden. Neue Anweisungen nur für tatsächlich noch unbearbeitete Klassen-/Pure-/Review-/UI-Folgeaufgaben; alte §§7.2/7.10 und gelieferte Definitionen bleiben Lesereferenzen. |
| `monty.composition.md` | Bereits implementierter Loader, retained Auswahl, Aktivierung, Reply-/History-/MCP-Unterstützung und deren qualifizierte Teile/Belege, insbesondere vorhandene Fortschrittsabschnitte §§27–35. Alte Kriterien §§4/5/16 nicht rückwirkend umschreiben. | Vorhandenen Nachfolger `Open contract and reference successors from K4` im `Remaining implementation` verwenden. Nur neue, noch unbearbeitete Familien-/Daten-/Artefakt-/MCP-Folgeaufgaben ergänzen; keine Wiederholung der gelieferten Contractkorrektur. |
| `monty.composition.implementation.md` | Bereits verwendete oder qualifizierte Komponenten-/Preloadbeispiele und ihre ursprünglichen Beschreibungskontexte; keine Massenkonvertierung alter Bodies. Unklarer Status bedeutet ebenfalls erhalten. | Vorhandenen Nachfolger `Open definition and example successors from K4` im `Remaining implementation` verwenden. Neue konkrete Komponenten-/Coverage-Consumer nur in deren noch unbearbeiteten Folgeaufgaben spezifizieren; gelieferte Referenzdefinitionen und alte Beispiele nicht nachträglich umarbeiten. |
| `prefix_v3_upgrade.md` | Implementierte Source-/Export-/Storage-/Compiler-/Job-/Encoding-Voraussetzungen und ihre Phase-B/C-Journale; abgeschlossene Teile auch innerhalb der Remaining-Liste. | Vorhandenen Nachfolger `Open definition, teaching and consumer successors from K4` in `Remaining implementation sequence — full completion` verwenden. Neue Anweisungen nur für bestätigte noch unbearbeitete A1/C1/C2-/Serving-/Upgrade-Einzelaufgaben; fertige Teile und gelieferte Teachingdefinitionen erhalten. |

Die Bezeichnungen sind neue Planüberschriften, keine Runtimefelder oder neuen
Ausführungswege. Bei schon vorhandener gleichwertiger **offener** Nachfolgersektion
diese verwenden statt einen zweiten konkurrierenden Auftrag anzulegen. Neue
Abschnitte nur bei einem tatsächlich relevanten neuen Delta außerhalb des
Textbereichs abgeschlossener Abschnitte einfügen. Ein bloßer Verweis kann genügen;
nicht automatisch in jeder Datei einen parallelen vollständigen Vertrag anlegen.

Der offene Folgeabschnitt nennt den maßgeblichen aktuellen Guide und den genauen
alten Bezug. In aktuellen, nicht geschützten Agent-Routern/Teaching-Einstiegen
auf diese neue Anweisung verweisen, soweit sie den betroffenen Consumer betreffen.
Keinen abgeschlossenen Abschnitt zum Setzen eines Backlinks öffnen. So müssen
Agents nicht zwischen widersprechenden Kopien anhand des neuesten Datums wählen.
Jede bestätigte Nachfolgeranweisung benennt ausdrücklich, welche frühere Aussage
für welchen zukünftigen Consumer nicht mehr die Codingvorlage ist. Ihre Geltung
folgt dem owning Komponentenvertrag und dem bestätigten Delta, nicht dem Prinzip
„neuester Text gewinnt“. Ungeklärte Widersprüche nicht durch weitere Banner kaschieren.

In anderen Guides/Entwicklungshinweisen bleiben notwendige aktuelle Korrekturen
möglich, soweit sie keine geschützten Implementierungs-/Abnahmeabschnitte ersetzen.
Auch dort gelten unveränderliche genehmigte Komponentenrevisionen und alte
Receipts weiter. Die ursprüngliche Planerstellung änderte nur diese Datei; der
anschließende Dokumentabgleich ist in §12 abgegrenzt.

### 1.2 Gleichzeitige Bearbeitung: semantisch lokalisieren und klein integrieren

Für jedes spätere zusammenhängende D-Paket gilt der vorhandene gemeinsame Checkout;
kein zweiter Entwicklungsbaum, Schreibdienst oder neuer globaler Lockmechanismus:

1. Mit `rg` aktuelle Fundstelle, Überschriftenkette, Zitat und benachbarte Absätze
   lesen. Bei mehreren Treffern deren jeweiligen Consumer/Vertrag unterscheiden.
   Keine Auswahl nach erstem Treffer, alter Zeile oder allein einem Abschnittsnamen.
2. Aktuelle scoped Änderungen und vorhandene technische Owner abgleichen. Wenn
   ein anderer Agent an derselben Passage arbeitet, die kurze Integration dieses
   Abschnitts über die bereits autorisierte Koordination abstimmen. Kein fremdes
   Paket unterbrechen, verwerfen, automatisch stagen oder zurücksetzen. Ohne
   geklärten gemeinsamen Schreibumfang an unabhängigen Aufgaben weiterarbeiten.
3. Den gerade gelesenen Datei-/Abschnittsinhalt samt Hash als zeitlich begrenztes
   Preimage erfassen. Unmittelbar vor der Anwendung Datei und Zielkontext erneut
   lesen. Bei Drift Plan/Delta gegen die neue Fassung neu prüfen, auch wenn Git-HEAD
   gleich blieb. Wurde die Sache bereits korrekt gelöst, referenzieren statt duplizieren.
4. Nur einen kleinen kontextgebundenen Patch gegen die aktuelle Fassung anwenden.
   Keine ganze Datei aus einem alten Snapshot, breiten `replace`-Lauf, festen
   Zeichenoffset oder früher gespeicherten kompletten String zurückschreiben.
   Fehlender/mehrdeutiger Kontext bedeutet neu lesen, nicht fuzzily anderswo anwenden.
5. Ein Preimagevergleich allein ist keine atomare Dateitransaktion. Während einer
   vereinbarten Integrationsphase fremde Änderungen erhalten; danach die neue
   Passage, ihr Umfeld und betroffene geschützte Abschnitte erneut prüfen. Ein
   nicht überlappender Merge braucht dennoch semantischen Review der Cross-Refs.
6. Bei mehreren Dateien gibt es keinen angenommenen atomaren Filesystem-Commit.
   Offene Paketänderungen als in Arbeit ausweisen, erforderliche Gegenstellen
   vollständig integrieren und erst dann eine konsistente Vertragsfassung melden.
   Code-/Release-Akzeptanz erfolgt nicht zwischen zwei nur halb angeglichenen Dateien.
7. Vor Checks die relevanten tatsächlichen Inputs festhalten. Änderungen nach
   einem Check nur auf dessen echte Inputclosure abgleichen: relevante Drift
   verlangt erneute Bewertung/gezielte Prüfung, rein verschobene Zeilen oder
   irrelevante Edits nicht automatisch eine neue Gesamtsuite. Paketchecks können
   volle Dokumentbytes umfassen; deren Anwendbarkeit nicht auf Abschnittshashes reduzieren.
8. Vor Abschluss nochmals scoped Diff, Links/Anker, geschützte Texte und neue
   Deltas prüfen. Zeilenverschiebung durch eine erlaubte neue Passage ist keine
   Änderung des alten logischen Abschnittsinhalts. Keine feste Länge, Zahl von
   Retries oder Zeitspanne zum vermeintlichen „Sicherstellen“ konfliktfreier Edits.

Preimages dienen dem lokalen Vergleich, nicht als neue dauerhafte Registry oder
Restaurierungsquelle. Der Nachweis darf die frühere Sourceidentität enthalten;
laufende aktive Quellen bleiben die Integrationsbasis. Unbekannte Drift wird
sichtbar geklärt, nicht durch Überschreiben oder ein bloßes „Dokumente fertig“ verdeckt.

### 1.3 Laufende Implementierungen nicht durch den Abgleich blockieren

Die Zuordnung dieses Plans ist eine Delta-Zuordnung zu bestehenden Ownern, keine
Neuplanung ihrer laufenden Arbeit. Vor Umsetzung den dann aktuellen Arbeitsstand
ermitteln; heutige Chatnamen, Zeilennummern und Statusannahmen sind keine dauerhaften
Zuständigkeits- oder Sperrregeln.

1. **Kein globaler Freeze oder neuer Vorgänger.** Nicht alle D-Pakete, alle sieben
   Pläne oder das frühere 67-Dateien-Inventar zur Voraussetzung für unabhängige
   Implementierungen, Tests oder deren begrenzte Abnahme machen. Nur eine konkret
   nachgewiesene gemeinsame Vertrags-/Inputabhängigkeit begründet eine lokale
   Integrationsabhängigkeit. Während ihrer Klärung unabhängige Arbeit fortsetzen.
2. **Ein Owner, ein Delta.** Zuerst prüfen, ob ein laufendes Paket denselben Caller,
   Vertrag, Test oder Nachfolger bereits bearbeitet. Dort nach Abstimmung ergänzen
   oder dessen Ergebnis referenzieren; keinen zweiten Fix, Parser, Testpfad oder
   parallelen Dokumentauftrag beginnen. Ein Hinweis an einen Chat erteilt ihm
   keinen zusätzlichen Auftrag und verschiebt nicht seine Abnahmebedingungen.
3. **K4 ist die Untergrenze, kein dauerhaft offenes Editziel.** Falls seit diesem
   Plan weitere K-Schritte oder Unterabschnitte abgeschlossen wurden, auch diese
   unverändert lassen. Die Tabellen in §§1.1/4.2 und der Textvorschlag in §4.3 gelten
   nur für dann noch offene passende Ziele. Neue Deltas beim bestehenden Owner im
   nächsten passenden offenen Folgeabschnitt ab K4 erfassen; kein fertiges Gate
   wieder öffnen. Fehlt ein solches Ziel, zunächst eine explizite Folgezuordnung
   klären; nicht still K7 erweitern, K8 erfinden oder abgeschlossene Texte ändern.
4. **Paketierte Bytes bewusst integrieren.** Vor einem Dokumentedit tatsächliche
   Einbettung, Releaseinventar, Seedgenerator und Integritätsconsumer prüfen.
   Planungstext nicht vorsorglich in Runtimequellen/Seeds/Releaseinventare aufnehmen.
   Änderungen an tatsächlich eingebetteten Guides oder generierten Komponenten
   mit dem zuständigen Writer im nächsten geeigneten bestehenden Paket integrieren.
   Keine unangekündigten solchen Edits während einer vereinbarten Prüfung ihrer
   Inputs; daraus entsteht keine Sperre für unabhängige Dateien oder Arbeitspakete.
5. **Builds und Evidenz gezielt behandeln.** Laufende Builds/Tests nicht abbrechen,
   Cargo-Caches nicht bereinigen und keine neuen Buildbäume oder Locks einführen.
   Die Entwicklungspolicy bestimmt gemeinsamen Target und serielle Cargo-Nutzung.
   Bei relevanter Drift Ergebnis und tatsächliche Inputidentität erhalten und die
   fehlende neue Aussage gezielt prüfen. Unveränderte Teilnachweise bleiben nutzbar;
   ein anderer Dokumenthash allein entwertet keinen fremden gesamten Arbeitsstand.
   Ein altes Binary belegt allerdings nicht die Übernahme neuer eingebetteter Bytes.
6. **Nur eigene Änderungen verantworten.** Für den Abschnittsschutz den unmittelbar
   vor dem eigenen Patch gelesenen Stand und den eigenen Patch vergleichen; nicht
   den gesamten gemeinsamen Diff als eigene Arbeit behandeln. Fremde Drift nicht
   zurücksetzen, um Schutz oder einen sauberen Diff vorzutäuschen. Überschneidungen
   mit Kontext und Owner klären; bis dahin keine Aussage über deren Gesamtabschluss.

Ein kohärentes Paket bedeutet die kleinste sachlich zusammengehörige Änderung mit
ihren nötigen Gegenstellen. Es bedeutet weder alle D01–D12 gemeinsam noch sämtliche
aktiven Agentenpakete. Ein bestätigter Fehler erhält seine konkrete Abhängigkeit
und bestehenden Owner; bloße Unsicherheit erzeugt keine allgemeine Warteschlange.

## 2. Ergebnis der erneuten Logikprüfung

Die zwölf Vorschläge werden mit folgenden Einschränkungen übernommen:

| Vorschlag | Präzisiertes Urteil und Grenze |
| --- | --- |
| 01 Historie/Iststand | Veraltete Gegenwartsbehauptungen korrigieren; historische Messungen und Receipts erhalten. Kein erneutes Monty-Upgrade wegen alter Planpassagen. |
| 02 `result` | Eine globale Zuweisung ist kein allgemeiner Preload-Vertrag. Nicht jede vorhandene Modulzuweisung ist falsch: zulässige historische/Standalone-Artefakte behalten ihren eigenen Vertrag. |
| 03 Pure-Beispiele | Wiederverwendbare Exportdefinitionen vollständig ausarbeiten. Nicht sämtliche alten Testprogramme automatisch konvertieren oder fehlende Pure-Approval durch Beispiele ersetzen. |
| 04 Read/Filter-Abdeckung | Im zuvor geprüften Entwurf fehlte Coverage an der Formatterkante. Vor Umsetzung aktuellen Graph prüfen; kein Beweis für einen heute aktiven Fehler oder jede angeblich falsche Ganzdateiaussage. |
| 05 Persistierte Felder | `channel` kann erklärende Sprache sein. Problematisch ist seine Verwendung wie ein speicherbares Feld. Keine zulässigen `step_id`-Referenzen innerhalb eines bestehenden Inputlayouts verbieten. |
| 06 Intents | Öffentliche Matching-Beispiele und private typisierte Eintrittstests trennen. Bestehende gemeinsame Parser-/Variantenvoraussetzungen nicht durch eine Dokumentausnahme umgehen. |
| 07 Bootstrap | `source: system` genügt nicht. Vorhandene Seeding-Funktionen nicht pauschal löschen; Provenienz, Review und Upgrade in deren bestehendem Owner ergänzen. |
| 08 Leases/Profile | Lokale Legacy-Anweisungen korrigieren. Worker-Claims, technische Isolation, Daten-IDs und externe Auth bleiben wirksam. Keine breite Löschung anhand von Schlüsselwörtern. |
| 09 MCP/Daten | Ausführbaren Quelltexttransport verbieten, erlaubte Quelldaten nicht. Advertisement und spätere Taskauswahl trennen; ausdrücklich fehlerhafte Deklarationen bleiben Fehler. |
| 10 Validatorbeispiel | Ein ausdrücklich illustratives Beispiel ist keine erfundene aktive API. Trotzdem durch eine vorhandene, exakte Referenz ergänzen/ersetzen, damit Agents nicht unnötig einen zweiten Parser bauen. |
| 11 Sempai | Zwei Workflow-Verträge präzisieren. Eine fehlende lokale Erklärung beweist keinen aktuellen Rekursionsfehler; vorhandene Gateway-/Reviewgrenzen zuerst verfolgen. |
| 12 Zwei Verbraucher | Die Einschränkung betrifft behauptete gemeinsame Abstraktionen. Eine konkrete notwendige Primitive oder technische Schnittstelle mit zunächst einem Consumer bleibt zulässig. |

Zusätzlich verbindlich: Diese Korrekturen erzeugen weder eigenständige Aktivierung
ungenutzter Bibliothekskomponenten noch eine neue Registry, Approval-API,
Ausführungs-VM, generische Workflow-Primitive oder automatische Wiederholung.

### 2.1 Auditbefunde und behobene Planfehler

| Risiko im bisherigen Plan | Korrektur dieser Fassung |
| --- | --- |
| „Alte Abschnitte erhalten“ und zugleich direkte Ersetzungsaufträge. | Fundstelle/Editziel trennen; neue Deltas nur in offenen Nachfolgern. Alte Texte bleiben, aktuelle nicht geschützte Router verweisen eindeutig auf den zuständigen Vertrag. |
| Anhängen ungeprüfter Nachfolger bei unklarem Status. | Erst konkrete Prüfaufgabe, dann bestätigtes Delta; bereits gelieferte Lösung wiederverwenden. |
| Parallele Edits nur durch „erneut lesen“ abgesichert. | Semantische Identifikation, frisches Preimage, kleine Kontextpatches, abgestimmter Schreibumfang und Nachkontrolle gemäß §1.2. Kein behauptetes Datei-CAS. |
| Unbedingte Abschlusscheckliste verlangt später noch nicht unterstützte Pure-/Referenceausführung. | Drei Ergebnisarten in §11.2: Textlieferung, aktuell betroffene ausführbare Pfade und explizit zukünftige nicht aktivierte Entwürfe. Kein verstecktes Vertagen eines aktiven Fehlers. |
| Gemeinsames Beispiel kann zweite Implementierung/Fixtureframework werden. | Vorhandene Factorys und Caller wiederverwenden; nur notwendige konkrete Nachfolger, keine zweite Schema-/Decoder-/Approval-/Runnerimplementierung. |
| „Keine neue Beispiel-UUID“ verhindert legitime neue Komponenten. | Keine erfundenen aktiven Identitäten; benötigte neue stabile Identitäten über den bestehenden Authoringpfad, Fixtures eindeutig getrennt. |
| Erhalt alter Receipts wird mit pauschaler Nicht-Neubewertung verwechselt. | Originalbeleg unverändert halten, seine Anwendbarkeit auf neue Bytes separat prüfen; keine automatische alte Acceptance für einen neuen Contract. |
| Coverage kann aus `truncated_by_default=false` fälschlich Vollständigkeit ableiten. | Auswahlparameter und konkrete Readersemantik berücksichtigen; fehlende Belege bleiben unbekannt. Raw-Provenienz und Vorschauabdeckung getrennt. |
| Beispiellimits und Inventarzahlen werden zu neuen Produktgrenzen. | Herkunft/Art aller Zahlen in §11.3 klären; keine neue Quota und keine kopierte Defaultquelle. |

Dieser Audit behauptet keinen neuen Runtimebug allein aus einer Textstelle.
Nachgewiesene neue aktive Defekte erhalten ihre eigene Folgearbeit und Acceptance;
bereits vorhandene richtige Implementierungen werden nicht erneut gebaut.
Alle Aussagen über aktuell vorhandene oder noch fehlende Unterstützung sind
Bestandsannahmen dieses gelesenen Stands. Vor ihrer Verwendung erneut prüfen;
zwischenzeitlich gelieferte Pure-/Review-/Consumerunterstützung wird mit ihrem
echten Scope übernommen, nicht wegen älterer Formulierungen künstlich blockiert.

## 3. Gemeinsamer Sollvertrag für alle Änderungen

### 3.1 Komponenten und Ownership

| Einheit | Vertrag | Gehört ausdrücklich nicht dazu |
| --- | --- | --- |
| Tool, Klasse 0 | Registrierte Rust-Primitive mit realer Implementierung, Callable/Adapter, typisiertem Operationsvertrag und retained Artefaktidentität. | Auswahl einer mehrstufigen fachlichen Aufgabe oder eigener Agentenloop. |
| ToolSkill, Klasse 13 | Nicht ausführender IBS-Bindingdeskriptor für eine Tool-Nutzung. | Toolaufruf, Python-Nutzungscode oder Berechtigungsgrant. |
| Skill, Klassen 1–3 | Eine wiederverwendbare Tool-Nutzung: Prosa plus ausdrücklich assoziierter vorladbarer PythonCode, Exporte, Schemas und Abhängigkeiten. | Pflicht zu eigenem Recipe/MCP-Kommando; Domain-/Scaffold-Hierarchie; versteckte mehrtoolige Aufgabe. |
| PythonCode, Klasse 22 | Implementiert Skill-Export oder reine Logik mit expliziten Verträgen und qualifiziertem Dependency-Graph. | Automatische Approval durch Syntax/Parsing oder automatische Toolfreigabe. |
| Recipe, Klasse 21 | Fachliche Reihenfolge, typisierte Datenkanten, Varianten, unterstützter Kontrollfluss, Fehler und Abschluss. | Zweiter Rust-Workflowloop; neue Felder allein aus einer Diagrammdarstellung. |
| ExtensionCatalogue, Klasse 23 | Domain-/Taskgruppenbeschreibung und Recipe-Inventar. | Mehrtooliger Skill oder implizit ausführbarer Code. |

Skill- und PythonCode-Teile können getrennte unveränderliche Revisionen besitzen.
Ihre genaue Kombination braucht ihren gültigen Approval-Nachweis. Ein Codebeispiel
in Prosa ersetzt keine Assoziation. ToolSkill-Binding ist Vorbereitung; erst die
ausgewählte Exportinvocation kann `host.<tool>(...)` erreichen. Aktuelle globale
Kernelpolicy und technische Regeln gelten vor jedem tatsächlichen Dispatch.

### 3.2 Auswahl, Ausführung und Freigabe

- Ein kohärenter genehmigter Katalog bestimmt die vollständige Taskauswahl:
  Recipe/Variante/Schritte/Layout, Exporte, Abhängigkeiten, Association-/Approval-
  Referenzen und tatsächliche native Artefakte. Keine spätere Latest-Auswahl.
- Ein UUID-Verweis je Komponentenschritt. Unabhängige Toolaufrufe bleiben getrennte
  Ausführungen. Die dokumentierte Kettenausnahme ist keine Freischaltung des
  aktuell engeren retained Runners; dessen tatsächliche Grenzen gelten weiter.
- Reine Logik braucht keine künstliche Skill-/ToolSkill-Hülle. Qualification einer
  Helper-Abhängigkeit ist keine unabhängige Pure-Step-Approval.
- Nur echtes No-Match führt in Tier 2. Fehler, Mehrdeutigkeit und begonnene
  Recipe-Ausführung werden nicht dort wiederholt. Shell/Subagent bleiben Tier 1.
- Laden hat keine Nutzungseffekte. Mutable Daten bleiben invocation-/taskbezogen.
  Source, Daten und Invocation werden nicht durch Stringsubstitution vermischt.
- Komponentenapproval, Workflowqualification, aktuelle Policy und MCP-Veröffentlichung
  sind verschiedene Tatsachen. Ein vollständiger Skill braucht kein eigenes Recipe.
  Aktuelles Deployment veröffentlicht dennoch vollständige konsumierende Recipe-
  Graphen; daraus keine standalone Bibliotheksaktivierung ableiten.
- Vorhandene Retry-/Recoverygrenzen bleiben erhalten, insbesondere aktuelle
  Stop-only-Profile. Timeout ist kein No-Effect-Beweis. Bestätigte Effekte werden
  nicht wegen Output-, Reply- oder Evidenzfehler wiederholt.

### 3.3 Prefix, Provider und Sempai

Provider-/Modellidentität, Credentials und manuelle Limits bleiben bei bestehenden
DB-/Publikationsownern. Pair-Design, Rolleninhalt und fertige Prefixgeneration sind
getrennt. Kohai formatiert den dynamischen Prompt, optionaler Sempai liefert dessen
finale Fassung zurück, Kohai fügt das vorgesehene fertige Prefix unverändert genau
einmal davor. Der tatsächliche vollständige Request wird am letzten Sendeboundary
geprüft, auch bei abgeschalteten optionalen Tokenbudgets. Keine neue Providerliste,
Servertemplate-Installation oder zweite Rollen-Limitsquelle aus diesem Plan.

## 4. Umsetzung ausschließlich ab K4; Entscheidung gegen ein K8

### 4.1 Warum kein K8 eingefügt werden soll

Die bisherigen Befunde begründen keine neue unabhängige Ownershipschicht.
Ein bestätigtes Consumerdelta kann einen neuen präzisen Datenvertrag benötigen,
bleibt aber beim vorhandenen technischen/fachlichen Owner.
K4 besitzt bereits Auswahl/Preload/Daten/Child/MCP, K5 Dokumentpakete und begrenzte
Validatorgrenzen, K6 Retention/Upgrade, K7 das gemeinsame Abschlussgate. Ein K8
„Architektur/Dokumente abgleichen“ würde deren Arbeit duplizieren und die bislang
abschließende Bedeutung von K7 unklar machen.

Die D01–D12 unten sind Arbeitspaket-IDs dieses Plans, keine neuen K-Gates oder
zweite Statusablage. Ihre Resultate in bestehende Owner-Records übernehmen. Falls
die Umsetzung später einen tatsächlich neuen, von K4–K6 nicht erfassten aktiven
Vertrag nachweist, diesen konkret begründen und den K-Plan gesondert aktualisieren;
aus dieser Möglichkeit wird jetzt weder K8 noch ein genereller Abschlusszwang.

### 4.2 K0–K3 erhalten; ausschließlich K4–K7 ergänzen

Alle Ergänzungen unterliegen §1.1. Frühere Befunde sind Lesereferenzen;
K4–K7 erfassen neue Folgearbeit, keine rückwirkende Änderung fertiger Planabschnitte.
Die Tabelle ordnet Zuständigkeiten zu; sie erklärt weder ein ganzes K-Gate noch
bereits gelieferte Teilaufgaben erneut für offen.

| Schritt | Änderung am bestehenden Plan | Nicht ändern |
| --- | --- | --- |
| K0–K3 | Keine Änderung, Ergänzung oder neue Abnahmeanforderung. Abgeschlossene Ergebnisse ausschließlich als vorhandene Ausgangsbasis referenzieren. | Schritte, Abschlussstatus, Originalidentitäten und damalige Nachweise bleiben unverändert. |
| K4 | Aktuelle Source-/Consumer-Ausgangsbasis und D01–D12-Disposition erfassen. D02/D04/D05/D06/D09/D10/D11 relevante aktuelle Consumer zuordnen: Source-vs-Invocation, Ergebnisabdeckung, echtes Schema, private Eintrittsarten, MCP-Daten, getrennte Advertising-/Taskidentität und Reviewzwecke. Änderungen an bereits früher qualifizierten Consumerverträgen und finale integrierte Binaryinputs hier abgleichen, nicht rückwirkend K0–K3 zuordnen. Vorhandene passende Evidenz zuerst nutzen. | Loader, Child-/MCP-Transport und globale VM nicht neu bauen; keine neuen Retry-/Occurrenceprofile freigeben. |
| K5 | D01–D12 als jeweils relevante Dokument-/Lehr-/Seed-Contractprüfung aufnehmen, einschließlich D04 Coverage und D09 Daten/MCP. Konkrete geänderte Paketbytes und nicht paketierte Entwicklungshinweise trennen. Aktualisierte Definitionen gegen vorhandene begrenzte Validatorregeln prüfen. | Bereits qualifizierte private Schritte nicht wegen allgemeiner offener Klassenprüfung verwerfen; keine komplette Pure-Approval oder produktive Klassenrouten in K5 ziehen. |
| K6 | Geänderte Seed-/Guidebytes mit Nachfolgerrevision, Overrides, alten Tasks und vorhandenen Artefakt-/Restoregrenzen behandeln. Bei fehlender sicherer Installation/Upgradeunterstützung genaue Blockade ausweisen. | Keine alten Approvals/Revisionen/Applied-Migrationen überschreiben; keinen universellen ABI-Loader voraussetzen. |
| K7 | Aktuelle D-Befunde und später delegierte Entwürfe unterscheiden. Für aktive Pfade gelten echte Belege oder sichere unterstützungsbezogene Blockaden. Zukunftsentwürfe brauchen eindeutige offene Owner/Gates, keine vorgezogene Aktivierung zur Dokumentabnahme. | K7 bleibt Abschlussgate; unveränderte funktionierende Pfade nicht abschalten, um es rechnerisch zu erfüllen. |

### 4.3 Historischer Ergänzungsvorschlag im K-Vorrangblock — bereits geliefert

Die folgende Vorlage dokumentiert den bereits durch §12 erledigten
Definitionsauftrag. Sie ist kein erneuter Einfügeauftrag an K4. Den vorhandenen
Definitionsnachfolger referenzieren; weder ihn doppelt anlegen noch nachträglich
neue Abnahmeforderungen in bereits gelieferte Teile einsetzen. Für ein tatsächlich
neues, noch unbearbeitetes Consumerdelta gelten §1.1/§1.3 und der dann offene
Folgeabschnitt beim bestehenden Owner. K0–K3 und Abschlussaufzeichnungen bleiben
unverändert:

> K0–K3 sind abgeschlossen und bleiben unverändert. Der Definitionsabgleich
> `Exact_V3_definition_update_at_K3.md` wird erst ab K4 als begrenzte
> Contract-/Dokumentkorrektur durchgeführt.
> Seine vorhandenen Runtimeconsumer gehören K4, Paket-/Lehr-/Reviewinputs K5 und
> Nachfolger-/Retentionsgrenzen K6. K7 schließt diese Ergebnisse mit ein.
> Er führt kein K8 ein und öffnet keinen abgeschlossenen Schritt erneut.
> Die vollständige Umsetzung späterer Pure-, Validator-, Bibliotheks- oder
> Prefixentwürfe bleibt bei deren bereits benannten offenen Ownern und Gates.
> Neue integrierte Nachweise müssen die tatsächlich geänderten Paketbytes erfassen.

K0–K3-Ergebnisse nicht rückwirkend umschreiben oder neu prüfen lassen. Berührt eine
neue Änderung dort zuvor qualifizierte Mechanik, erhält allein die konkrete neue
Änderung ihren Nachweis in K4–K6 beim vorhandenen technischen Owner. Das eröffnet
kein altes K-Gate erneut. Mit aktuell arbeitenden Chats finale Inputs vor dem
nächsten gemeinsamen Build abgleichen; passende Evidenz wiederverwenden.
Unveröffentlichte Prosa allein erzwingt keinen neuen laufenden Providercall.

## 5. Durchführung in begrenzten Paketen

1. Vor Änderungen aktuelle Owner/Hashes/Abschnitte und vorhandene Evidenz erfassen.
   Den Appendix dieses Plans nicht als unveränderliche Beschreibung fremder Edits
   behandeln. Bereits erledigte Punkte mit Fundstelle als erledigt abgleichen und
   deren Abschnittsinhalt unverändert erhalten. §1.1-Matrix vor jedem Edit anwenden.
2. Zuerst D01/D02/D05/D07/D08/D12 im bestehenden owning Guide/Router präzisieren,
   soweit nicht geschützt. Nur wo erforderlich zugehörige offene Folgeanweisungen
   in den Plänen ergänzen; keinen vollständigen neuen Vertrag pro Datei kopieren.
   Das verhindert neue Fehlimplementierungen ohne Runtimeumbau.
3. D03/D04/D06/D09/D10/D11 nach tatsächlicher Callerabhängigkeit in begrenzte
   Vertrags-/Beispielpakete aufteilen; sie bilden keine gemeinsame Vorbedingung.
   Vorhandene Callerabhängigkeiten bestimmen die genaue Reihenfolge. D03 und D10
   beginnen mit Klassifizierung/Vertrag, nicht mit Freischalten allgemeiner Pure-Approval.
4. Spiegel, konkrete Verbraucher und offene zukünftige Planpassagen im selben Paket
   angleichen. Geschützte alte Passagen nur aus den neuen Folgeabschnitten referenzieren.
   Anschließend D03/D10 über eine gemeinsame Referenz nach §7 verbinden.
5. Geänderte paketierte Quellen über K5/K6 behandeln, finale integrierte Inputs
   mit den ab K4 zuständigen Runtime-/Prefix-/Validatorownern abgleichen und
   K7-Disposition dokumentieren.

## 6. Detaillierte Änderungen D01–D12

**Leseregel für alle D-Pakete:** Die in §12 dokumentierte Lieferung nicht erneut
ausführen. Nachfolgende Handlungssätze gelten nur für noch unbearbeitete spätere
Consumerdeltas, soweit §1.1/§1.3 dies nach aktuellem Abgleich erlauben.
Die jeweils genannten Fundstellen erklären den
Befund. In den fünf Plänen ist das Editziel ausschließlich der zugehörige offene
Folgeabschnitt aus §1.1. „Ersetzen/löschen/historisch kennzeichnen“ beschreibt dort
die Behandlung für zukünftige Anweisungen; es gestattet keinen Eingriff in die
bereits umgesetzte Bezugspassage. Neue Code-/Komponentennachfolger benötigen ihre
eigene spätere Folgearbeit, nicht eine rückwirkende Änderung des alten Planschritts.

### D01 — Historie und tatsächlichen Stand lokal unterscheiden

**Fundstellen:** `simplified_v3.md`, Überschrift „Phase 3a — Globalen Monty beim
Boot starten und dauerhaft betreiben“, Text
„Engine und Komposition verwenden derzeit den Git-Tag v0.0.16“/„noch nicht umgesetzt“;
API-Lückenabschnitt zu `LimitedTracker`; ältere Gegenwartsabsätze in Composition
§§4/15/26; lokale Crate-Inventare mit veralteten Runtime-/Profilangaben.

**Schritte:**

1. Tatsächliche Pins in `Cargo.toml`, Engine-/Monty-Host-Manifests, vendored
   Vertrag und aktuellem Supportindex prüfen. Eine alte Planaussage ist kein
   Auftrag für `cargo update` oder ein erneutes Interpreterupgrade.
2. Im neuen offenen K4-Folgeabschnitt den aktuellen Ausgangspunkt formulieren:
   vorhandene lokale Monty-1.0-Integration wiederverwenden; verbleibende Konten-/
   Recovery-/Consumer-Gates einzeln benennen. Die frühere Phase-3a-Einleitung bleibt stehen.
3. Im neuen Folgeabschnitt den alten Befund vom 06.10. und die Trackeranalyse als
   historische Referenzen einordnen und verlinken. Die Originalpassagen weder
   verschieben noch mit neuer Historieüberschrift oder Inlinehinweisen verändern.
   Historische Versions-/MSRV-Werte werden im neuen Auftrag nicht zu neuen Defaults.
4. In Composition „absenter Adapter“/„aktueller alter Composer“-Passagen mit den
   inzwischen vorhandenen §§33–35-/Ownerbelegen abgleichen. Abgeschlossene Arbeit
   im neuen offenen Folgeabschnitt als historischen Auftrag referenzieren;
   ursprünglichen Auftrag nicht bearbeiten, neue echte Restaufgaben getrennt benennen.
5. Alte Code-/Pfadnamen nur als historische Referenz erhalten, wenn sie tatsächlich
   nicht mehr existieren. Aktuelle Codinganweisungen nennen reale Owner/Caller.

**Für neue Anweisungen ausschließen:** Gegenwartsbehauptungen zu überholtem Pin,
fehlendem Singleton oder fehlendem bereits implementiertem Loader. Alte Passagen,
historische Resultate und deren technische Grenzen nicht löschen oder umschreiben.

**Impact/Abnahme:** Dokument-/Link-/Iststandprüfung in K5; keine Runtimeänderung nur
wegen dieser Korrektur. Spätere Phase 3a beginnt bei ihren tatsächlichen offenen
Gates. Ein Agent darf aus der Anleitung kein zweites Upgradeprojekt ableiten.

### D02 — Definition, Invocation, Rückgabewert und Step-Body trennen

**Fundstellen:** `monty.composition.md` §4 „requires `result` assignment“, §5
„BrassClaw also requires assigned `result`“; Komponenten-/Lehrvorlagen und
`retained_source`-Verweise; `validator_v3.md` §§5a/7.2.

1. Am aktuellen `retained_source.rs`, `retained_preload.rs` und Runner prüfen,
   welche Artefaktart welcher Prüfung unterliegt. Die nicht-preloadende
   `result_store_sites`-Prüfung nicht auf alle Exportdefinitionen verallgemeinern.
2. Im neuen offenen Composition-Folgeabschnitt anstelle der beiden alten pauschalen
   Sätze die vier Fälle aus §2 dieses Plans als zukünftige Anweisung formulieren:
   Definition ohne Effekt; Export mit `return`; compilergebundene Invocation mit
   Ergebniserfassung; ausdrücklich historischer/Standalone-Step-Body.
3. In jedem aktuellen Beispiel die Definition und den späteren Aufruf visuell
   trennen, soweit dieses Beispiel selbst ein offenes Editziel ist; sonst nur den
   neuen Nachfolger korrekt darstellen. Eine Modulzuweisung nicht hinzufügen, um einen
   unpassenden Validator zufriedenzustellen.
4. Gespeicherte Source-/Export-/Invocationverträge unverändert streng lassen:
   kein dynamisches `eval`, kein Modell-Exportname, keine Datenquellsubstitution.
5. Vorhandene Loader-/Rückgabe-/Zero-effect-Fälle wiederverwenden. Nur bei
   tatsächlicher Reader-/Runneränderung passende reale Regression ergänzen.

**Nur in offenen Anweisungen löschen/ersetzen:** allgemeine Pflicht zur Modulzuweisung.
**Unverändert erhalten:** die alten Bezugspassagen und andersartige Step-Body-Verträge;
ihre historische Geltung wird im neuen offenen Folgeabschnitt erklärt.
K4 prüft tatsächliche Consumers; K5 korrigiert Lehrinputs. Keine neue Runnerart.

### D03 — Pure-Beispiele nach tatsächlicher Artefaktart überarbeiten

**Fundstellen:** `monty.composition.implementation.md`, vorgeschlagene Pure-Teile
ab „Proposed reusable parts“, insbesondere Filter/Formatter; `scripts/prefix/
sempai-worked-examples.md` Decision map und vollständige Programme;
`sempai-authoring-reference.md`, Compatibility-Hinweise; Validator-Purevertrag.

1. Alle betroffenen Codeblöcke vollständig inventarisieren und als Definition,
   Invocation, Standalone-Test, historischer Body oder nicht ausführbarer Entwurf
   klassifizieren. Gleiche Klassifizierung auf JSON-`content`-Spiegel anwenden.
2. Bei für neue wiederverwendbare Ausführung vorgesehenen Pure-Komponenten eine
   effektfreie Funktion mit `return`, endlichen rekursiven Ein-/Ausgabeverträgen,
   öffentlichem Export und privatem Dependency-Graph ausarbeiten. Keine leere
   ToolSkill-/Skill-Hülle erzeugen.
3. Passenden separaten Aufruf und tatsächliche retained Export-/Layoutzuordnung
   zeigen. Fehlende allgemeine Pure-Approval ausdrücklich als offenen Gate belassen.
4. Die Aussage „helper ... assign ... module-level result“ im Decision map auf
   ihren Standalone-Vertrag begrenzen. Nicht als universelle Codeautorenregel lehren.
5. Standalone- und eingefrorene historische Beispiele mit ihren ursprünglichen
   erwarteten Ergebnissen erhalten; nur aktuelle Teachingquellen aktualisieren.
   Weder alte Probeantworten noch historische Erfolgsbelege umschreiben.
6. Je geänderter Semantik positive/boundary/negative Fälle prüfen, einschließlich
   Missing/Null, Bool-vs-Int, ungültiger Producer und mutable Isolation. Bereits
   passende Fälle wiederverwenden, keine Assertions nur zum Wortlaut ergänzen.

**Nur in offenen Entwürfen/aktuellen nicht abgeschlossenen Teachingquellen löschen:**
unbeschriftete Doppelbeispiele, die dieselbe aktuelle Komponente gleichzeitig als
inkompatiblen Step-Body und Preload-Export anweisen. Alte umgesetzte Planbeispiele
nicht umarbeiten; ein klar getrennter neuer Nachfolger zeigt den künftigen Vertrag.
**Nicht löschen:** nutzbare begrenzte Testartefakte oder referenzierte historische Samples.

**Zeitliche Grenze:** Definition/Label/Schemaabgleich K5. Vollständige produktive
unabhängige Pure-Ausführung bleibt Validator Steps 3/8–10 und V35-03/04; nicht
aktivieren, um K5 oder K7 als abgeschlossen melden zu können.

### D04 — Abdeckung durch Read → Filter → Antwort erhalten

**Fundstellen:** Composition §10.2 (`literal-lines`, Inputlayouts und sechs
Komponentenschritte), Implementation „proposed-line-reply-formatter“, R10-Abgrenzung;
Roadmap-Referenztests und tatsächliche zukünftige Verbraucher der Pure-Teile.

1. Tatsächliches `read_file`-Ergebnis prüfen: `content`, `path`, `lines_shown`,
   `total_lines`, `truncated_by_default` und dessen genaue Bedeutung. Keine neuen
   Backendfelder oder Raw-Provenienz aus diesen Anzeigemetadaten erfinden.
   Auch die tatsächlich gebundenen Offset-/Limit-/Repräsentationsparameter
   berücksichtigen. `truncated_by_default=false` besagt nicht allein, dass eine
   explizit begrenzte Auswahl die ganze Datei umfasst. Zählwerte/EOF und fehlende
   Metadaten nur gemäß belegter Readersemantik auswerten, keine neue Vollständigkeitsheuristik.
2. Filter kann weiterhin eine reine Liste liefern, sofern das Recipe zusätzlich
   die unveränderten Abdeckungsfelder des Read-Ergebnisses an den Formatter bindet.
   Alternativ einen explizit versionierten Resultvertrag verwenden; kein stiller
   Typwechsel einer freigegebenen Filterrevision. Kleinste passende Änderung wählen.
3. Für den Formatter einen wiederverwendbaren Vertrag aus Auswahl plus expliziter
   Coverage definieren. Pfad/Bereich/Anzahl/Truncation als Daten übergeben, nicht
   aus dem nummerierten Text zurückerraten. Kennzeichen nicht mit `complete:true`
   für bytegetreue Vollaufnahme gleichsetzen.
4. Antwort bei unvollständiger Vorschau eindeutig begrenzen, etwa „Keine Treffer
   in der gelesenen Vorschau; nicht die ganze Datei untersucht“. Treffer,
   Whitespace-Treffer und leere Datei unterscheiden. Vollständige Abwesenheit nur
   behaupten, wenn der konkrete Vertrag sie tatsächlich belegt.
5. Layout, Producer-/Consumer-Schemas, Formattercode, Prosa und Beispielantworten
   in einer später autorisierten konkreten Nachfolgerimplementierung gemeinsam
   aktualisieren; für das jetzige Planpaket deren vollständigen Vertrag beschreiben.
   Gleichen Formatter nur dort teilen, wo der Consumer
   tatsächlich dessen Coveragevertrag liefern kann (Tail/Preview können abweichen).
6. Für einen tatsächlich implementierten Nachfolger ganze Datei, gecappte Vorschau,
   explizites Teilintervall ohne Default-Truncation, Match außerhalb der Vorschau, EOF/leere Datei,
   Whitespace, Unicode und fehlerhaftes Read-Ergebnis im echten ausgewählten Pfad
   prüfen. Fehlender Match ist kein fehlgeschlagener Toolcall; Readfehler führt
   nicht zu „keine Treffer“, Replay oder Tier-2-Fallback.

**Offenen Nachfolger definieren:** Antwortvertrag und Datenkanten für unvollständige
Eingabe vollständig beschreiben. Das bestehende Beispiel/aktive Layout nicht als
Nebenwirkung dieses Plans ändern; vorhandenen Coverage-/Formatterconsumer zuerst wiederverwenden.
**Nicht ändern:** Read-Primitive in einen Workflow verwandeln; R10 durch Abstrippen
von Zeilennummern ersetzen; vorhandene genehmigte Resultverträge in place ändern.

**Aktuell/zukünftig:** Zunächst feststellen, ob ein real aktiver Consumer betroffen
ist. Dann eine neue K4-/fachliche Folgearbeit mit eigenem Nachweis vorsehen;
seinen alten Implementierungsabschnitt und damaligen Beleg nicht korrigieren.
Ein nur geplanter Referenzfall
bleibt D03/V35-05/06 und braucht keine vorgezogene Pure-Aktivierung für K7.

### D05 — Echte Recipe-Syntax und erklärende Diagramme unterscheiden

**Fundstellen:** Simplified §§1.1/9 (`channel:"rust"`/`channel:"orchestrator"`),
Root-/Crate-Guides, Validator §7.10, Composition-/SKR-/RCP-Templates.

1. Aktuelle `StepDescriptionEntry`-/Step-/Binding-/Layout-/Invocationparser lesen.
   Schritt-Felder und eingebettete Referenzfelder getrennt festhalten.
2. In offenen neuen Authoring-Anweisungen `knowledge`/`stepnumber` und eine UUID pro
   component-`include` verwenden. Neue nicht geschützte Diagramme als erklärend
   kennzeichnen; alte geschützte Diagramme nur im offenen Nachfolger einordnen.
3. Keine globale Ersetzung von `step_id` vornehmen: `step_id` in einem tatsächlich
   unterstützten Result-Inputref ist zulässig, obwohl es kein frei hinzufügbares
   Step-Feld ist. Dasselbe gilt für unterstützte retained Dokumentbereiche.
4. `input_layouts` und `invocations` gemäß realem retained Dokumentowner zeigen,
   nicht als unbewiesene Felder eines Legacy-SQL-Konstruktors. Fehlende Adapter
   durch den bestehenden Owner implementieren, nicht in Beispielen hineinbehaupten.
5. Ein vorhandenes parser-/fixturegeprüftes Beispiel für Bindingpaar, Pure-Schritt
   und Datenübergabe referenzieren; falsche Klasse, leeres/mehrfaches Include,
   fehlendes Binding und forward reference als gezielte Negativfälle aufnehmen.

**Löschen/ersetzen:** erfundene Felder aus als speicherbar angebotenen Beispielen.
**Historisch:** tatsächliche alte Schemata nur mit genauem damaligem Consumer.
K4/K5 prüfen Aktualität; fehlendes neues Schema bleibt bei Phase 0a/Composition.

### D06 — Öffentliche Intents von interner Admission trennen

**Fundstellen:** `recipe.md` allgemeine Zehn-Beispiele-Regel und Checkliste,
Root-Intent-Anweisungen, Prefix Phase A, Validator Step 6 und private Sempai-/Prefixpfade.

1. Vorhandene Einstiegstypen verfolgen: öffentliches Intent-Matching, vertrauenswürdige
   interne Recipe-Auswahl und MCP als normaler Chatclient. Einen Admissiontyp
   nicht allein aus Namen, fehlenden Beispielen oder Prosa ableiten.
2. Zehn-Beispiele-Regel ausdrücklich auf öffentlich gematchte Recipes begrenzen;
   jede öffentliche Variante weiter mit passenden positiven/negativen Fällen prüfen.
3. Für private Workflows stattdessen den echten typisierten Eintritt, erlaubte
   Herkunft, explizite Recipe-/Variantenwahl, Inputlayout und Abschlussart prüfen.
   Keine öffentliche Route, User-Nachricht oder Replypflicht zur Testmengenerfüllung erfinden.
4. Bereits benötigte interne Varianten-/Parserdaten erhalten. Wenn ein aktueller
   Parser trotzdem gemeinsame Felder verlangt, dessen zulässigen privaten Vertrag
   nachweisen oder im vorhandenen Owner ergänzen; Prosa kann ihn nicht wegdefinieren.
5. Für beabsichtigtes MCP zusätzlich Kommandogrammatik, Cross-Skill-Negativfälle,
   normalen Match und korrelierte Chatcompletion qualifizieren. Internes Recipe
   wird dadurch nicht automatisch veröffentlichungsfähig.
6. Regeln in Guides/Root-Anweisungen und den neuen offenen Prefix-/Validator-
   Folgeabschnitten gemeinsam angleichen. Frühere Phase-A-/Step-6/7/12-Passagen
   bleiben als Bezug erhalten, soweit umgesetzt oder im Status unklar.
   Tests für unerlaubten öffentlichen Zugriff auf private
   Pfade erhalten und nur bei konkreter Lücke erweitern.

**In neuen offenen Anweisungen ausschließen:** Pflicht zu künstlichen öffentlichen
Intents privater Workflows; keine Löschung bereits umgesetzter alter Planpassagen.
**Nicht löschen:** öffentliche Matchinganforderungen oder interne technische Guards.
K4/K5 liefern aktuellen Abgleich; zukünftige private Routen behalten ihre Owner.

### D07 — Bootstrap nicht auf Quellenlabel reduzieren

**Fundstelle:** `.claude/rules/skills.md`, „Authoring ... inserting ... or seeding
it as `source = "system"`“, sowie gleichartige Authoring-/Seedinghinweise.

1. Die verkürzte Alternative durch die beiden tatsächlichen Pfade ersetzen:
   installiert authored mit Q1/Behavior/human-Q2/exakter Combination; ausgeliefert
   mit nichtzirkulärer Installation-/Integritäts-/Q1-/Behaviorqualifikation.
2. Revision speichern, Assoziation erklären, Evidenz schreiben, Katalog publizieren
   und Tool dispatchen als unterschiedliche Operationen beschreiben. Ein Insert
   oder Label bewirkt keinen der anderen Schritte automatisch.
3. Reale Seeder/Upgrade-/Overrideowner prüfen. Unter `ON CONFLICT DO NOTHING`
   erreicht eine neue Rust-Konstante nicht automatisch bestehende Daten.
4. Für geänderte paketierte Prosa/Guides aktuelle und nächste exakte Revisionen,
   Artefakt-/Approvalreferenzen, Originale und Overrideverhalten über bestehende
   Versions-/Installationsmechanismen festlegen. Keine neue UUID allein wegen
   Prosaänderung, aber auch keine Mutation einer genehmigten alten Revision.
5. Bootstrap-Negativfälle Quellenlabel ohne Beleg, veränderte Bytes unter alter
   Identität, Overrides und wiederholten Boot anhand vorhandener Tests abgleichen.
   Fehlende sichere Upgradeunterstützung explizit blockieren, nicht umgehen.

**Löschen:** Label als hinreichende Authoring-/Freigabeanweisung.
**Erhalten:** legitime Rust-Seeder, alte genehmigte Bytes, Authored-/Seed-Unterscheidung.
K5/K6 sind die Abnahmeowner; allgemeine Validatorwriter bleiben zukünftige Arbeit.

### D08 — Lease-/Profilinventar nicht als v3-Handlungsregel lehren

**Fundstellen:** `crates/AGENTS.md` Tabelle zu Authorization/Approvals/Run-state/
Runtime-policy, insbesondere „dispatch before fingerprinted lease claim“;
betroffene Crate-Guides und historische Scope-/Runprofilbeschreibungen.

1. Jede Fundstelle als Istinventar, noch nötiger Legacy-Consumer oder aktuelle
   Zielanweisung klassifizieren. Vorhandene technische Sicherheitsgrenzen nicht
   aus einer Textsuche heraus entfernen.
2. Imperative neue v3-Toolgrant-/Fingerprint-Leaseanforderung aus aktuellem
   Coding-Routing entfernen. Wenn der Code noch existiert, direkt am Tabelleneintrag
   „Legacy/Compatibility; im v3-Cutover ablösen, nicht als neue Anforderung erweitern“ ergänzen.
3. Workerclaim/Attempt-Fencing, vorhandene Daten-/Conversation-IDs, externe
   OAuth-/Auth-Waits, Q1/Q2 und Sandbox-/Secret-/Netzwerkgrenzen explizit ausnehmen.
4. Neue Codingentscheidungen zum bestehenden vereinfachten Policyowner routen.
   Tatsächliche Auth-/Scopeentfernung nur in dessen bereits festgelegtem
   Consumer-/Daten-Cutover umsetzen; kein Sicherheitsumbau in diesem Textpaket.
5. Die zwei `reborn-feature`-Kopien prüfen und gegebenenfalls bytegleich anpassen.
   Deren Entwickler-Skill ist kein BrassClaw-Runtime-Skill.

**Historisch kennzeichnen:** aktuelle Legacy-Implementierungsverantwortung.
**Löschen:** widersprechende universelle neue v3-Handlungsregel, nicht Code/Altdaten.
K5 Textabgleich; tatsächlicher Policy-/Datencutover bleibt Simplified Phasen 2/3/6/8.

### D09 — MCP-Daten, Konvertierung und Auswahlidentitäten präzisieren

**Fundstellen:** Composition §16 „Reject ... source-bearing payloads“, §5
„Do not accept repr strings or depth-limit placeholders“; Connections Phase F
„Existing tasks and advertised MCP calls keep them“; entsprechende Lehr-/Roottexte.

1. `mcp_recipe_catalogue.rs`, Kommandoqualifikation, Chatbridge und gewöhnliche
   Admission verfolgen. Es bleibt nur der bestehende normale Chat-Ausführungspfad.
2. Quelltext als Ausführungspayload verbieten, erlaubten Quelldatentext nicht.
   Ein Source-Inspection-Aufruf darf korrekt erfassten Pythontext mit Quotes,
   Newlines oder Markern als deklarierte Eingabe transportieren.
3. Verbatim-/Delimiterregeln der konkreten Commanddeclaration beachten: zulässige
   Daten dürfen trotzdem an deren tatsächlicher Grammatik scheitern. Kein neues
   universelles Escaping oder Decoderfeld erfinden, um den Test passend zu machen.
4. Bei Monty→Daten-Konvertierung rohe unzulässige Typen vor verlustbehafteter
   Stringifizierung ablehnen. Einen echten String mit gleichem Markertext nicht
   durch Inhaltsblacklist sperren. Validator §7.3 ist der konsistente Bezug.
5. Advertisement pinnt beworbenen Vertrag/Identität, nicht automatisch einen
   ausführbaren gewöhnlichen Task. Vor Submit/Capture aktuelle Vertrags-/Routing-
   Kompatibilität prüfen; geändert/entzogen wird abgelehnt. Keine geheime Auswahl
   alter Recipes und kein No-Match-Fallback zur vermeintlichen Toolausführung.
6. Nach tatsächlichem Capture gilt die retained Taskauswahl durch Wait/Child/
   Abschluss. Live Tool-/Providerregeln bleiben aktuell. Providerdisconnect ist
   weder Taskcancelbeweis noch Erlaubnis zum Replay.
7. Optionales Recipe-`mcp_call` beibehalten. Abwesenheit ist zulässig, vorhandene
   korrupte Deklaration bleibt Fehler des owning Vertrags. Keine neue Exposure-Registry.
8. Echte Source-Daten, echte Markerstrings, unzulässige Rohwerte, fehlende/fehlerhafte
   Deklaration, Withdraw vor Capture, Update nach Capture und verlorene Lieferung
   durch bestehende aktuelle Callerfälle abdecken; unveränderte Fälle wiederverwenden.

**Im neuen offenen Folgeabschnitt präzisieren:** pauschales Verbot von Source-Daten/
Markertext und Gleichsetzung von Advertisement und Taskpinning. Alte implementierte
MCPabschnitte bleiben unverändert. **Erhalten:** Kommandobounds, Fehlerklassifikation,
exakte Recipe-Verträge und qualifizierter Chattransport. Neue Consumerfälle gehören
K4/K5, auch wenn die zugrunde liegende Mechanik bereits vor K4 qualifiziert wurde.

### D10 — Reales vollständiges Validator-Referenzbeispiel liefern

**Fundstellen:** Validator §§7.2/7.10, illustrative `host.inspect_source`, leere
`tool_bindings`; vorhandener Vertrag `docs/reborn/source-inspection.md` und die
zugehörigen Source-/Inventory-Factorys.

1. Aktuelle Primitive `builtin.source_inspection`/`host.source_inspection`, deren
   Source-Profil, echten Export und Resultvertrag prüfen. Der derzeit dokumentierte
   Parserreport ist JSON-Text; nicht ohne separaten Parsing-Schritt als Dict benutzen.
2. Im neuen offenen Validator-Folgeabschnitt einen tatsächlichen engsten passenden
   Beispielvertrag ergänzen; die alte Source-Facts-Passage nur als Herkunft
   verlinken, nicht ändern. Keine neue `host.inspect_source`-Primitive erzeugen.
   Falls eine abstrakte Rolle weiter nützlich ist, separat als Rolle kennzeichnen.
3. Definition ohne Aufruf und getrennte retained Invocation zeigen. Tatsächliche
   Exportsignatur aus der Factory übernehmen, nicht Keyword-/Dictformen vermischen.
4. Die unvollständige JSON-Fragmentanleitung nicht allein durch UUID-Ersetzen als
   ausführbar ausweisen. Passenden tatsächlichen ToolBindingdatensatz, ToolSkill-
   Descriptor, Association, Exportwahl, Task-/Stepinputs und Graphclosure beilegen.
   `params`/Errorpolicy aus der realen Form übernehmen; `content` konfiguriert nichts.
5. Ein geprüftes vollständiges Beispiel aus existierendem Konstruktor verwenden.
   Fehlende vollständige Approval/ordinary Activation deutlich zeigen. Eine
   Fact-Inspection ist kein Klassenvalidator und kein vertrauenswürdiger Verdictwriter.
6. Parser-/Schema-/Graphprüfung mit effektfreier Assembly und tatsächlicher
   Source-Inspection über den bestehenden contained/global Caller prüfen. Falsche
   Bindingidentität, fehlende Invocation, Dateninjection und unerwarteter Report
   scheitern ohne Kandidatencodeausführung und ohne erfundene Q1-Evidenz.

**Nur in offenen neuen Anweisungen ausschließen:** vermeintliche Produktionsanleitung
mit erfundenem Callable oder ungeklärtem leerem Binding. **Erhalten:** alte symbolische
Entwürfe unverändert; die neue vollständige Referenz erklärt ihren begrenzten Zweck.
K5/Composition liefern Referenz; acht produktive Validatorrouten bleiben Steps 1–12.

### D11 — Sempai-Promptkorrektur und Completed-turn Review trennen

**Fundstellen:** Sempai v3.5 §3 „Per-target token limits...“ und Steps 2/6/7/9–12;
Prefix dynamischer Handoff, Simplified §1.1/§10.1; aktuelle Interceptor-/Post-turn-Caller.

1. Beide bestehenden Eintrittspfade mit Caller, ursprünglichem Task, ausgewähltem
   Modell/Design/Prefix, Eingabe, Result, Effekten und Abschlussart erfassen.
   Aus dem gemeinsam verwendeten Namen keine gemeinsame Resultform ableiten.
2. Pre-call explizit definieren: bereits formatierter dynamischer Kohai-Prompt
   hinein, finaler dynamischer Prompt zurück; Sempais eigener Prefix bleibt dessen
   Requestbestandteil. Keine zweite Kohai-Formatierung oder automatische Aktivierung.
3. Post-turn explizit definieren: unveränderliche abgeschlossene Vorgangsevidenz
   hinein, Analyse und eventuell unreviewte Kandidaten zurück. Kein Zurückändern
   oder Replay der bereits veröffentlichten Originalantwort.
4. Gemeinsame Modelltransport-, Parser-, Speicher- und Pure-Teile nur bei gleichem
   Vertrag teilen. Claims, Quell-/Subjektidentität, Checksets und Completion dürfen
   durch gemeinsamen Transport nicht zusammenfallen. Q1/Q2 bleiben getrennt.
5. Die bestehende Eligibility der Post-turn-Analyse erhalten: nur die tatsächlich
   dafür zugelassenen ursprünglichen No-Match-Vorgänge, keine automatische Ausweitung
   auf alle Matched-, Validator- oder Revieweraufträge. Vorhandene Vermeidung
   rekursiver Review verfolgen und dokumentieren. Nur wenn
   eine konkrete Lücke belegt ist, im zuständigen Entry-/Transportowner korrigieren;
   kein neues Benutzerflag oder allgemeines Bypassrecht für Komponenten schaffen.
6. Beide Targets, gleiche/verschiedene Paare, optionale deaktivierte Pre-review,
   aktuelle Limits bei gepinnten Originalen, unvollständige Evidenz und verlorene
   ursprüngliche Modellantwort ohne Replay gegen vorhandene Consumerbelege abgleichen.

**Löschen/ersetzen:** mehrdeutige Result-/Sequenzbeschreibungen, die beide Zwecke
ineinander überführen. **Nicht löschen:** legitime gemeinsame Usages oder native
Settlement-/Accountinggrenzen. Begriffe und vorhandene Consumer ab K4/K5;
Businesskonvertierung bleibt
Sempai Steps 2/6/10–12 und V35-09–16/28, Serving beim Prefix-Owner.

### D12 — Zwei-Consumer-Regel auf gemeinsame Abstraktionen begrenzen

**Fundstellen:** Connections Phase E „Before adding an interface...“, Abschluss
„New shared effect primitives ... two real consumers“; Sempai Ziel zusätzlicher
Consumer; Root-/Crate-Anweisungen zu fehlenden Primitiven.

1. Universal formulierte Interface-Regel auf die behauptete Zusammenführung
   duplizierter Verträge begrenzen. Für „gemeinsam“ mindestens zwei reale passende
   Consumer und dieselben relevanten Semantiken belegen.
2. Eine notwendige neue Primitive/technische Schnittstelle mit zunächst einem
   Consumer erlauben, wenn keine vorhandene Operation/Komposition den Bedarf
   deckt. Fehlenden Vertrag und owning Layer konkret begründen.
3. Keine zweite Primitive bei bloß anderem Selector, Displaynamen oder Recipe;
   kompatible ToolSkill-/Skill-/PythonCode-Komposition bevorzugen.
4. Sempais ausdrücklich geforderte tatsächliche zusätzliche Wiederverwendung
   unverändert lassen: Dort ist sie ein Ziel der Konvertierung, kein universelles
   Existenzkriterium jeder Komponente. Keine künstlichen Verbraucher zum Zählen bauen.
5. Positive Gegenbeispiele in die Codinganweisung aufnehmen: neue wirklich fehlende
   einzelne Operation zulässig; zwei nur oberflächlich ähnliche Custodyoperationen
   nicht zu einer unsicheren Universaloperation zusammenlegen.

**Ersetzen:** pauschales Interfaceverbot bei nur einem Consumer.
**Erhalten:** Nachweispflicht gemeinsamer Semantik und Sempai-Konvertierungsziele.
K5 Textabgleich; konkrete neue Interfaces bleiben im jeweiligen bisherigen Owner.

## 7. Gemeinsame Referenz statt konkurrierender Beispiele

Die Referenz wird in den bestehenden Guides/Fixtures verankert, nicht als neuer
Ausführungsdienst oder zweite normative Schemaablage. D03/D04/D05/D06/D09/D10
benutzen dieselbe geprüfte Quelle, soweit ihre tatsächlichen Verträge passen.

1. Vorhandene Factorys, Formatter, Coverageconsumer und geeignete Fixtures suchen;
   deren reale Export-/Schematreue zuerst prüfen. Keine unzugeordneten UUIDs als
   vorhandene/aktive Komponenten ausgeben. Notwendige neue Komponenten erhalten
   Identitäten ausschließlich über den vorhandenen Authoring-/Installationspfad;
   offline Fixtureidentitäten bleiben ausdrücklich Testdaten. Keine neue Toolprimitive
   allein für das Referenzbeispiel. Ein öffentliches Read-/Filter-/Reply-Beispiel
   kann seine tatsächliche Code-/Approvalgrenze behalten, solange es als Entwurf
   gekennzeichnet bleibt. Ein bereits qualifiziertes anderes Beispiel ist nicht
   automatisch gleichwertig.
2. Für den konkreten Wiederverwendungsnachweis geeignete vorhandene Consumer
   derselben Skill-/Code-/Bindingkombination zeigen; kein Reverse-Recipe-Link im
   Skillgraph. Ein zusätzlicher Consumer ist nur nötig, wenn der behauptete konkrete
   Nachweis ihn braucht; keine neue Workflowfunktion allein zur Erfüllung einer Zahl.
3. Eine Pure-Exportdefinition ohne ToolSkill, ein ToolSkill-/Invocationpaar und
   den Ergebnis-/Coveragefluss vollständig darstellen. Matchingbeispiele und
   interne Eintrittstests gemäß D06 trennen.
4. Optionalen MCP-Teil separat zeigen. Ein vollständiger Skill ohne MCP bleibt
   gültig; ein deklariertes fehlerhaftes Kommando wird nicht ignoriert.
5. Vollständige tatsächliche retained Records aus dem vorhandenen Writer erzeugen
   oder gegen ihn prüfen: Komponentenrevisionen/Closure, Descriptor/Association,
   Layout/Invocation, Evidenzreferenzen und Workflowabschluss. Keine freistehende
   Symbol-JSONdatei als ausführbaren Nachweis ausgeben.
6. Quellen der Beispielteile exakt verlinken. Doppelte manuell gepflegte aktuelle
   JSON-/Pythonkopien nur in offenen/nicht geschützten Bereichen nach verifizierter
   Referenzangleichung entfernen; notwendige
   ausgelieferte Teachingkopien reproduzierbar synchronisieren und prüfen.

Keine eigene Example-Registry, zweite Schemaquelle, neue generische Factory oder
Dokumentgeneratorplattform bauen. Vorhandene Writer/Parser und bestehende Test-
Consumer benutzen; eine neue kleine Hilfsfunktion nur bei nachgewiesenem Bedarf.
Einen Factoryoutput ohne passende Evidenz nicht allein durch erfolgreiche Erzeugung
zu einem genehmigten ausführbaren Beispiel erklären. Ein passender Prüfbeleg bleibt
an seinen konkreten Caller, seine Features und exakten Inputs gebunden.

**Gate:** Aktuelle Dokumentreferenz kann als Design geprüft sein, während echte
Pure-/Workflowapproval noch offen ist. Erst deren tatsächliche Gates erlauben
Aktivierung; Dokumentvollständigkeit wird nicht durch erfundene Evidenz hergestellt.

## 8. Welche Abschnitte löschen, ersetzen oder historisch kennzeichnen

**Geltung dieser Tabelle:** Keine der Aktionen erlaubt die Änderung eines bereits
umgesetzten Abschnitts der fünf Pläne. Dort wird der alte Inhalt unverändert
referenziert und die jeweilige neue Anweisung ausschließlich im offenen Nachfolger
aus §1.1 formuliert. „Löschen“ gilt nur für noch unbearbeitete neue Entwürfe oder
aktuelle nicht abgeschlossene Guide-/Teachingtexte. „Historisch kennzeichnen“
bedeutet bei geschützten Passagen ausschließlich eine Einordnung im neuen
Folgeabschnitt, keine nachträgliche Beschriftung des Originals.

| Abschnitt/Text | Aktion | Schutzbedingung |
| --- | --- | --- |
| Simplified Phase 3a „derzeit v0.0.16 / noch nicht umgesetzt“ | Original erhalten; im neuen offenen K4-Folgeabschnitt den historischen Ausgangspunkt referenzieren und den aktuellen Ausgangsvertrag formulieren. | Alte Einleitung/Trackerpassagen samt Belegen nicht verändern oder verschieben. |
| Composition §§4/5 globale `result`-Pflicht | Original erhalten; im neuen offenen Composition-Folgeabschnitt D02 für zukünftige Arbeit definieren. | Keine rückwirkende Änderung umgesetzter Body-/Preloadabschnitte. |
| Pure-Entwürfe mit Bodyform statt behauptetem Export | Neue vollständige Funktion/Invocation im offenen Nachfolger ergänzen. Nur noch unbearbeitete Entwürfe dürfen dort ersetzt werden. | Umgesetzte Planbeispiele und eingefrorene Prüfarchive unverändert erhalten. |
| Teaching Decision map universelle Modulzuweisung | Universalität löschen, auf Standalone-Vertrag beschränken. | Dessen tatsächlich passende Beispiele bleiben verwendbar. |
| Read-/Filterformatter ohne Coverage | Neuen Coverage-/Antwort-/Layoutnachfolger gemäß D04 im offenen Abschnitt definieren. | Alten Implementierungsabschnitt und freigegebene Resultverträge nicht ändern. |
| Als SQL/JSON verwendete `channel`-/erfundene Stepfelder | In neuen offenen Codinganweisungen tatsächliche Felder zeigen; in noch unbearbeiteten Entwürfen falsche Felder entfernen. | Umgesetzte alte Diagramme unverändert referenzieren; zulässige Inputref-`step_id`-/retained Dokumentfelder erhalten. |
| Zehn öffentliche Intents für private Workflows | In neuen offenen Plananweisungen Eintrittsartenregel ergänzen; aktuelle nicht abgeschlossene allgemeine Guides passend präzisieren. | Alte umgesetzte Plananforderung und öffentliche Matching-/Negativabnahme erhalten. |
| `source = system` als ausreichender Seedweg | Verkürzte Freigabeanweisung löschen/ersetzen. | Legitime Seeder und vertrauenswürdige Bootstrapmechanik behalten. |
| Fingerprinted Approval-Lease als neue v3-Dispatchpflicht | Aus Zielanweisung entfernen; verbleibenden Code lokal als Legacy beschreiben. | Technische Claims, Datenidentitäten, Auth und Security nicht entfernen. |
| MCP pauschal „source-bearing payloads“ verbieten | Im offenen Folgeabschnitt ausführbaren Payload von deklarierten Source-Daten trennen. | Qualifizierten alten MCPabschnitt und echte Grammatik/Bounds erhalten. |
| Stringmarker pauschal als ungültige Daten behandeln | Im offenen Nachfolger rohe verlustbehaftete Konvertierung ablehnen und echte Strings ausdrücklich zulassen. | Alte umgesetzte Bezugspassagen nicht löschen. |
| MCP Advertisement als bereits ausführbare Taskauswahl | Zwei explizite Identitäts-/Zeitpunkte in der offenen Folgearbeit definieren. | Alten Abnahmeumfang, Stale-before-capture Ablehnung und retained laufende Tasks erhalten. |
| Validator erfundenes Beispielcallable / leeres Binding | Reales vollständiges Beispiel im offenen Nachfolger ergänzen. | Ursprünglichen symbolischen Entwurf unverändert als Herkunft referenzieren. |
| Sempai undeutlicher Vorher-/Nachher-Review | Durch zwei separate Workflowverträge ersetzen. | Kein neuer Reviewloop oder unbelegter Bypass. |
| Zwei-Consumer-Pflicht für jedes neue Interface | Universalität löschen; gemeinsame Abstraktionen gesondert prüfen. | Tatsächliche Wiederverwendungsziele und Sicherheitsgrenzen erhalten. |
| Wiederholte aktuelle Beispiel-/Schemafassungen | Nach gültiger Referenzumstellung überflüssige Kopien löschen. | Shipped Inhalte nur über qualifizierte neue Pakete ersetzen; stabile Links/Anker erhalten. |

Nicht löschen: ganze Implementierungsjournale, alte Seeds/UUIDs, angewandte
Migrationen, native Artefakte, noch verwendete Aliase, Overrideinformationen oder
historische Acceptance-Receipts. Ein alter Befund wird im neuen offenen
Folgeabschnitt eingeordnet und nicht nachträglich im Original umgeschrieben.

## 9. Auswirkungen auf aktuelle und zukünftige Planabschnitte

| Plan | Ausschließlich offenes Änderungsziel | Zukunft präzise weiterführen |
| --- | --- | --- |
| Simplified | Bereits gelieferter K4-Nachfolger gemäß §1.1; nur nach aktuellem Einzelstatus noch unbearbeitete Ergänzungen K4–K7. Verweise auf §§1.1/9, Phase 3a und gemeinsame Runtime-/Reviewverträge sind Quellen, keine Editaufträge an umgesetzten Stellen. | Noch offene neue Consumer in Phasen 0a/3a/6–8, §8.2 und §10.1 präzisieren, soweit deren Einzelaufgabe tatsächlich unbearbeitet ist; sonst ausschließlich offene Folgearbeit hinzufügen. |
| Validator | Vorhandener Definitionsnachfolger im Remaining-Bereich gemäß §1.1; kein erneuter Anlageauftrag. Neue Deltas zu Subject/Preload/Beispielen mit Bezug auf Steps 1–3/5a/6/7/10–12 und §§7.2/7.3/7.10 dort angeben. Die alten Steps/Beispiele nicht pauschal bearbeiten. | Acht Klassenrouten, Pure-Approval, vertrauenswürdige Writer, Verhalten und Q2 behalten ihre tatsächlich offenen Aufgaben; fertige Voraussetzungen unverändert verwenden. |
| Composition | Vorhandener Contract-/Referenznachfolger im Remaining-Bereich; neue Anweisungen nur zu noch unbearbeiteten Einzelaufgaben. §§4/5/10/16 und qualifizierte §§27–35 bleiben Bezugspassagen, keine Änderungsziele. | Offene Familienmigration, Commands und Parentconsumer konsumieren die neuen Anweisungen ohne Wiederöffnung fertiger Loader-/Reply-/MCP-Unterstützung. |
| Implementierungsanhang | Vorhandener Komponenten-/Beispielnachfolger im Remaining-Bereich; gelieferte Definitionen nicht wiederholen. Neue Pure-/Coverage-/Parserbeispiele dort, alte L-/N-/R-/Formattertexte nur verlinken. | R10 hält Rohprovenienz getrennt von Vorschauabdeckung; R09 und Subagentmigrationen behalten ihre Gates. Keine Konvertierung bereits verwendeter Beispiele in place. |
| Prefix | Vorhandener Definitions-/Teaching-/Consumernachfolger im Remaining-Bereich; nur tatsächlich unbearbeitete Consumerfolgearbeit ergänzen. Frühere Phase-A-/§5-/Source-/Phase-B/C-Passagen nur als Herkunft referenzieren. | Nur tatsächlich unbearbeitete neue A1/C1/C2-/Serving-/Upgradeconsumer ergänzen; fertige Exporte, Jobs und Messprerequisites weder umschreiben noch erneut implementieren. |
| Sempai | §3 und Steps 2/6/10–12 zwei Reviewzwecke und echte gemeinsame Usages; Lehrquellen angleichen. | Steps 4/8 dynamische Occurrences, Businesskonvertierung und Cutover bleiben offen; ursprüngliche Antworten/Kandidaten nicht für neue Evidenz wiederholen. |
| Connections | Phase E Interface-Regel, D/F Advertising-/Taskzeitpunkte, private Consumer und Abnahmematrix. | Konsolidierung, Recovery und Upgrade brauchen echte Consumersemantik; notwendige neue einzelne technische Schnittstelle zulässig. |
| v3.5-Roadmap | V35-02 Dokumente, 04/06/07 Submission/Referenz/Deployment, 16 gemeinsame Sempai-Nutzung, 17–20 Validator/Prefix, 25 MCP passend referenzieren. | Kein Work-ID wird durch eine Prosaänderung als implementiert markiert. V35-03/04 und 13/14 behalten ihre eigenen fehlenden Unterstützungsgates. |

## 10. Vollständiger 67-Dateien-Abgleich

Dies ist die Liste der früheren Änderung, nicht eine Anweisung, jede Datei erneut
zu ändern. „Prüfen“ kann mit begründetem „keine weitere Änderung“ enden. Jeder
Eintrag erhält seine Disposition im bestehenden Owner-Record; keine neue dauerhafte
Statusdatenbank. Invarianten aus §3 überall prüfen; anschließend nur die genannten
lokalen **offenen** Regeln/Beispiele ändern. Für die fünf v3-Pläne ist ausschließlich
das Editziel aus §1.1 zulässig; die unten genannten alten Abschnitte sind Quellen.
Zusätzliche tatsächlich abhängige Dateien ebenfalls
erfassen, insbesondere Quellfixtures, echte Seedwriter, Referenzmanifest und Consumer.

| Nr. | Datei | Konkreter Schwerpunkt | Umsetzung / Disposition |
| --- | --- | --- | --- |
| 1 | [.agents/skills/reborn-feature/SKILL.md](../../.agents/skills/reborn-feature/SKILL.md) | D08 Routing/Owner; Entwickler- versus Runtime-Skill klar, Spiegelgleichheit. | Geprüft; Entwickler-Skill-Spiegel bytegleich, kein neuer Delta. |
| 2 | [.claude/rules/skills.md](../../.claude/rules/skills.md) | D02/D07; Label-Shortcut und aktuelle Input-/Preloadanweisung. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 3 | [.claude/skills/reborn-feature/SKILL.md](../../.claude/skills/reborn-feature/SKILL.md) | Spiegel von Nr. 1, gleiche Metadaten/Invocationpolicy erhalten. | Geprüft; Spiegel zu Nr. 1 bytegleich, keine Änderung. |
| 4 | [AGENTS.md](../../AGENTS.md) | D01/D05/D06/D08 und nur Routing statt konkurrierender Schemaautorität. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 5 | [CLAUDE.md](../../CLAUDE.md) | Gleiche Verträge wie Nr. 4, lokale historische Körper nicht durch Banner verschleiern. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 6 | [README.md](../../README.md) | Kurzbeschreibung gegen §3 prüfen; keine zusätzliche Vollständigkeits-/MCPpflicht. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 7 | [crates/AGENTS.md](../../crates/AGENTS.md) | D08 Tabelle Authorization/Approvals/Runtime-Policy lokal kennzeichnen. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 8 | [agent_loop/AGENTS](../../crates/brassclaw_agent_loop/AGENTS.md) | D01/D08 aktuelle versus Legacy-Loopverantwortung. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 9 | [agent_loop/CLAUDE](../../crates/brassclaw_agent_loop/CLAUDE.md) | Wie Nr. 8, kein Rust-Recipefallback. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 10 | [engine/AGENTS](../../crates/brassclaw_engine/AGENTS.md) | D02/D05/D08 Runnerartefakte und echte technische Grenzen. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 11 | [engine/CLAUDE](../../crates/brassclaw_engine/CLAUDE.md) | D01/D02 Implementierungsinventar/Preload, keine neuen Legacy-Konstruktionen. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 12 | [engine/MONTY](../../crates/brassclaw_engine/MONTY.md) | D01 korrekt gekennzeichnete Historie erhalten; aktuellen Pin nur einmal maßgeblich erklären. | Geprüft; aktueller Control.7-Pin und historische Grenzen bereits lokal korrekt. |
| 13 | [extensions/AGENTS](../../crates/brassclaw_extensions/AGENTS.md) | §3 und D08 Domain-/Komponenten-/Kernelrollen prüfen. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 14 | [extensions/CLAUDE](../../crates/brassclaw_extensions/CLAUDE.md) | Wie Nr. 13; Metadatum ist kein Grant/Ausführung. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 15 | [host_runtime/AGENTS](../../crates/brassclaw_host_runtime/AGENTS.md) | D08/D12 technische Primitive/Policy statt Workflowloop. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 16 | [host_runtime/CLAUDE](../../crates/brassclaw_host_runtime/CLAUDE.md) | Wie Nr. 15, echte Binding-/Hostgrenzen erhalten. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 17 | [pg/AGENTS](../../crates/brassclaw_pg/AGENTS.md) | D07/D08 Revisionen/Transaktionen versus Freigabe, keine neue Zuständigkeitskopie. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 18 | [product_workflow/AGENTS](../../crates/brassclaw_product_workflow/AGENTS.md) | D06/D08/D09 Admission-/Verwaltungs-/Toolrechte unterscheiden. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 19 | [product_workflow/CLAUDE](../../crates/brassclaw_product_workflow/CLAUDE.md) | Wie Nr. 18, keine neue Engine aus einem Produktport. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 20 | [reborn/AGENTS](../../crates/brassclaw_reborn/AGENTS.md) | D01/D08/D11 tatsächlich ausgewählte Driver/Gatewayrollen. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 21 | [reborn/CLAUDE](../../crates/brassclaw_reborn/CLAUDE.md) | Wie Nr. 20, historische Driver-/Profilangaben lokal kennzeichnen. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 22 | [reborn_cli/AGENTS](../../crates/brassclaw_reborn_cli/AGENTS.md) | D01/D08 Management braucht keinen zusätzlichen Orchestrator. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 23 | [composition/AGENTS](../../crates/brassclaw_reborn_composition/AGENTS.md) | D01/D05–D11 konkrete unterstützte Composition-/Review-/MCPpfade. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 24 | [composition/CLAUDE](../../crates/brassclaw_reborn_composition/CLAUDE.md) | Wie Nr. 23; vorhandene Writer/Consumer erhalten, neue Abgleiche erst K4 zuordnen. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 25 | [webui_ingress/AGENTS](../../crates/brassclaw_reborn_webui_ingress/AGENTS.md) | D06/D08/D09 authentische Herkunft versus Operatoridentität. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 26 | [webui_ingress/CLAUDE](../../crates/brassclaw_reborn_webui_ingress/CLAUDE.md) | Wie Nr. 25, technische Ingressguards nicht löschen. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 27 | [skills/AGENTS](../../crates/brassclaw_skills/AGENTS.md) | D01/D02/D07 crate-eigene v3-Revisionen gegenüber alten Typ-/Injectionpfaden. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 28 | [completed_turn_review_components.rs](../../crates/brassclaw_skills/src/completed_turn_review_components.rs) | D07/D11 deklarierte interne Consumer, keine Skill-Eigentümerschaft; Versionsimpact. | Geprüft; interne Consumerreferenzen bereits optional, Seedbytes unverändert. |
| 29 | [global_bootstrap_components.rs](../../crates/brassclaw_skills/src/global_bootstrap_components.rs) | D02/D07/D09 tatsächlicher öffentlicher Replyvertrag und qualifizierte Deklaration. | Geprüft; öffentlicher Reply-/Deklarationsvertrag unverändert. |
| 30 | [model_completion_components.rs](../../crates/brassclaw_skills/src/model_completion_components.rs) | D02/D07/D11 Modellusage, JSON-Result und Consumervertrag nicht vermischen. | Geprüft; vorhandene Modellusages unverändert; zwei Reviewzwecke im offenen Sempai-Plan präzisiert. |
| 31 | [prefix_generation_components.rs](../../crates/brassclaw_skills/src/prefix_generation_components.rs) | D07 erzeugte Prosa bleibt konkrete Referenz; kein zusätzliches eigenes Recipe verlangen. | Geprüft; referenzierter Consumer keine Skillvoraussetzung; generierte Seedbytes unverändert. |
| 32 | [prefix_source_components.rs](../../crates/brassclaw_skills/src/prefix_source_components.rs) | D07 echte Source-/Resultverträge und konkrete Consumer erhalten. | Geprüft; konkreter Source-/Storeconsumer unverändert, keine neue Recipepflicht. |
| 33 | [validator_components.rs](../../crates/brassclaw_skills/src/validator_components.rs) | D02/D03/D07/D10 echte Fact-/Pure-/Validatorrollen und Nachfolgerbytes. | Geprüft; bestehende Fact-/Pure-Factory als Referenz genutzt; kein Seedwriter geändert. |
| 34 | [turns/AGENTS](../../crates/brassclaw_turns/AGENTS.md) | D01/D06/D08/D09 fachliche IDs/Claims, keine Profile als neue Grants. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 35 | [turns/CLAUDE](../../crates/brassclaw_turns/CLAUDE.md) | Wie Nr. 34; Taskcompletion ist kein Approval. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 36 | [webui_v2/AGENTS](../../crates/brassclaw_webui_v2/AGENTS.md) | D06/D07/D08 genaue Subject-/Freigabe-/Publikationsdarstellung. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 37 | [webui_v2/CLAUDE](../../crates/brassclaw_webui_v2/CLAUDE.md) | Wie Nr. 36; UI erfindet weder API noch Evidence. | Geprüft; vorhandene Rollen-/Ownergrenzen passen zum D-Scope, kein zusätzlicher Edit. |
| 38 | [development-policy.md](../development-policy.md) | Evidenzgrenzen erhalten, keine zweiten generellen Regeln aus D-Paketen. | Geprüft; bestehende Evidenz-/Buildpolicy angewandt, keine zweite Policy. |
| 39 | [implementation-status.md](../implementation-status.md) | D01 aktuelle scoped Tatsachen; Dokuarbeit ist keine komplette Architekturabnahme. | Source-only Nachweislink ergänzt; keine neue Runtime-/K-Abnahme. |
| 40 | [Recipe_Reviewed_v3_plan.md](Recipe_Reviewed_v3_plan.md) | D02/D05/D06 Ziel/alte Reviewwege auf gemeinsame Verträge abgleichen. | Geprüft; bestehender Ziel-/Reviewentwurf bleibt im bisherigen Scope. |
| 41 | [monty.composition.implementation.md](monty.composition.implementation.md) | D02–D05/D07/D09–D11 sämtliche tatsächlichen Beispiel-/Consumerinstanzen. | Nur offener Nachfolger ergänzt; alle ursprünglichen Abschnitte unverändert. |
| 42 | [monty.composition.md](monty.composition.md) | D01/D02/D04–D06/D09 aktuelle Kriterien und konkrete Beispielkanten. | Nur offener Nachfolger ergänzt; alle ursprünglichen Abschnitte unverändert. |
| 43 | [monty.composition.recipe-implementation.md](monty.composition.recipe-implementation.md) | D03–D06/D09/D11 betroffene RCP-Verbraucher, keine pauschale Neuschreibung aller Recipes. | Geprüft; öffentliche RCP-Coverage bereits ausdrücklich öffentlich, alte Beispiele unverändert. |
| 44 | [monty.composition.recipes.md](monty.composition.recipes.md) | D01/D05/D06/D09 alte Definitionen versus aktive Pfade, kein routbares internes Recipe erfinden. | Geprüft; Schema-/Bindinglücken bereits begrenzt, keine neue Umsetzung behauptet. |
| 45 | [monty.composition.skill-recipes.md](monty.composition.skill-recipes.md) | D05/D06/D09/D10 SKR-Zweck/Bindings/Kommandos, historische Anker erhalten. | Geprüft; MCP-/Referenzrollen bereits separat, keine globale Recipe-Kardinalität. |
| 46 | [prefix_v3_upgrade.md](prefix_v3_upgrade.md) | D01/D03/D05–D07/D09–D11 Quellen/Teaching/A1/C1/C2, heutige Closure versus Zielinventur. | Nur offener Nachfolger ergänzt; alle ursprünglichen Abschnitte unverändert. |
| 47 | [sempai_v3.5.md](sempai_v3.5.md) | D03/D06/D07/D11/D12 zwei Zwecke, echte Wiederverwendung und zukünftige Gates. | Zwei zukünftige Reviewverträge getrennt; bestehende Business-/Aktivierungsgates offen. |
| 48 | [simplified-v3-implementation.md](simplified-v3-implementation.md) | D01 aktuelle Übersicht korrigieren; historische journalartige Einträge/Receipts erhalten. | Geprüft; Implementierungsjournale/Receipts unverändert. |
| 49 | [simplify_connections_v3.5.md](simplify_connections_v3.5.md) | D06/D08/D09/D12 gemeinsame Semantik, Advertising/Capture und Interface-Regel. | Offene Interface-/MCP-Zuordnung korrigiert, keine Custodyimplementierung geändert. |
| 50 | [zencoder-extension-plan.md](zencoder-extension-plan.md) | D01/D05/D07 vorhandene/fehlende Handler und Blockaden ehrlich halten. | Geprüft; tatsächliche Handler-/Blockadegrenzen unverändert. |
| 51 | [component-deployment.md](../reborn/component-deployment.md) | D06/D07 unveränderte complete-Recipe-Deploymentgrenze, kein eigener Skillwrapper als Pflicht. | Geprüft; Complete-Recipe-Deployment und Usageprüfung bereits getrennt. |
| 52 | [retained-component-inputs.md](../reborn/contracts/retained-component-inputs.md) | D02/D05 typed Eingaben/Resultrefs und Sourcegrenzen. | Geprüft; unterstützte Resultref-Grammatik unverändert. |
| 53 | [retained-first-party-capability.md](../reborn/contracts/retained-first-party-capability.md) | D07/D08/D09 exakte Native-/Policy-/Invocationidentität. | Geprüft; native Auswahl/Policy/Artefakte unverändert. |
| 54 | [raw-file-snapshot.md](../reborn/raw-file-snapshot.md) | D04 Rohaufnahmeprovenienz gegenüber Vorschau-Coverage erhalten. | Geprüft; Rohprovenienz bleibt getrennt von Vorschau. |
| 55 | [source-inventory-inspection.md](../reborn/source-inventory-inspection.md) | D09/D10 korrekte Source-Daten, endlicher Parserreport und private/optionale Commandconsumer. | Geprüft; bestehender Inventarparser/-decoder wiederverwendet, kein zweiter Parser. |
| 56 | [recipe-first-architecture-audit.md](../recipe-first-architecture-audit.md) | D01/D08/D12 aktuelle Routingvorschläge versus historische Quellbeobachtungen. | Geprüft; historische Befunde und vorhandene Owner bleiben erhalten. |
| 57 | [path_to_v3.5.md](../../path_to_v3.5.md) | §9 dieses Plans auf bestehende Work-IDs abbilden, keine fiktiven Abschlüsse. | Bestehende Work-IDs präzisiert, keine Work-ID abgeschlossen. |
| 58 | [prefix_plan2.md](../../prefix_plan2.md) | D01 als ältere Quelle behandeln; aktuelle Leser zum Upgradeplan routen. | Geprüft; ältere Quelle bereits zum Upgradeowner geroutet. |
| 59 | [recipe.md](../../recipe.md) | D02/D04–D06 öffentlich/privat, echtes Schema und checked Referenz. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 60 | [sempai-authoring-reference.md](../../scripts/prefix/sempai-authoring-reference.md) | D02/D03/D05–D07/D09/D11 aktuelle Authoringverfahren. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 61 | [sempai-monty-compatibility.md](../../scripts/prefix/sempai-monty-compatibility.md) | D01/D02/D03/D09 korrekte Artefaktart und Marker-/Source-Daten. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 62 | [sempai-worked-examples.md](../../scripts/prefix/sempai-worked-examples.md) | D02/D03/D05/D06 vollständige aktuelle versus Standalone-Beispiele. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 63 | [simplified_v3.md](../../simplified_v3.md) | §4/§9 dieses Plans, D01/D05–D09/D11 und aktuelle/future Gates. | Nur offener Nachfolger ergänzt; alle ursprünglichen Abschnitte unverändert. |
| 64 | [skills.md](../../skills.md) | D02/D03/D06/D07/D09 kein Recipe als Skillpflicht, genaue Usage-/Reviewgrenzen. | Dokumentdelta umgesetzt; aktuelle Guides/Teaching korrigiert, keine Aktivierung. |
| 65 | [tools.md](../../tools.md) | D07/D08/D12 Primitive, echtes Artefakt, Livepolicy; keine Workflowhülle. | Geprüft; Primitive/Livepolicy/fehlende Operation bereits korrekt getrennt. |
| 66 | [toolskills.md](../../toolskills.md) | D05/D07 Bindingdeskriptor versus Code/Grant; tatsächliche Adapterregeln. | Geprüft; Binding ist Metadatum, reale Adapter-/Schrittgrenzen unverändert. |
| 67 | [validator_v3.md](../../validator_v3.md) | D01–D03/D05–D07/D09–D11 Subjects, Regeln, tatsächliche Beispiele und künftige Writer. | Nur offener Nachfolger ergänzt; alle ursprünglichen Abschnitte unverändert. |

## 11. Nachweise, Kommunikation und Abschluss

### 11.1 Abnahme nach geändertem Vertrag auswählen

| Änderung | Notwendiger Nachweis |
| --- | --- |
| Nur Anweisung/Historielabel/Link | Relevanten Diff, Logik, Links/Anker und Whitespace prüfen. Kein Cargo nur für Prosa. |
| Entwickler-Skill-Kopie | Zusätzlich passende Skill-Strukturprüfung, Invocationpolicy erhalten, Spiegel bytegleich. |
| Python-/JSON-Beispiel | Tatsächliche Schema-/Konstruktorform und Semantik prüfen; Syntax allein ist kein Monty-/Runtimebeleg. |
| Geänderte Seedprosa/eingebettete Guides | Tatsächliche Release-/Seedinputs und neue Checksums/Revisionen erfassen; relevante vorhandene Factory-/Integritäts-/Bootstrap-/Consumerprüfungen. Kein Umbenennen alter Receipts. |
| Reader/Runner/Resultschema geändert | Fokussierter echter Caller-Test plus relevante Lints/Features; negative Pfade, mutable Isolation und Output-/Effektfehler, soweit betroffen. |
| Admission/DB/Publikation/Upgrade | Tatsächlicher Backend-/Produktcaller, Commit-/Identity-/Retentionsfälle; keine Fakepasses oder broad Vollsuite ohne Bedarf. |
| Bereits vorhandene akzeptierte Mechanik | Exakte Anwendbarkeit des Belegs prüfen, referenzieren und nicht identisch erneut implementieren/testen. |
| Bereits umgesetzter Planabschnitt | Text und Abschnittsgrenzen vor/nach dem Paket vergleichen: keine inhaltliche Änderung, Verschiebung oder nachträgliche Beschriftung. Neue Anweisung liegt außerhalb im offenen Folgeabschnitt; ein neuer Whole-file-/Pakethash ist kein Grund, alte Abschnittsbelege umzuschreiben. |

Für alle kompilierten Checks gelten gemeinsame Source-/Buildownership,
Diskspacegrenzen und serieller Cargo-Lock aus der Entwicklungspolicy. Null Fälle,
Skip oder anderer Byte-/Featurestand gilt nicht als passende neue Acceptance.

### 11.2 Getrennte Abschlussarten statt eines unbedingten Komplettheitslabels

| Art | Wann sie zutrifft | Erforderlicher Abschluss und Grenze |
| --- | --- | --- |
| Geprüfte Dokument-/Contractlieferung | Korrekte offene Nachfolgeranweisung oder nicht geschützte Guidepräzisierung ohne Runtimeverhaltensänderung. | Logik, Quellen, Links, Typ-/Artefaktzuordnung und offene Abhängigkeiten geprüft. Das erfüllt keine neue Produkt-/Pure-/Bootstrap-Akzeptanz. |
| Tatsächlich betroffener vorhandener ausführbarer Pfad | Aktueller Caller erreicht den bestätigten geänderten oder defekten Vertrag. | Neue gezielte Folgearbeit beim vorhandenen Owner mit realem anwendbarem Nachweis. Ein aktiver ungelöster Fehler kann nicht als bloßer Zukunftsentwurf disponiert oder zum Bestehen von K7 versteckt/deaktiviert werden. |
| Zukünftiger nicht aktivierter Entwurf | Benötigter neuer Consumer/Graph ist noch nicht unterstützt oder nicht aktiviert. | Vollständiges benanntes Delta, Voraussetzungen, Owner und tatsächliches offenes Gate verlinkt; keine erfundene Ausführungs-/Approvalevidenz. Dokumentarbeit kann fertig sein, die spätere Implementierung ausdrücklich nicht. |

Für jedes D-Paket vor der Checkauswahl die zutreffenden Arten bestimmen. Ein Paket
kann mehrere besitzen: geänderte Teachingbytes sind Dokument-/Paketarbeit, während
der beispielhaft beschriebene noch nicht aktivierte Pure-Graph zukünftige Arbeit
bleibt. Das eine Ergebnis darf nicht als Beweis für das andere ausgegeben werden.

- [ ] D01–D12 und neu konkret gefundene Abhängigkeiten sind anhand aktueller Quellen
  als bestätigt/erledigt/zu prüfen und nach obiger Abschlussart disponiert. Kein
  Wortlautbefund wird ohne aktuellen Callerbezug zu einem behaupteten Runtimebug.
- [ ] Für die geänderten offenen Agentenanweisungen sind Skill-/Recipe-, Approval-/
  Policy-, private Admission- und Artefaktgrenzen widerspruchsfrei. Geschützte alte
  Texte bleiben erhalten und werden von neuen Anweisungen eindeutig eingeordnet.
- [ ] Coverage-/Source-Inspection-/Sempai-/MCP-Nachfolger enthalten genaue Verträge.
  Wo ein aktueller ausführbarer Consumer betroffen ist, stimmen Implementierung
  und echte Evidence. Wo nur ein Entwurf existiert, wird keine Ausführung behauptet.
- [ ] Die historische 67-Dateien-Liste ist auf aktuelle Existenz/Zuständigkeit
  geprüft; kein Zwang, jede Datei zu ändern oder sechs Seeddateien pauschal neu
  auszuliefern. Neue tatsächlich relevante Verbraucher sind zusätzlich erfasst.
- [ ] Neue Zuordnungen beginnen ausschließlich bei K4; K4–K7 und tatsächliche
  zukünftige Owner sind synchron. K0–K3 bleiben abgeschlossen und unverändert;
  offene Aufgaben werden weder versteckt noch zu neuen pauschalen K3-Gates gemacht.
- [ ] Jede Planänderung liegt in einem bestätigten offenen Editziel; kein Inhalt
  eines geschützten logischen Abschnitts wurde geändert oder umgehängt. Verschobene
  physische Zeilennummern allein sind kein Verstoß.
- [ ] Fremde aktuelle Edits wurden erhalten, Änderungen seit den Checks auf deren
  echte Inputclosure geprüft und nur anwendbare Resultate als Beleg verwendet.
- [ ] Geänderte Paketbytes besitzen vor behaupteter installierter Nutzung ihre
  tatsächlichen Nachfolger-/Integritätsnachweise. Bis dahin explizit source-/design-
  only berichten; alte Receipts behalten ausschließlich ihren ursprünglichen Scope.

K7 verwendet weiterhin seinen bestehenden Vertrag für die bereits unterstützten
Pfade. Eine bloße Verlinkung zukünftiger Arbeit repariert keinen dort tatsächlich
aktiven Fehler; umgekehrt verlangt Dokumentabnahme nicht das Freischalten aller
noch offenen v3-/v3.5-Funktionen.

### 11.3 Zahlen und Grenzen aus ihrem tatsächlichen Owner übernehmen

- Klassenkennungen, Formatversionen, K-/D-/V35-IDs und die frühere 67-Dateien-Liste
  sind Identitäten/Inventar, keine Runtimequoten. Die zwölf D-Pakete begrenzen nicht
  die Anzahl tatsächlich zu findender Consumer oder Fehler.
- Eine frühere Zahl privater geprüfter Schritte oder Routen ist historischer
  Evidenzumfang, kein hardcodiertes Bestehenskriterium. Aktuellen Umfang aus dem
  maßgeblichen Manifest und der realen Route ableiten.
- Öffentliche Intent-Beispielanforderungen kommen aus `recipe.md`, nicht aus einem
  neu kopierten Zähler in Runtime/Validator/Tests. D06 ändert den Geltungsbereich,
  nicht still den bestehenden öffentlichen Policywert. Interne Herkunftstests
  hängen nicht von einer künstlichen Anzahl öffentlicher Beispiele ab.
- Daten-/Parser-/Source-/Transport-/Modell-/Zeit-/Speichergrenzen beim vorhandenen
  Settings-/Schema-/Capabilityowner erfassen. Keine feste neue Byte-, Token-,
  Retry-, Timeout- oder Listenobergrenze für die Beispiele erfinden; endliche
  Schemas/Bounds brauchen ihren konkreten begründeten Consumervertrag.
- Ein Beispielwert/Fixturegrenzfall ist ausdrücklich Testdatenpunkt, kein neuer
  Produktdefault. Historische Werte werden weder in aktuelle Einstellungen kopiert
  noch benutzt, um Output zu kürzen, ungültige Inputs zu reparieren oder Evidenz
  still wegzulassen. Unterschiedliche technische Grenzen nicht aus Bequemlichkeit
  zu einer einzigen Zahl zusammenlegen.

### 11.4 Andere Implementierungen informieren

Nach einem fertigen kohärenten Änderungspaket den bereits vom Menschen autorisierten
anderen Implementierungs-Chats konkrete Pfade/Hashes und Auswirkungen mitteilen:
geänderte Vertrags-/Seed-/Dokumentbytes, bestehende unveränderte Owner und Gates,
aktuelle Testscope sowie erforderliche neue integrierte Inputs. Nachrichten sind
Information, keine eigenmächtige Übernahme ihrer Arbeit oder neue Releasefreigabe.
Den Umfang der Nachricht ausdrücklich nennen: Planänderung, Sourceänderung oder
qualifiziertes ausführbares Delta. Bei einem reinen Plan-Audit keine Neutests,
Neubuilds, Unterbrechungen oder neue Aufgaben ohne konkrete betroffene Inputs
verlangen. Rückmeldung eines anderen Chats ist kein allgemeines Abschlussgate;
eine echte noch ungeklärte gemeinsame Abhängigkeit bleibt konkret ausgewiesen.

Insbesondere Prefix-/Runtime-Chats dürfen ältere Bilder weiter für ihren historischen
Diagnoseumfang verwenden, aber nicht als Abnahme geänderter eingebetteter Bytes.
Keine unerwarteten Edits nach dem angekündigten Paketabschluss. Bei neuen
notwendigen Änderungen zuerst den betroffenen Nachweisumfang neu abgleichen.

**Abschluss dieses Planauftrags:** Diese Datei und ihre Quellen-/Link-/Logikprüfung.
**Abschluss der späteren Umsetzung:** die umgesetzten begrenzten D-Pakete mit ihren
Ownernachweisen und ehrlicher K7-Disposition. Beides ist weder komplette v3-/v3.5-
Implementierung noch produktiver Cutover, Deployment oder universelle Crash-Recovery.

## 12. Umsetzung des autorisierten Dokumentabgleichs — 2026-10-28

Der nachfolgende Betreiberauftrag autorisiert die begrenzte Umsetzung dieses Plans.
Die Dokument-/Contractlieferung ist erfolgt; maßgeblich ist der
[genaue Nachweis](../validation/2026-10-28-exact-definition-update.md) mit
[betroffenen Byteidentitäten](../validation/2026-10-28-exact-definition-inputs.json).
Die 67 individuellen Dispositionen in §10 sind keine universelle Codeabnahme.

| Paket | Geliefertes Ergebnis | Weiterhin bestehende Grenze |
| --- | --- | --- |
| D01 | Lokale Engine-v2-Historie präzisiert; aktueller Monty-/Loaderausgangspunkt im offenen K4-Nachfolger. | Kein erneutes Upgrade und kein Edit alter Phase-3a-Belege. |
| D02 | Vier Artefaktarten in Composition und aktuellem Teaching getrennt. | Bestehende Loader-/Runnerprüfung unverändert. |
| D03 | 15 Python-Teachingprogramme als Standalone/Test klassifiziert, Spiegel unverändert; zwei vollständige zukünftige Pure-Definitionen mit separaten Verträgen. | Keine allgemeine Pure-Aktivierung; alte umgesetzte Beispiele erhalten. |
| D04 | Vollständiger zukünftiger Read-/Selection-/Coverage-/Formatter-/Replyvertrag samt Offset/Limit und Fehlerfällen. | Kein nachgewiesener aktiver Formatterbug; kein aktives Layout oder R10-Rohprofil geändert. |
| D05 | Tatsächliche Factory-/Binding-/Layout-/Invocationreferenz; richtige Stepfelder und zulässige Inputref-Felder. | Kein neues Schema oder symbolische IDs als aktive Daten. |
| D06 | Öffentliche Intents und private typisierte Aufnahme in Guide/Root/offenen Plänen getrennt. | Tatsächliche Parser-/Public-MCP-Qualifikation bleibt erforderlich. |
| D07 | Quellenlabel-Shortcut entfernt; bestehende immutable Bootstrap-/Authored-/Upgradepfade präzisiert. | Keine sechs Seedwriter neu ausgeliefert; installierte Übernahme nicht behauptet. |
| D08 | Legacy-Lease-/Profilrollen lokal im Crate-Router und Engine-Inventar eingeordnet. | Keine Claims/Auth/Policy entfernt; Entwickler-Skill-Spiegel unverändert gleich. |
| D09 | Ausführungspayload versus Source-Daten, rohe Konversion versus Markerstrings und Advertisement versus Taskcapture präzisiert. | Reale Grammatik/Qualifikation/Livepolicy und alte Taskauswahl unverändert. |
| D10 | Vollständige vorhandene Parserreferenz im bestehenden Source-Inspection-Guide, aus realer Factory/Retention/Test statt erfundenem Callable. | Parserfacts sind JSON-Text und kein Verdict; alte Testbelege nicht als neuer Lauf ausgegeben. |
| D11 | Zwei Reviewzwecke mit getrennten Eingaben/Resultaten/Completion und bestehenden Eligibilitygrenzen. | Business-/Servingkonvertierung bleibt bei vorhandenen Ownern. |
| D12 | Offene Connections-Regel auf gemeinsame Abstraktionen begrenzt; legitime fehlende Einzelschnittstelle zulässig. | Keine künstlichen Verbraucher oder neue Universal-Custodyprimitive. |

**Nachweise:** finale Linux-Teachingprüfung 29 Beispiele/132 Verhaltensfälle/sechs
Defektmutationen; echte bestehende Compiler-Evidenzfunktion; 15 CPython-Prüffälle
der zukünftigen Pure-Definitionen; Links/Whitespace/Spiegel-/Abschnittsschutzprüfung.
Keine neue Monty-/Q1-/Q2-/K7-/Deploymentabnahme. K5-Paketuptake und K6-installierte
Nachfolger sowie alle ausdrücklich zukünftigen Runtimegates bleiben getrennt offen.
Laufende andere Implementierungen behalten Auftrag und Zuständigkeit.

## 13. Erneuter Schutz abgeschlossener Abschnitte — 2026-10-29

Alle fünf vorhandenen Nachfolger stehen im vorgesehenen K4-/Remaining-Bereich.
Alte Phase-3a-/Loader-/Preload-/MCP-/Source-/Beispielabschnitte bleiben gemäß
§1.1/§6/§8 ausschließlich Bezugspassagen. Der heutige Abgleich ändert keine dieser
fünf Dateien, keine eingebetteten Inputs und keinen historischen Exact24-Beleg.

Korrigiert wurden verbliebene Anlageanweisungen in §1.1/§4.3/§9: Die Nachfolger
sind bereits geliefert und werden referenziert, nicht nochmals erstellt. Die
bloße Überschrift „Open“/„Offene“ macht ihren erledigten Definitionsanteil nicht
wieder unbearbeitet. §4.2 ordnet Owner zu, ohne fertige Teilaufgaben zu öffnen;
§6 verlangt keine Wiederholung der in §12 belegten Dokumentlieferung.
Neue Präzisierungen gehören ausschließlich zu nach aktuellem Einzelstatus noch
unbearbeiteten Folgeaufgaben. Auch seitdem abgeschlossene K4–K7- oder Unterabschnitte
bleiben erhalten; bei unklarem Status zunächst nur die konkrete offene Prüfung
beim bestehenden Owner erfassen. Keine rückwirkende K0–K3-/Abnahmeänderung.
