# Prefixless redefine at K3

**Stand:** 2026-10-31. **Status:** ausschließlich Plan; keine Code-, Runtime-,
Bestandsdokumentations- oder Aktivierungsänderung durch Erstellung dieser Datei.
Der Dateiname bezeichnet den Gesprächskontext, keinen neuen K3-Schritt und keine
Wiedereröffnung abgeschlossener K0–K3-Arbeit. Umsetzung in offenen Folgepaketen
der bestehenden Owner; vorhandene Abnahmen und laufende Aufgaben bleiben erhalten.

## 1. Ziel und neuer Auswahlvertrag

Die bisherige pauschale Prefixfreiheit der Prefix-Erstellungsarbeitsabläufe wird
für künftig qualifizierte Workflowrevisionen durch folgende **Vorausfüllung des
sichtbaren Prefix-Settings im WebUI** ersetzt:

1. Geeigneter, freigegebener Prefix-Maker vorhanden → diesen vorauswählen.
2. Andernfalls kompatibler, freigegebener Base-Prompt vorhanden → diesen vorauswählen.
3. Andernfalls → Prefixless mit den festen geprüften Arbeitsinstruktionen vorauswählen.

**Die Kette setzt ausschließlich einen sichtbaren, änderbaren Formularvorschlag.**
Sie ist keine automatische Laufzeitwahl und hängt keinen Prefix still an.
Maßgeblich ist die beim ausdrücklichen Erstellungsauftrag sichtbare Auswahl.
Die folgenden Detailregeln und alle Dateimatrizen sind in diesem Sinn umzusetzen;
„automatische Präferenz“ bezeichnet ausschließlich die Vorausfüllung.

Die Auswahl gilt für erforderliche LLM-Arbeitsaufrufe zur Prefixvorbereitung,
Konfigurationsauswertung einschließlich optionaler B1/B2-Aufrufe, Designerstellung,
Generate/Create/Regenerate/Redesign und deren erforderliche Child-/Reviewarbeit.
Deterministische Schritte bleiben deterministisch und erhalten keinen neuen
Modellaufruf. Unabhängige gewöhnliche Chats und unabhängige Reviews behalten ihren
normalen Auswahlvertrag. Ein expliziter Kandidaten-Verhaltenstest verwendet den
Kandidaten als Prüfgegenstand; er ist kein gewöhnlicher Erstellungsaufruf.

**Geeignet/freigegeben** bedeutet nicht nur gespeichert oder generiert: Die genaue
Generation ist für diese Verwendung qualifiziert und zur Nutzung freigegeben,
passt zum ausführenden Provider-/Modellpaar, dessen Design/Adapter und dem
konkreten Arbeitsauftrag und ist unter den aktuellen technischen Regeln nutzbar.
Aktivierungs- und Q1/Q2-Verträge bleiben beim bestehenden Prefixowner. Der Auftrag
erklärt weder alle Base-Prompts automatisch geeignet noch die Erstellung selbst
zur Aktivierung. Kein Reviewer-Prefix wird als zusätzliche vierte Priorität eingeführt.

Bei der initialen Vorausfüllung hat ein verwendbarer Maker Vorrang vor Base; eine
manuelle Wahl hat immer Vorrang vor diesem Vorschlag. Drafts, unfreigegebene, veraltete oder
nachweislich inkompatible Kandidaten sind nicht geeignet. Solche fachlichen
Nichtverfügbarkeiten müssen von DB-/Integritäts-/Resolverfehlern unterscheidbar
sein: technische Fehler führen zum Fehler, nicht zum stillen Weitergehen in der
Kette. Eine explizite Auswahl bleibt exakt oder fehlerhaft und nimmt nicht an der
automatischen Ersatzwahl teil. Kein Durchprobieren älterer Generationen als
versteckte Ausweichwahl; bestehende aktive, kohärente Katalogauswahl verwenden.

Beim Absenden wird die sichtbare Auswahl serverseitig gegen den kohärenten
Bestand geprüft und vor dem ersten betroffenen Modelleffekt exakt erfasst.
Vorausfüllgrund und tatsächliche Betreiberwahl werden getrennt behalten. Ein späterer
Dispatch-, Mess-, Policy- oder Fitfehler löst keine automatische neue Auswahl,
keinen Ersatzattempt mit anderem Prefix und keinen Tier-2-Fallback aus. Reguläre
Retries bleiben ausschließlich nach dem bestehenden Outcome-/Effekt-/Attemptvertrag
mit derselben erfassten Auswahl zulässig. Insbesondere ein
übergroßer tatsächlicher Request wird blockiert, nicht nachträglich auf Base oder
prefixfrei umgestellt. Keine neue Kürzungs- oder Reservekonstante einführen.

### 1.1 Sichtbares, vor Generate editierbares Setting

- Vor „Generate“ steht ein beschriftetes Feld „Prefix für diesen Erstellungsaufruf“.
  Es bietet **Prefixless** und die freigegebenen, für das ausführende Paar und den
  Arbeitsauftrag kompatiblen Prefixe an, einschließlich anderer Profile als
  Maker/Base. Die Kette bestimmt nur die initiale Vorauswahl, nicht die Optionsliste.
- Eine manuelle Änderung wird von Refresh, Rollen-/Katalognotifications oder
  erneutem Laden der Vorschlagsdaten nicht überschrieben. Bei Paarwechsel die
  Auswahl neu validieren; eine nun ungültige manuelle Wahl sichtbar beanstanden,
  nicht still durch einen neuen Vorschlag ersetzen. Neue Vorschläge ausdrücklich
  zur Übernahme anbieten. Kein weiteres Bestätigungsmodal vor Generate nötig.
- „Generate“ sendet genau den sichtbaren Wert mit dem ursprünglichen Auftrag.
  Der Server prüft Freigabe/Kompatibilität erneut und pinnt die genaue Generation.
  Eine zwischen Anzeige und Absenden geänderte Generation/Revision wird als
  sichtbarer Konflikt mit erforderlicher erneuter Auswahl behandelt, nicht still
  durch latest ersetzt. Nach Admission ist die Wahl für den Job unveränderlich.
- Prefixless ist auch bei vorhandenem Maker/Base manuell wählbar. Das ist ein
  ausdrücklicher Auftrag ohne generierten Prefix, kein Default- oder Fehlerfall.
  Feste Instruktionen und technische Guards bleiben vollständig wirksam.
- Dieselbe Bediensemantik gilt vor Create/Redesign/Regenerate, soweit diese einen
  Modellauftrag auslösen. Kein Hintergrund-Use-/B1/B2-Aufruf darf vor dieser
  sichtbaren Auswahlentscheidung einen Maker/Base allein aufgrund des Vorschlags
  übernehmen. Bestehende bereits zugelassene Hintergrundjobs behalten ihren Vertrag;
  die neuen Vorbereitungsabläufe müssen die Auswahl vor ihrer Admission verfügbar
  machen. Fehlende Oberfläche/Admission ist ein offener Owneranschluss.
- API-/CLI-Aufträge geben die Auswahl ausdrücklich über den passenden bestehenden
  bzw. kompatibel erweiterten Vertrag mit. Keine zusätzliche automatische
  Maker/Base-Laufzeitkette für headless Aufträge aus diesem UIvertrag ableiten.

### 1.2 Entsprechende Prefixeinstellung für Recipes

Recipes erhalten im vorhandenen Recipe-Editor dieselbe Auswahlmöglichkeit:
Prefixless oder ein freigegebener kompatibler Prefix. **Kein Generate-Button für
Recipes.** Der Wert gehört zur gespeicherten Recipe-/Variantenkonfiguration und
deren normalem Review-/Freigabepfad; er wird vor der Ausführung durch den bestehenden
Auswahlowner gebunden. Ein anderes Setting pro Step wird nicht erfunden.
Ein vorhandener Default bleibt als bisherige explizit beschriftete Option erhalten;
Default und Prefixless sind verschiedene Werte. Die Maker/Base-Vorausfüllregel
gilt für Erstellungsformulare, nicht pauschal für alle neuen oder alten Recipes.

**Sourcebefund:** `recipe_model_input.rs` und `recipe.md` beschreiben bereits
Default/Selected sowie qualifizierte PrefixFreeWorkflow-Deklarationen. Das ist
noch kein Nachweis eines sichtbaren Recipe-Prefixfelds oder allgemeinen
Prefixless-Runnersupports. Im geprüften WebUI wurde kein solches dediziertes Feld
gefunden; `component-submission-panel.js` bietet generische JSONeinreichung.
Deshalb bei Umsetzung vorhandenen Recipe-Editor/Consumer erneut lokalisieren und
das Setting dort wiederverwenden oder gezielt ergänzen. Allgemeines Recipe-
Prefixless nicht durch eine erfundene Bootstrap-Purpose oder bloßes Weglassen
des Felds darstellen; fehlende Unterstützung im bestehenden Owner qualifizieren.

Bei erforderlichen Child-/Reviewaufrufen gehört die sichtbare Wahl zum jeweils
ausführenden Paar. Andere Paare brauchen vor Admission eine sichtbare passende
Auswahl bzw. gespeicherte qualifizierte Recipekonfiguration. Die vorausgefüllte
Kette wird nicht heimlich im Child wiederholt; kein Parentprefix auf andere Paare
übertragen. Native Bindungen und vorhandene unabhängige Reviewerrollen bleiben
erhalten, bis ihr explizit geplanter Nachfolger vollständig unterstützt ist.

## 2. Schutz, Zuständigkeit und Ausgangsbefund

Maßgeblich sind [Entwicklungspolicy](docs/development-policy.md),
[Implementierungsindex](docs/implementation-status.md), die vier Komponentenguides
und die Schutzregeln in [Exact K3-2 §§1.2–1.3](docs/plans/Exact_V3_definition_update_at_K3-2.md).
Dieser Plan löst für zukünftige Arbeit nur die widersprechende Pflicht zur
generellen Prefixfreiheit sowie die Verschiebung jedes Makeranschlusses auf v3.5
ab. Sonstige Verträge bleiben wirksam, insbesondere Daten-/Code-Trennung,
Quellenprüfung, Approval, ursprüngliche Modell-/Authbindung und vollständiger Guard.

Der bisherige Exact-Plan §5.1 fordert bereits eine dokumentierte Erweiterungsstelle
im bestehenden Requestowner. Die vorausgehende Sourceprüfung fand keinen
entsprechenden konkreten Codeverweis. Allgemeine Default-/Selected-/PrefixFree-
Strukturen und Capture-/Encodingbausteine existieren, beweisen aber keinen
qualifizierten durchgängigen Erstellungsworkflow. Der native Resolver in
`crates/brassclaw_reborn_composition/src/task_model_provider.rs` weist den
PrefixFreeWorkflow-Zweig derzeit zurück. Bei Umsetzung erneut gegen den dann
aktuellen Baum prüfen; keine vorhandene Infrastruktur nochmals beauftragen.

### 2.1 Auditbefunde und verbindliche Integrationsgrenzen

Sourceprüfung vom 2026-10-31, keine Ausführungsabnahme:

| Aktueller Code | Konsequenz für die nachfolgenden Planänderungen |
| --- | --- |
| `crates/brassclaw_skills/src/recipe_model_input.rs`: striktes `recipe-model-input-bindings/1`, nur Default/Selected/PrefixFreeWorkflow; Selected nennt ein Profil, keine Generation | Die UIvorausfüllung benötigt kein neues Runtimepräferenztag. Bestehende Selected-/Default-Deklarationen wiederverwenden; fehlende Darstellung der ausdrücklichen Prefixless-Wahl und Revisionsbindung nach vollständiger Consumerinventur kompatibel ergänzen. Keine neuen Tags in alte strikte DTOs schreiben. |
| `crates/brassclaw_reborn_composition/src/task_model_capture.rs` und `task_model_capture_record.rs`: Capture stammt nur aus einer echten PrefixFreeWorkflow-Deklaration; `TaskSelection::capture` in `task_model_provider.rs` lässt dann die Prefixbindings leer | Abgesendete UI-/gespeicherte Recipewahl braucht ihren nachgewiesenen Workflowbezug im bestehenden Captureowner. Alten Free-Capture nicht umdeuten oder als Auswahlkontext mit Prefix verwenden. Explizites Prefixless ist auch bei vorhandenen Prefixen erlaubt und benötigt keine nachgewiesene Abwesenheit von Maker/Base. |
| `task_model_provider.rs`: Provideraufnahme und `PgPrefixAcceptances::bindings()` sind getrennte Reads; Parenthand-off gibt dasselbe Selectionobjekt zurück und prüft einen vorhandenen Capturekontext exakt | Kohärenz vor Aktivierung tatsächlich herstellen bzw. revisionsgeprüft nachweisen. Kein neuer Child-Capture darf an legitimer Parentweitergabe scheitern oder deren Herkunftsprüfung umgehen. Workflowfamilie, genaue Recipe-/Occurrencebindung und pro-Paar-Auswahlergebnis unterscheiden. |
| `task_model_provider.rs`: vorbereitete Clients/Guards werden rollenweise in `Prepared.roles` gehalten; `original_prefix`, `preflight_compatibility` und `prepare_final_request` erwarten eine echte Prefixqualifikation | Gemeinsame Clients sind keine globale Prefixauswahl. Prüfen, ob der Cache weiterhin nur routenbezogene Daten hält; anfrage-/prefixabhängige Guards passend binden oder Cacheidentität kompatibel erweitern. Insbesondere normale und Kandidatentestaufrufe dürfen bei gleicher Rolle keinen falschen Guard teilen. |
| `crates/brassclaw_reborn_composition/src/pg_monty_model_response.rs`: historischer Evidencewriter weist PrefixFreeWorkflow zurück; `final_model_encoding.rs` hat bereits einen separaten prefixfreien Messzweig | Vorhandenen Encoder wiederverwenden, aber Response-/Review-/Wirebelege und Guard im selben Datenfluss anschließen. Struktureller Messsupport allein aktiviert den dritten Zweig nicht; alte `/1`-Belege behalten ihre Bedeutung. |
| `crates/brassclaw_reborn_composition/src/prefix_acceptances.rs`: aktive Targetbindings und getrennte Pair-/Profile-/Generationqualifikationen | Nur die aktive veröffentlichte Auswahl pro Namespace/Provider/Modell/Profil prüfen, nicht neueste gespeicherte oder beliebige frühere Generationen suchen. Freigabe, Aktivierung und aktuelle technische Nutzbarkeit getrennt ausweisen. |

**Kein neues pauschales Approvalgate:** Eignung im Erstellungsworkflow wird durch
Qualifikation des geänderten Consumers und seiner Zusammensetzung mit vorhandenen
Prefixverträgen nachgewiesen. Unveränderte freigegebene Prefixe oder Komponenten
erhalten dadurch nicht automatisch neue Q1/Q2-Pflichten pro Job. Tatsächlich
geänderte Prefixartefakte und Workflowrevisionen folgen ihrem bestehenden Vertrag.
Existiert das aktive Artefakt, aber der erforderliche Auswahl-/Reader-/Guardpfad
noch nicht, ist das fehlende Implementierung: nicht als „kein geeigneter Prefix“
deklarieren, um dauerhaft den dritten Zweig zu aktivieren. Neue Workflowrevisionen
erst nach Qualifikation aller von ihnen beanspruchten Zweige aktivieren.

**Kein globaler Frischezwang:** „Veraltet“ bezieht sich auf die nachgewiesene
Kompatibilität für neue Verwendung, nicht schon auf eine neuere Draft-/Quellrevision.
Gültige Altartefakte und gepinnte Jobs bleiben nach den bestehenden Verträgen
nutzbar; Livepolicy und harte technische Grenzen werden weiterhin geprüft.

Die folgende Dateiliste unterscheidet notwendige Vertragsänderungen,
zu prüfende Anschlussdokumente und unveränderliche historische Belege. Keine
pauschale Textersetzung von „prefixfrei“: Der Ausdruck bleibt für den dritten
Auswahlzweig, gezielte Tests und historische Nachweise richtig.

### 2.2 Änderungsgrenze: ausschließlich hinter dem aktuellen Arbeitsstand

**Verbindlich für alle Pläne, nicht nur die fünf besonders geschützten Dateien:**
Dieser Auftrag schreibt keine vorhandenen Implementierungsschritte um. Weder
abgenommene noch gerade implementierte, zur Integration eingereichte oder bereits
in Prüfung befindliche Teilpakete erhalten rückwirkend andere Anforderungen.
Die neue Regel steht in einem zusätzlichen offenen Folgepaket hinter dem jeweils
aktuellen Implementierungs-/Integrationsschritt des zuständigen Owners. Vorhandene
Schrittnummern, Reihenfolge, Fortschrittsmarker und Abnahmekriterien bleiben erhalten.
Ein als „offen“ bezeichneter Gesamtabschnitt ist kein freier Änderungsbereich.

**Geprüfte Ausgangslage am 2026-10-31:**

| Owner / Planbezug | Aktuelle Grenze und zulässiger Anschluss |
| --- | --- |
| Root/Auth, `simplified_v3.md` und zentrale Queue | K3 ist weiter offen. BUILD-024/025 sind scoped abgenommen; BUILD-026 ist als Implementing eingetragen. Keinen dieser Schritte, ihren K3-Unterauftrag oder ihre Abnahme ändern. Neue Prefixanforderungen ausschließlich als künftigen Folgeauftrag ab K4 beschreiben; keine neue Voraussetzung für den Abschluss dieser aktuellen Reparaturen. |
| Exact K3-2 | §§17.1–17.20 enthalten bereits gelieferte bzw. scoped abgenommene Voraussetzungen; die vorhandene I1/I2-Integration ist weiter offen. BUILD-002 ist Proposed, BUILD-023 zur unabhängigen I3-Designabnahme ebenfalls Proposed. Diese Aufträge/Proposals nicht umdefinieren oder neue Auswahlfälle in ihre alte Abnahme einschieben. Einen zusätzlichen Nachfolger nach dem dann aktuellen Integrationspaket planen und kompatibel an dessen Ergebnisse anschließen. |
| Prefix, Composition und Komponentenimplementierung | Bereits qualifizierte Pair/Profile/Generation-, Capture-, Modellhost-, Encoding-, Preload- und native Childbausteine wiederverwenden. Nur neue Auswahl-/Zusammensetzungsdeltas im angehängten Folgepaket; kein Wiederöffnen alter L-/R-/V35-/K4-Abschnitte oder alter K3-2-Folgeaufträge. |
| Validator | BUILD-018/019 und weitere vorhandene Subjekt-/Checkerabnahmen bleiben scoped erhalten; allgemeine Q1/Q2-/Pure-/S-Arbeit ist weiter offen. Den aktuell bearbeiteten oder bereits eingereichten Schritt nicht auf neue Prefixpräferenzen umstellen. Nur nachgelagerte erforderliche Reviewconsumer an den neuen Vertrag anschließen. |
| Connections, Sempai, Recipeplan und Roadmap | Jeweilige aktuellen Teilpakete vor Ausführung neu feststellen; ungeklärter Status berechtigt zu keinem Eingriff. Neuer offener Anhang mit konkreter Anschlusskante statt Änderung vorhandener Steps oder Beispiele, die als akzeptierter Planteil dienen. |

Queuezustände und Abschnittsenden sind Momentaufnahmen, keine dauerhaft
festgeschriebenen Schrittgrenzen. Vor jedem späteren Dokumentpatch anhand von
Statusindex, aktuellem Handoff, Queue und zugehörigen Receipts neu bestimmen:
Owner, aktuelles Paket, abgenommene Teilgrenzen und konkreter offener Nachfolger.
Diese Zuordnung in der Dateimatrix dieses Plans fortschreiben. Fehlt ein belastbarer
Stand, nur einen nachgelagerten offenen Anschluss beschreiben; keine alte Arbeit
neu klassifizieren. Die führende Integration legt den Ausführungszeitpunkt nach
dem betreffenden laufenden Paket fest. Das ist keine Pflicht, sämtliche früheren
Pläne erst vollständig zu beenden, und kein neues globales Freigabegate.

### 2.3 Widerspruchsfreiheit aktueller Doku und Architektur

Planschutz und aktuelle Vertragsdokumentation sind unterschiedlich zu behandeln:

1. **Pläne:** Bestehende Abschnitte unverändert lassen und einen neuen offenen
   Nachfolger anhängen. Darin alle abgelösten Aussagen mit Dateipfad,
   Überschriftenkette und charakteristischem Satz konkret benennen. Für die neue
   Workflowrevision gilt der neue Vertrag; frühere Auswahlverträge und damalige
   Nachweise bleiben ihrem alten Scope zugeordnet. Kein unbestimmtes „neuere
   Regel hat Vorrang“ ohne genaue Ablösungszuordnung.
2. **Aktuelle normative Doku/Architektur:** Nicht nur einen weiteren widersprechenden
   Absatz ans Ende hängen. Alle betroffenen ungeschützten aktiven Aussagen,
   Zusammenfassungen, Tabellen, Beispiele, Diagramme und Bedienhinweise unmittelbar
   auf denselben neuen Zielvertrag ausrichten oder durch einen eindeutigen
   Contractverweis ersetzen. Gleichzeitig den tatsächlichen Implementierungsstand
   separat beschreiben: Sollvertrag geändert bedeutet nicht Runtime aktiviert.
3. **Geschützte Dokuaussagen/Records:** Historische oder abgenommene Bytes bleiben
   unverändert. Ihre Geltungsgrenze im neuen, außerhalb des geschützten Abschnitts
   liegenden Nachfolger ausdrücklich dokumentieren. Aktuelle Navigation aus
   Statusindex, aktuellen Guides und sonstigen veränderbaren Einstiegspunkten
   führt direkt zum maßgeblichen Nachfolger; alte Deep-Links bleiben nutzbar und
   sind im Nachfolger als alte Vertragsfassung eingeordnet. Keine nachträglichen
   Marker in die geschützten Originalabschnitte einschieben.
4. **Ein gemeinsamer Vertrag:** `docs/reborn/model-token-limits.md` ist der
   Vertragsowner; dieser Plan ist Änderungsauftrag und Inventar. Die drei
   Auswahlregeln dürfen in Übersichten zusammengefasst, aber nicht mit anderen
   Voraussetzungen oder Fehlersemantiken neu definiert werden. „Immer prefixfrei“,
   „Maker zwingend vorhanden“, „Base automatisch in jedem Chat“ und „Maker nur
   v3.5“ dürfen für neue Erstellungsworkflows nirgends parallel aktiv gelten.

Abnahme der Textangleichung verlangt damit Widerspruchsfreiheit der **aktuell
geltenden Aussagen**, nicht das Löschen abweichender historischer Belege. Bei
unklarer Schutzgrenze keine behauptete Konsistenz: exakte Fundstelle als offenen
Dokumentationspunkt führen, bis ihre gültige Einordnung nachgewiesen ist.

## 3. Step 1 — Fundstellen und Schutzstatus einfrieren

- [ ] Vor der Dokumentangleichung aktuellen Status, laufende Ownerarbeit und
  vorhandene Diffs lesen. Fremde Änderungen weder überschreiben noch zurücksetzen.
- [ ] Für jede Plandatei die Grenze aus §2.2 explizit festhalten; Textänderungen
  nur im zusätzlich angelegten Folgepaket hinter dieser Grenze. Bereits
  vorgeschlagene/integrierte neue Steps nicht still auf die neue Regel umschreiben.
- [ ] Für jede Fundstelle Pfad, Überschriftenkette und charakteristischen Satz
  festhalten; Zeilennummern sind nur Suchhilfen. Die Dateimatrix in §§4–8 im selben
  Plan aktualisieren, keine zweite Trackingdatei erzeugen.
- [ ] Vollständigen Suchlauf über aktuelle Markdown-/MDX-/Text-/Architekturdateien
  durchführen: `prefixfrei`, `prefix-free`, `prefixless`, `PrefixFreeWorkflow`,
  `Prefix-Maker`, `kein Prefixlookup`, `no accepted-prefix lookup`,
  `v3.5` sowie englische Pflichtformulierungen zu Base-/Makerverwendung.
  Verlinkte Dokumente und eingebettete Authoringreferenzen zusätzlich verfolgen.
- [ ] Alte Nachweise, Snapshots, generierte Pakete und abgeschlossene Abschnitte
  als geschützt erfassen. Die fünf Dateien aus Exact §1.3 ausschließlich durch
  jeweils einen neuen offenen Nachfolger ergänzen, einschließlich ihrer bereits
  existierenden K3-2-Folgeabschnitte; dort nichts rückwirkend umschreiben.

**Ergebnis:** vollständiges aktuelles Fundstelleninventar mit begründeten
Änderungs-/Schutzentscheidungen und aktuellen Ownergrenzen, bevor die nachfolgenden
Textpatches beginnen. Die Matrixtexte unten sind fachliche Änderungsziele, niemals
eine Erlaubnis zum Überschreiben vorhandener Implementierungsschritte.

### 3.1 Ausführung und überprüfbares Ergebnis jedes Dokumentationssteps

Die Schritte dieses Plans sind **Dokumentationsarbeit**. Ihre Ergebnisse sind
Textpatches und präzise nachgelagerte Owneraufträge, nicht bereits implementierte
Resolver oder bestandene Runtimefälle. Die Reihenfolge ist 1 → 2 → 3 → 4 → 5 → 6 → 7;
bei einem unklaren Vertragsdetail Step 2 berichtigen und betroffene Consumertexte
nachführen, statt unterschiedliche lokale Lösungen zu dokumentieren.

| Step | Konkrete Ausführung | Fertig, wenn |
| --- | --- | --- |
| 1 — Inventar | Für jeden Treffer in den Dateimatrizen notieren: Überschriftenkette, alter charakteristischer Satz, Schutzstatus, Owner, aktuelles Paket, erlaubtes neues Textziel und Entscheidung ändern/nachfolgen/unverändert. Bei „unverändert“ den fachlichen oder historischen Scope begründen. Suchtreffer ohne neuen Änderungsbedarf sind ebenfalls zuordnen. | Alle bekannten Dateien und neu entdeckten aktiven Treffer sind eingeordnet; kein Schutzstatus wird aus dem Wort „offen“ abgeleitet. |
| 2 — Gemeinsamer Vertrag | Im gemeinsamen Owner einen neuen eindeutig betitelten Abschnitt „Prefix-Erstellungsaufrufe — Maker/Base/prefixfrei“ mit stabil nutzbarem Anker erstellen. Auswahlfolge, Eignung, Fehler, Instruktionszusammensetzung, Erfassungszeitpunkt, Child-/Reviewregel und Altjobgrenze aus §§1/4 vollständig festlegen. Im Exact-Nachfolger die konkrete Ablösungstabelle aus §2.3 anlegen. | Eine autoritative Stelle beantwortet jeden Fall der Auswahlentscheidung; alle Consumer können auf denselben Anker verweisen. Neue Zustands-/Formatnamen erscheinen nur als Zielentwurf, solange kein passender Reader existiert. |
| 3 — Ownerpläne | In jeder Plandatei aus §5 einen neuen Folgeabschnitt nach §2.2 anlegen. Darin Pflichtgliederung aus §5.1 verwenden und den eigenen Anteil an der gemeinsamen Regel benennen. Fachliche und technische Aufgaben zwischen Prefix, Composition und Validator einmalig zuordnen. | Jede erforderliche Verantwortung hat genau einen Implementierungsowner; jeder Consumer nennt seinen Anschluss und seine Voraussetzungen. Alte Steps und ihre Abnahmen sind unverändert. |
| 4 — Doku/Architektur | Pro Datei aus §6 sämtliche aktiven Vorkommen prüfen, nicht nur den ersten Treffer. Ungeschützte widersprechende Zieltexte direkt korrigieren; geschützte Befunde gemäß §2.3 einordnen. Kurzfassungen, Diagramme und Beispiele im selben Patch nachführen. Bestehende unterstützte `/1`-JSONbeispiele nicht mit unimplementierten Tags erweitern. | Keine zwei für denselben neuen Workflow geltenden Aussagen verlangen unterschiedliche Prefixwahl. Tatsächlicher Support bleibt sichtbar getrennt vom neuen Sollvertrag. |
| 5 — Status/Referenzen | Statusindex und zulässige Handoffnavigation auf den neuen gemeinsamen Vertragsanker und die konkreten neuen Ownerabschnitte verlinken. Betroffene eingebettete Referenzen samt vorhandenem Integritätsowner aufführen; spätere Paketlieferung als offenen Anschluss markieren. | Der neue Auftrag ist auffindbar, alte Receipts und Paketbytes sind unangetastet und keine installierte Übernahme wird behauptet. |
| 6 — Abnahmezuordnung | Jede Zeile aus §8 im neuen zuständigen Ownerabschnitt referenzieren und erforderlichen echten Caller sowie beobachtetes Ergebnis angeben. Gemeinsame Fälle nur einmal führen; andere Owner verweisen darauf. Noch fehlende Caller ausdrücklich als Implementierungsbedarf benennen. | Kein Fall ist bloß eine abstrakte Testforderung ohne Owner und Beobachtung; neue Anforderungen sind nicht in alte Abnahmematrizen eingeschoben. |
| 7 — Abschluss | Inventar erneut gegen Suchlauf und finalen Diff abgleichen, alle lokalen Links/Anker prüfen und Append-only-Schutz gegen den erfassten Dateistand kontrollieren. Ergebnis und verbleibende Runtimeanschlüsse kurz in diesem Plan dokumentieren. | Alle Treffer sind begründet erledigt oder als konkrete offene Blockade ausgewiesen; Dokumentabschluss wird bei einer ungelösten aktiven Normenkollision nicht behauptet. |

Für Step 1 diese **Spalten im vorhandenen Inventar** verwenden: Datei ·
Überschriftenkette/Altsatz · Schutzstatus · aktuelles Ownerpaket · neues Textziel ·
Aktion · Abschlussbeleg. Keine neue Registry oder zusätzliche Reportdatei anlegen.
Die Details können unter der jeweiligen Matrixzeile stehen, wenn eine zusätzliche
breite Tabelle die Lesbarkeit verschlechtert.

## 4. Step 2 — Einen gemeinsamen Nachfolgevertrag formulieren

| Datei | Fundstelle / erforderliche Änderung |
| --- | --- |
| [docs/reborn/model-token-limits.md](docs/reborn/model-token-limits.md) | Neuer maßgeblicher Nachfolger zu „Open K3-2 successor — retained prefix selection and prefix-free construction workflows“: Auswahlkette aus §1, Eignung, Fehlerabgrenzung, Erfassungszeitpunkt, Child-/Reviewweitergabe und vollständige Messung definieren. Offene Aussagen „even when other prefixes exist“, „Optional model work remains prefix-free“ und „only the v3.5 idea“ nach Schutzprüfung ablösen oder durch lokalen Nachfolger klar eingrenzen. Alten Abschnittsanker für bestehende Links erhalten. |
| [docs/plans/Exact_V3_definition_update_at_K3-2.md](docs/plans/Exact_V3_definition_update_at_K3-2.md) | Expliziten datierten offenen Nachfolger für §1.1/P02/P03, §1.7, §§4–5.2, Datei-/Integrationsmatrizen, Arbeitsaufträge I1/I2 und Abnahmematrix anlegen. Die generelle Prefixfreiheit samt rekursiver Vererbung durch gemeinsame Auswahlregel ersetzen; §5.1 ist künftig Integrationsauftrag statt bloßer v3.5-Vormerkung. Frühere Betreiberentscheidung und abgenommene Fortschrittsabschnitte unverändert lassen. |
| [docs/plans/Exact_V3_definition_update_at_K3.md](docs/plans/Exact_V3_definition_update_at_K3.md) | Als Vorgänger und Schutzreferenz prüfen; soweit aktive Leser daraus Erstellungsanforderungen beziehen, neuen offenen Querverweis auf diesen Nachfolger und den gemeinsamen Vertrag ergänzen. Keine abgeschlossenen K3-Abschnitte oder damaligen Kriterien ändern. |

Im gemeinsamen Vertrag folgende Punkte eindeutig festlegen:

- [ ] Sichtbare Vorauswahl, manuelle Betreiberwahl und retained Ergebnis trennen.
  Für die UIvorausfüllung ist kein neues automatisches Runtime-Auswahltag nötig.
  Vorhandene Selected-/Default-Verträge verwenden, wo passend; explizites
  Prefixless und die vollständige Formular-/Job-/Recipeweitergabe nur bei
  tatsächlichem Bedarf kompatibel erweitern. Kein neuer Tag in alten strikten
  DTOs und keine Umdeutung von PrefixFreeWorkflow als allgemeines Recipe-Prefixless.
  UI hält mindestens Vorschlag/Begründung, aktuellen Wert, manuell-geändert-Status
  und angezeigten Revisionsbezug auseinander. Server vertraut nicht dem UIlabel,
  sondern prüft den abgesendeten konkreten Wert und dessen Herkunft/Kompatibilität.
- [ ] Pro bereits erfasstem ausführendem Paar auswählen. Designziel und
  Designausführer sind getrennt; ein Child mit anderem Paar bekommt keinen
  Parentprefix. Erforderliche Unterabläufe übernehmen die sichtbar beauftragte bzw. gespeicherte
  Auswahl für ihre eigene Route, nicht die Vorausfüllautomatik oder pauschal dieselben Prefixbytes. Ergebnisse für bekannte Routen vor Effekten
  erfassen; später zulässig zugelassene Childs nur aus dem retained Katalog und
  ihrer ursprünglichen Route vor ihrem ersten Effekt binden, nie global latest.
  Auflösungsidentität umfasst mindestens Workflowrevision/-zweck, ursprüngliche
  Ausführerroute und Instruktions-/Zusammensetzungsvertrag: gleiche Paare können
  in verschiedenen Schritten unterschiedliche Eignungsanforderungen haben.
  Bestehende Variantengranularität nicht durch erfundene pro-Step-JSONfelder
  umgehen; fehlende interne Occurrencebindung im bestehenden Owner ergänzen.
- [ ] Kein neues Modell oder Authmaterial aus der Prefixwahl ableiten.
  Inherited absence bleibt Abwesenheit. Gleiche relevante retained Auswahl darf
  in späteren Schritten nicht erneut nach Maker/Base suchen.
- [ ] Ersterstellung bleibt ohne vorhandenes Artefakt möglich. Bei Erzeugung
  eines Maker-Nachfolgers darf eine geeignete frühere freigegebene Generation
  Arbeitskontext sein; der neue Kandidat genehmigt oder bewertet sich nicht selbst.
- [ ] Feste fachliche Instruktionen und strikte Antwort-/Quellenverträge bleiben
  in allen Zweigen wirksam. Base ergänzt sie und ersetzt sie nicht pauschal.
  Maker kann nur ausdrücklich qualifizierte Instruktionsabschnitte ersetzen;
  keine doppelte Einfügung, kein doppeltes Framing, keine zweite Formatierungsrunde.
  Ohne einen solchen Ersatzvertrag bleiben die festen Instruktionen vollständig
  vorhanden. Für Base und Maker die System-/Dynamikrollen und Instruktionspriorität
  am tatsächlich unterstützten Adapter prüfen. Ein widersprechender Prefix wird
  nicht allein durch angehängtes „ignoriere frühere Regeln“ kompatibel gemacht.
- [ ] B1/B2 weiter auf das gelieferte Quellenpacket beschränken. Wissen aus Base
  oder Maker ist keine zusätzliche Providerquelle und darf unbekannte Fakten
  nicht auffüllen. Falls ein Prefix diesen Vertrag nicht einhalten kann, ist er
  für diesen Aufruf ungeeignet. Payloadhash bleibt Payloadhash; vollständige
  Requestidentität bindet zusätzlich Instruktionen und ausgewählte Generation.
- [ ] Bootstrapauswahl nicht von noch zu erzeugendem Paardesign abhängig machen:
  Ausführung braucht eine technisch belegte Route samt Encoder/Protokoll auch
  prefixfrei, aber keinen generierten Maker/Base als Voraussetzung. Existierende
  Prefixe müssen zum tatsächlich verwendeten akzeptierten Ausführerdesign passen;
  ein neues Ziel-/Designproposal ersetzt diese Bindung nicht. Ist ohne passende
  Ausführergrundlagen kein sicherer Request möglich, konkrete Readiness melden;
  keinen ungeprüften Template-/Nicht-Task-Fallback verwenden.
- [ ] Bei der ursprünglichen UIvorausfüllung sind Maker-/Baselookups
  erforderlich; bei Admission wird die sichtbare Wahl geprüft, nicht die Kette erneut ausgeführt. Im danach erfassten prefixfreien Ausführungszweig findet kein
  erneuter Accepted-Prefixlookup statt. Die alte pauschale Zusicherung „kein
  Lookup während des gesamten Workflows“ ist für den neuen Vertrag falsch.
  Namespace, aktive Targetbindingrevisionen, genaue Qualifikationen und belastbare
  Auswahl-/Nichtverfügbarkeitsgründe im selben Capturebezug festhalten, ohne
  zusätzliche serviceweite Registry. Fremde Namespaces sind keine Kandidaten.
- [ ] Tatsächlichen Gesamtrequest einschließlich Prefix, Quellen, History,
  Tools, Framing und Outputvorgaben messen. Harte Prefix-/Prompt-/Kontextgrenzen,
  Livepolicy, Cancellation und Attemptgrenzen in allen drei Zweigen erhalten.
- [ ] Laufende, wartende und wiederaufgenommene Altjobs behalten ihre echte
  bisherige Auswahl. Keine Umklassifizierung aus Namen oder fehlenden Feldern;
  neue UI-/Auswahlsemantik gilt erst für neue qualifizierte Workflowrevisionen.
  Exakte Prefixpayloads, Design-/Adapterreferenzen und Qualifikationen über
  Wait/Resume/Child erhalten und vor GC schützen. Die neue Nutzung als
  Arbeitskontext ist eine echte lebende Referenz, auch jenseits des regulären
  Zweivorgängerarchivs; keine unreferenzierte Kopie nur im VMheap.

## 5. Step 3 — Bestehende Ownerpläne angleichen

Diese Dateien konsumieren den gemeinsamen Vertrag, statt je einen eigenen
Auswahlalgorithmus zu definieren. In allen folgenden Plänen ausschließlich
neue offene Abschnitte „Prefixless redefine — Maker/Base/prefixfrei“ ergänzen,
gemäß §2.2 hinter der aktuellen Implementierungsgrenze. Alte Steps und vorhandene
K3-2-Folgeabschnitte bleiben auch dann unverändert, wenn die Matrix „angleichen“,
„ablösen“ oder „nicht länger“ formuliert; das erfolgt nur im neuen Nachfolger.

| Datei | Erforderlicher offener Folgeauftrag |
| --- | --- |
| [docs/plans/prefix_v3_upgrade.md](docs/plans/prefix_v3_upgrade.md) | Fachlicher Prefixowner: Eignungs-/Freigabeprüfung und Auswahlkette, ursprüngliche Use-/Generationjobs, Bootstrap-B1/B2, Create/Redesign, Bereitschaftsanzeige und Auswahlbelege. Im neuen Nachfolger die bisherigen Abnahmezeilen „Qualified PrefixFreeWorkflow“ und „no accepted-prefix lookup“ auf die Ausführung nach expliziter Prefixless-Wahl begrenzen; nur die UIvorausfüllung benötigt die Maker/Base-Kette. Sichtbares editierbares Setting und verbindliche Übernahme bei Generate planen. Alle drei Zweige und Retention/GC abnehmen. Maker nicht länger ausschließlich v3.5-Vormerkung. |
| [docs/plans/monty.composition.md](docs/plans/monty.composition.md) | Workflow-/Compositionowner: Recipeentwurf mit wiederverwendeten Komponenten, Auswahlregel und genaue retained Ergebnisse, ursprüngliche Modell-/Authrouten, rekursive Child-/Reviewkanten. Keine neue Rust-Workflowprimitive und kein zweiter Client. |
| [docs/plans/monty.composition.implementation.md](docs/plans/monty.composition.implementation.md) | Vorhandene Resolver-, Capture-, Gateway-, Response-, Encoder- und Guardconsumer als einen verbundenen Implementierungsauftrag zuordnen. Dokumentierten Einfügepunkt konkret benennen; kein wirkungsloser Platzhalterpfad. Reader vor neuen Writern/Seeds qualifizieren. |
| [simplified_v3.md](simplified_v3.md) | Neuer offener Nachfolger zum K3-2-Folgeauftrag: automatische Präferenz von ihrer erfassten Auswahl trennen, Lifecycle-/Wait-/Resume-/Childvertrag und ursprüngliche Provider-/Authbindung für alle drei Zweige festhalten. Keine Änderung an K0–K3, bisherigen Phasen oder alten Nachweisen. |
| [validator_v3.md](validator_v3.md) | Neuer offener Reviewnachfolger: erforderliche Reviews übernehmen die Regel pro ausführendem Paar statt stets PrefixFreeWorkflow. Echte Kandidatentests und unabhängige normale Reviews abgrenzen; Prefix-Q1/Q2, Komponentenapproval und Reviewunabhängigkeit erhalten. |
| [docs/plans/monty.composition.recipes.md](docs/plans/monty.composition.recipes.md) | Workflow-/Variantenanforderungen und Beispielerläuterungen angleichen. PrefixFreeWorkflow nicht mehr als pauschale Deklaration jeder Generierungsvariante empfehlen. Keine erfundenen Persistenzfelder oder versteckten pro-Step-Overrides. |
| [docs/plans/simplify_connections_v3.5.md](docs/plans/simplify_connections_v3.5.md) | Use-/Discovery-/Readiness-Anschluss auf neue Regel verweisen lassen. UI zeigt tatsächliche Maker-/Base-/prefixfreie Auswahl; fehlender Prefix blockiert den erlaubten Bootstrap nicht. Kein zusätzlicher Use-Effekt oder unangeforderte Generierung. |
| [docs/plans/sempai_v3.5.md](docs/plans/sempai_v3.5.md) | Erforderliche Reviews im Erstellungsworkflow nutzen die Kette für ihr eigenes Paar. Normale unabhängige Sempai-Reviews behalten ihren Reviewer-Prefixvertrag. Parent-/Reviewer-/Maker-Prefixe nicht vertauschen. |

### 5.1 Pflichtinhalt der neuen Ownerabschnitte

Jeden neuen Planabschnitt in dieser Reihenfolge schreiben:

1. **Anschluss und Scope:** konkretes vorausgehendes aktuelles Ownerpaket,
   unveränderte bestehende Schritte, betroffene neue Workflowrevisionen und Link
   zum gemeinsamen Vertrag. Keine pauschale Abhängigkeit „erst gesamten Plan fertig“.
2. **Wiederverwendung und Delta:** vorhandene Stores/Resolver/Capture-/Guardpfade
   mit tatsächlichem Pfad und Symbol benennen; anschließend die fehlende Änderung
   dieses Owners beschreiben. Die Befunde in §2.1 sind Startpunkte und vor der
   endgültigen Zuordnung erneut zu prüfen, keine garantierte dauerhafte APIliste.
3. **Eingang und Ergebnis:** ursprüngliche Workflow-/Instruktions-/Paarbindung
   als Eingang; gewählte aktive Generation oder ausdrücklich gewähltes Prefixless plus
   Auswahlgrund als Ergebnis. Vertrauenswürdige Herkunft, Retention und Empfänger
   benennen. Consumer verweisen auf den gemeinsamen Producer statt ihn zu kopieren.
4. **Begrenzte Umsetzungsschritte:** beim jeweiligen Owner in der Folge
   Deklaration/Readerkompatibilität → kohärente Aufnahme/Auflösung → tatsächliche
   Request-/Child-/Reviewweitergabe → Evidence/Retention/Anzeige planen. Nur
   dessen eigenen Teil ausformulieren; fremde Teile als konkrete Anschlusskante
   referenzieren. Fehlendes Schema erst nach Consumerinventur entscheiden.
5. **Fehler und Rückwärtskompatibilität:** Abwesenheit/Ungeeignetheit von Fehlern
   unterscheiden, Altjobreader und ursprüngliche Auswahl erhalten, keine
   Aktivierung bei fehlender Runtimeunterstützung. Keine Downgrade- oder
   Effektreplaylogik als Fehlerbehandlung einführen.
6. **Abnahme und Abschluss:** zugeordnete Fälle aus §8 mit echtem Caller und
   beobachtbarem Sollresultat benennen. Source-, Dokument-, Integrations- und
   Aktivierungsabschluss getrennt melden. Writer-/Seedaktivierung folgt erst nach
   qualifiziertem verbundenem Reader-/Workflowpfad, nicht bereits nach dem Textpatch.

### 5.2 Präzise Vorausfüllung und Übernahme der Betreiberwahl

1. Beim Öffnen eines neuen Erstellungsformulars das ausführende Paar und den
   Arbeitsauftrag binden. Geeigneten aktiven Maker suchen; andernfalls geeignete
   aktive Base; andernfalls Prefixless. Das Ergebnis mit Begründung sichtbar
   vorausfüllen. Keine Generation starten und keinen Prompt verändern.
2. Zusätzlich alle anderen freigegebenen kompatiblen Prefixe als Optionen anbieten.
   Betreiber kann jede angebotene Option oder Prefixless wählen. Manuelle Wahl
   hat Vorrang vor dem Vorschlag und bleibt bis zur ausdrücklichen Änderung erhalten.
3. Fehler bei Lookup, Identität oder Integrität sichtbar melden, nicht als
   „kein Prefix vorhanden“ in eine scheinbar erfolgreiche Vorausfüllung umwandeln.
   Unabhängig davon kann eine ausdrücklich gewählte Option nur nach vollständiger
   serverseitiger Prüfung zugelassen werden; kein Fehler wird zum Auswahlbeleg.
4. Bei Generate genau den aktuellen Formularwert samt Originalauftrag und
   Revisionsbezug senden. Server prüft diese Wahl und pinnt das genaue Ergebnis
   vor Effekten. Bei stale Auswahl Konflikt zeigen, keinen anderen Prefix einsetzen.
5. Request genau nach dieser Wahl aufbauen: bei Prefixless nur feste Instruktionen,
   bei gewähltem Prefix genau dessen qualifizierte Einfügung. Livepolicy, Fit,
   Retry und Responseevidence bleiben an denselben Auftrag gebunden.
6. Recipe-Editor verwendet denselben Auswahl-/Validierungsvertrag bei normalem
   Speichern/Review und spätem Taskcapture, nicht den Generate-Ablauf. Da Recipes
   keine Generationsjobs sind, ist ihre gespeicherte Profilwahl nach bestehendem
   Vertrag beim Taskstart exakt aufzulösen; alte Taskpins bleiben unverändert.

Vorausfüllung braucht keine LLM-Entscheidung. Kein zusätzlicher Runtimepräferenz-
resolver und kein unsichtbares Fallback zwischen Maker/Base/Prefixless wird geplant.

## 6. Step 4 — Architektur- und Bedienungsdokumentation angleichen

Hier gilt die direkte Konsistenzpflege aus §2.3 für ungeschützte aktuelle Prosa.
Die Schutzprüfung erfolgt pro Abschnitt: insbesondere `recipe.md` enthält auch
Implementierungs-/Auditbefunde, die nicht zu neuen Abnahmen umgeschrieben werden.
Bei solchen Abschnitten den genauen Nachfolger statt einer rückwirkenden Korrektur
verwenden. Eine Datei kann beide Arten von Text enthalten.

| Datei | Erforderliche Änderung / Prüfung |
| --- | --- |
| [recipe.md](recipe.md) | Abschnitt zu `model_input_bindings`, Default/Selected und `prefix_free_workflow` präzisieren: Format-/Readerbefund von neuem Workflowziel trennen, UIvorausfüllung von gespeicherter Recipewahl und exakter Taskauflösung trennen; sichtbares Recipefeld mit Prefixless und anderen freigegebenen Prefixen planen, ohne Generate. Bestehende `/1`-Records nicht umdeuten. Aktuelle Runnerablehnung ehrlich erhalten, bis verbunden qualifiziert. |
| [docs/agents-v3/09-sempai-kohai.md](docs/agents-v3/09-sempai-kohai.md) | K3-2-Auswahl-/Lifecycle-Nachfolger: korrekte eigene Ausführerwahl bei Review, normale Rollenverträge separat und stets finaler Guard. |
| [docs/agents-v3/10-prefix-base-prompt.md](docs/agents-v3/10-prefix-base-prompt.md) | Base als zweite qualifizierte Arbeitskontextoption beschreiben; keine universelle automatische Voranstellung vor alle Rollenprefixe. Prefixgenerierung, Prefill und tatsächliche Nutzung unterscheiden. |
| [docs/agents-v3/12-agent-loop.md](docs/agents-v3/12-agent-loop.md) | Retained Auswahlergebnis, dynamische Instruktionen, optionale Reviewhand-offs und einmalige Prefixeinfügung durch den bestehenden Requestpfad beschreiben. Keine zweite Workflowloop. |
| [docs/agents-v3/17-webui-prefix-tab.md](docs/agents-v3/17-webui-prefix-tab.md) | Erstellungsstatus und Auswahlgrund für Maker/Base/prefixfrei erläutern; vorhandene Freigabe-/Readinessanzeigen wiederverwenden. Generiert ist nicht aktiviert. |
| WebUI-/Recipebeschreibung in `docs/agents-v3/17-webui-prefix-tab.md`, `recipe.md` und den Ownerplänen | Sichtbares Feld mit Vorausfüllung, manueller Änderung, Prefixless und anderen freigegebenen Prefixen vollständig beschreiben. Vorhandene UIanschlüsse `pages/settings/components/prefix-tab.js`, `prefix-model-designs.js`, `component-submission-panel.js` und deren Hooks/APIclient bei späterer Umsetzung inventarisieren; konkrete Recipeoberfläche verifizieren. Gemeinsame Options-/Validierungslogik wiederverwenden, nicht zwei abweichende Prefixkonfigurationen anlegen. EN/DE-Labels, Lade-/Fehler-/stale-Zustand und manuellen Änderungsstatus mitplanen. Diese Plandatei ändert keine UIdateien. |
| [scripts/prefix/prefix-maker-reference.md](scripts/prefix/prefix-maker-reference.md) | „Prefix-Maker must itself be fresh“ und „use its previous accepted generation“ als Pflicht ablösen: alter geeigneter Maker bevorzugt, sonst Base, sonst feste Instruktionen. Designausführer/Ziel und Kandidat als Prüfgegenstand getrennt halten. |
| [scripts/prefix/bootstrap/v1/system.txt](scripts/prefix/bootstrap/v1/system.txt), [extract_bootstrap_facts.txt](scripts/prefix/bootstrap/v1/extract_bootstrap_facts.txt), [select_bootstrap_prefill.txt](scripts/prefix/bootstrap/v1/select_bootstrap_prefill.txt) | Feste Promptartefakte auf Konflikte mit vorangestelltem Maker/Base prüfen. `/v1` nicht in-place umschreiben, wenn als genaue Vertragsbytes retained: erforderlichen versionierten Nachfolger beim bestehenden Promptowner planen, Manifest-/Schema-/Validatorbezug gemeinsam qualifizieren. Antwortschema und Payloadhash nicht ohne fachlichen Grund verändern. |
| [scripts/prefix/release-reference-inventory.md](scripts/prefix/release-reference-inventory.md) | Referenz-/Paketverfahren darauf prüfen, wie die korrigierten aktiven Texte in neue immutable Pakete gelangen. Keine alten Snapshotbytes oder Receipts ersetzen. |
| [docs/reborn/prefix-source-compiler.md](docs/reborn/prefix-source-compiler.md) | Offline-/Sourcecompiler von modellgestützten Arbeitsaufrufen abgrenzen; vorhandenes Makerprofil nicht als Beleg einer bereits aktiven Maker-Auswahl darstellen. Nur betroffene Anschlussaussagen ändern. |

Zusätzliche Anschlussprüfung, Änderung nur bei tatsächlichem Widerspruch oder
fehlender notwendiger Navigation:

- [ ] [AGENTS.md](AGENTS.md), [CLAUDE.md](CLAUDE.md),
  [docs/development-policy.md](docs/development-policy.md), [skills.md](skills.md),
  [tools.md](tools.md), [toolskills.md](toolskills.md): neue Regel auf den
  Erstellungsworkflow begrenzen, allgemeine normale Prefixauswahl und bestehende
  Komponenten-/Kernelverträge nicht pauschal umschreiben.
- [ ] [docs/agents-v3/04-ibs.md](docs/agents-v3/04-ibs.md),
  [docs/agents-v3/16-kernel-composition.md](docs/agents-v3/16-kernel-composition.md)
  und betroffene crateeigene `CLAUDE.md`/`AGENTS.md`: tatsächliche Capture-/Binding-
  und Ownerbeschreibungen verfolgen; nur operative Widersprüche korrigieren.
- [ ] [README.md](README.md), vorhandene übersetzte READMEs und
  [path_to_v3.5.md](path_to_v3.5.md): Navigation und Roadmap prüfen. Aktuellen
  Makeranschluss nicht weiter ausschließlich als spätere v3.5-Idee ausgeben;
  unabhängige v3.5-Arbeit bleibt unverändert.
- [ ] [prefix_plan2.md](prefix_plan2.md),
  [scripts/prefix/PREFIX-MAKER-USAGE.txt](scripts/prefix/PREFIX-MAKER-USAGE.txt),
  Sempai-Authoringreferenzen und weitere im Step-1-Suchlauf gefundene aktive
  Anleitungen: historischen/offline Scope feststellen; keine Offlinekommandos
  allein wegen des neuen Modellaufrufvertrags ändern. Widersprechende aktuelle
  Pflichttexte mit Nachfolgerverweis abgrenzen.

## 7. Step 5 — Status, historische Belege und eingebettete Referenzen

- [ ] [docs/implementation-status.md](docs/implementation-status.md): neuen offenen
  Nachfolger mit diesem Plan und dem gemeinsamen Vertrag verknüpfen. Bisherige
  scoped PrefixFree-Capture-/Encodingbefunde bleiben richtig für ihren Scope;
  neue Auswahlkette nicht als implementiert oder aktiviert melden.
- [ ] [docs/development-handoff.md](docs/development-handoff.md): nur aktuelle
  Navigation um den nachgelagerten neuen Auftrag ergänzen; vorhandene offene,
  laufende oder eingereichte Arbeitsaufträge nicht auf neue Kriterien umstellen.
  Historische Übergaben erhalten. Keine neue Zuständigkeits-/Trackingstruktur.
- [ ] [docs/development-build-queue.md](docs/development-build-queue.md) nur lesen;
  etwaige spätere Requests gemäß Policy an den führenden Owner geben. Nur dieser
  schreibt die Queue. Historische Buildanforderungen nicht umformulieren.
- [ ] `docs/validation/2026-10-29-exact-*`, weitere Validationreceipts,
  `docs/archive/`, Changelog-Historie, generierte Prefixpakete und archivierte
  Source-/Teaching-Snapshots unverändert lassen. Ein neuer aktueller Nachfolger
  grenzt ihre Aussage ab, ohne den damaligen Beleg zu ändern.
- [ ] Eingebettete Consumer in `prefix_release_documents.rs`,
  `prefix_reference_seeds.rs`, Releaseinventaren sowie zugehörigen
  Dokument-/Seedintegritätslesern für die spätere Lieferung erfassen. Veränderte
  Referenzprosa benötigt neue genaue Paket-/Revisionsbezüge; keine alten
  Checksummen oder akzeptierten DBversionen überschreiben. Dieser Plan ändert
  keine dieser Code-/JSON-/Seeddateien und beauftragt keine blinde Neugenerierung.

## 8. Step 6 — Verbindliche Abnahmematrix in die Ownerpläne aufnehmen

Die folgenden Fälle als zusammenhängende künftige Akzeptanz aufnehmen, nicht als
Behauptung heute vorhandener Tests. Bestehende gültige Evidenz wiederverwenden;
geänderte Verträge benötigen passende Nachfolgernachweise.

### 8.1 Zuständige Prüfpfade und beobachtbare Ergebnisse

Die nachfolgende Zuordnung ist in die neuen Ownerabschnitte zu übernehmen.
Dateinamen bezeichnen bestehende Erweiterungsstellen, keine Behauptung, dass die
neuen Fälle dort schon vorhanden oder bestanden sind. Keine parallele Testsuite
anlegen; bei inzwischen verschobenen Symbolen den tatsächlichen Nachfolger nutzen.

| Fallgruppe aus §8.2 | Owner und vorhandene Anschlussstellen | Erforderliche Beobachtung im späteren Nachweis |
| --- | --- | --- |
| Priorität, Freigabe, Identität, Namespace und Fehlerabgrenzung | Prefixowner: `prefix_acceptances.rs`, `prefix_acceptance_tests.rs`; Compositionconsumer: `task_model_provider.rs`, `task_model_provider_tests.rs` | Exakt die sichtbare manuelle oder unverändert vorausgefüllte Wahl samt Qualifikation bzw. explizites Prefixless erfasst; bei Fehler kein SDKversand. Kein Suchen archivierter Generationen oder fremder Namespaces. |
| Deklaration, kohärenter Capture, Altjobs und Childs | Composition/Simplified: `recipe_model_input.rs`, `task_model_capture.rs`, `global_recipe_ports.rs`, `native_model_usage_tests.rs`, vorhandene native Childadmission | Tatsächliche immutable Recipe-/Variantenauswahl und persistierter Capture stimmen überein; späterer aktiver Wechsel verändert Ergebnis nicht. Legitime Childherkunft funktioniert, fremde Herkunft und unbekannte Tags werden abgewiesen. |
| Requestaufbau, Guard, Cacheisolation und Antwortbeleg | Bestehender Modell-/Encodingowner: `task_model_provider.rs`, `pg_monty_model_response.rs`, `final_model_encoding.rs`, `final_model_encoding_tests.rs`, `runtime/tests/canonical_model_usage_tests.rs` | Tatsächliche an den SDKadapter übergebene Bytes enthalten genau den gewählten Prefix oder keinen; Messung, Guard und korrelierter Antwortbeleg binden denselben Stand. Bei Hardlimitverletzung kein Providerattempt. |
| B1/B2, Instruktionsrollen und erforderlicher Review | Exact-/Compositionconsumer und Validatorreview: versionierte Bootstrap-Promptartefakte, bestehende Bootstrap-Antwortprüfung und native Reviewconsumer | Vollständiger tatsächlich versandter Request mit festen Instruktionen, Quellenpacket und optionalem Prefix; fehlende Quellen werden nicht aus Prefixwissen ergänzt. Review verwendet eigene erfasste Route/Generation; Kandidat bleibt getrenntes Testsubjekt. |
| Liveänderung, Retry, Wait/Resume und GC | Bestehender Lifecycle-/Prefixowner und dessen native PG-/Tasktests | Persistierte ursprüngliche Auswahl und Attemptzählung bleiben erhalten, Livepolicy blockiert bei Bedarf; keine alternative Auswahl und kein Replay. Physischer GC erhält lebend referenzierte Artefakte. |
| Status und Bedienung | Connections-/Prefix-/Sempaiowner, vorhandene Setup-/Job-UI und deren native HTTP-/Browserconsumer | Sichtbar sind ausführendes Paar, tatsächlicher Zweig und Auswahlgrund bzw. Readinessfehler; kein zusätzlicher Use-, Generate- oder Aktivierungseffekt durch Anzeige, Refresh oder Reconnect. |

### 8.2 Fallmatrix

| Fall | Erwartung |
| --- | --- |
| Geeigneter freigegebener Maker und Base vorhanden | Maker wird sichtbar vorausgefüllt; nur bei unveränderter Absendung genau erfasst und einmal verwendet. |
| Maker fehlt oder ist nachweislich ungeeignet; geeignete freigegebene Base vorhanden | Base wird sichtbar vorausgefüllt; tatsächliche Wahl bleibt bis Generate änderbar. |
| Beide geeigneten freigegebenen Optionen fehlen | Prefixless wird vorausgefüllt; bei Absendung ohne Änderung vollständige feste Instruktionen und kein Scheinartefakt. |
| Leere Installation / erstes Paar | Nichtzirkulärer dritter Zweig ist vollständig funktionsfähig. |
| Maker-/Baseprüfung hat DB-, Integritäts- oder Resolverfehler | Fehler vor Modelleffekt; keine verdeckte Ersatzwahl. |
| Explizit gewählter Prefix unbrauchbar | Auswahlfehler; keine automatische Maker/Base/prefixfrei-Kette. |
| Vorhandenes Makerprofil oder Draft ohne Nutzungsfreigabe | Kein Nachweis für nutzbaren Maker und keine automatische Aktivierung. |
| Erstes/späteres Create und Maker-Redesign | Dieselbe Kette; Nachfolger ist nie eigene Freigabeautorität. |
| B1/B2 mit Base/Maker, fehlende oder widersprüchliche Quellen | Strikter Quellen-/Antwortvertrag gilt; Prefixwissen füllt keine Providerfakten auf. |
| Erforderlicher Child/Review mit gleichem oder anderem Paar | Gemeinsame Regel, passend erfasste eigene Generation, ursprüngliche Route/Auth. |
| Child mit erfasster Modellabwesenheit | Kein neues Modell aus globaler Rolle oder Parentprefix herleiten. |
| Generation aktiviert sich während Job/Wait/Retry | Bereits erfasste Auswahl bleibt; kein neues latest-Lookup. |
| Echter Kandidaten-Verhaltenstest | Kandidat ausdrücklich Prüfgegenstand, keine Produktionsaktivierung. |
| Ausgewählter Prefix oder Gesamtrequest überschreitet harte Grenzen | Blockieren ohne Trunkierung, Neuauswahl oder zweiten Effekt. |
| Alle drei tatsächlichen SDKrequests | Vollständige Messung/Guard/Policy und genaue Instruktions-/Generation-/Wirebelege. |
| Prefix ersetzt qualifizierte Makerinstruktionen | Einmalige Einfügung; keine verlorenen Aufgaben-/Antwortverträge oder Duplikate. |
| Parallele normale Chats und Erstellungsjobs | Keine globale Modusmutation oder Vermischung der Auswahlverträge. |
| Altjob ohne neue UI-/Auswahlmetadaten | Originalvertrag bleibt lesbar und unverändert; keine heuristische Migration. |
| Erforderlicher neuer Auswahlrecord trifft alten strikten Reader | Explizite Nichtunterstützung vor Aktivierung; keine Defaultinterpretation, neuer Writer erst nach Readerqualifikation. |
| Gleichzeitiger Provider-/Targetbindingwechsel während Capture | Ein kohärenter Stand oder expliziter Erfassungskonflikt, keine gemischte Route/Generation. |
| Parent-/Child-Capture mit verschiedener Recipe-/Occurrenceidentität | Legitime gemeinsame Workflowherkunft nachweisbar; keine blanket Ablehnung und kein fremder Child darf die Auswahl erben. |
| Gleiches Paar mit verschiedenen Instruktionszwecken oder Kandidatentest | Passendes Ergebnis und eigener Requestguard; keine falsche Wiederverwendung rollenbasierter Prepareddaten. |
| Prefix liegt vor, aber nativer Consumer unterstützt seinen Anschluss nicht | Implementierungs-/Readinessfehler; nicht als fachliches Fehlen zu prefixfrei ausweichen. |
| Keine geeigneten Prefixe, aber technischer Encoder/Protokollpfad fehlt | Konkreter Readinessfehler, kein ungeprüfter Bootstraprequest. |
| Fremder Namespace oder nur neuere Draftgeneration vorhanden | Kein namespacefremder Zugriff, kein Umgehen der aktiven Auswahl, keine automatische Entwertung gültiger Artefakte. |
| Quellenpacket, feste Instruktionen und Base/Maker enthalten kollidierende Anweisungen | Qualifizierte Zusammensetzung wahrt den festen Arbeits-/Quellenvertrag oder weist den Prefix als ungeeignet zurück. |
| Erlaubter Retry nach klassifiziertem Outcome | Gleiche Prefix-/Instruktionsbindung, persistierte Attemptzählung und bestehende Effektsicherheit; keine Ersatzwahl. |
| GC bei wartendem Arbeitsjob mit älterer Makergeneration | Artefakt und genaue Bezüge bleiben geschützt, erst nach Freigabe lebender Referenzen sammeln. |
| Maker/Base vorhanden; Betreiber wählt vor Generate Prefixless | Kein generierter Prefix im tatsächlichen Request; Vorschlagskette überschreibt den Wert nicht. |
| Betreiber wählt anderen freigegebenen kompatiblen Prefix | Genau diese Wahl wird geprüft, erfasst und einmal verwendet, unabhängig vom Maker/Base-Vorschlag. |
| Manuelle Änderung, danach Refresh/Reconnect oder neue Vorschlagsdaten | Änderung bleibt sichtbar erhalten; kein stilles Zurücksetzen und kein Modelleffekt. |
| Ausführerwechsel macht manuelle Auswahl inkompatibel | Sichtbarer Konflikt vor Absenden; keine heimliche Ersatzwahl. |
| Generation/Revision ändert sich zwischen Formularanzeige und Generate | Stale-Konflikt und erneute sichtbare Auswahl, keine unbemerkte Übernahme anderer Bytes. |
| Recipe speichern und ausführen | Gleichartige Prefixoptionen im Recipe-Editor; gespeicherte Auswahl durch normalen Reviewpfad, exakte Bindung beim Taskstart, kein Generate-Button. |
| Recipe wählt Prefixless bei vorhandenen Prefixen | Explizite Abwesenheit bleibt erhalten; kein Defaultfallback und keine erfundene Bootstrap-Purpose. |
| Headless oder erforderlicher Child ohne sichtbare/gespeicherte Auswahl | Keine heimliche Maker/Base-Vorauswahl beim Dispatch; konkreter Vorbereitungs-/Vertragsfehler statt unbemerkter Einfügung. |
| Generate-Doppelklick oder ungewisse Antwort nach Admission, anschließend Formularänderung | Vorhandene Job-/Request-ID und ursprüngliche Auswahlbytes zur Wiederholung behalten; kein zweiter Modelleffekt, keine neue Vorauswahl und kein Überschreiben des bereits zugelassenen Jobs. Eine tatsächlich neue Betreiberwahl ist ein neuer ausdrücklicher Auftrag. |
| Alle Zweige bei deaktivierten Tokenbudgets und unterschiedlichen gültigen Outputvorgaben | Keine künstlichen Token-/Retrievalcaps oder erfundene Reserve; technische Modell-/Output-/Kontext-/Protokollgrenzen und vollständiger Guard bleiben wirksam. Outputvorgaben stammen aus der tatsächlich gebundenen Quelle, nicht aus einer Maker/Base-Ersatzkonstante. |

## 9. Step 7 — Dokumentprüfung und Abschlussgrenze

- [ ] Alle Einträge der Dateimatrix als geändert, durch neuen offenen Nachfolger
  abgedeckt oder mit Scopebegründung unverändert markieren.
- [ ] Für jeden Plan vor/nach dem Patch nachweisen: alle vorher vorhandenen
  Abschnitte und Step-/Abnahmetexte unverändert; ausschließlich der zugeordnete
  neue Nachfolger ergänzt. Neue Dokuanforderungen nicht in alte Checklisten
  einfügen und bestandene Checkboxen nicht zurücksetzen. Keine neuen Nummern
  reservieren, die mit aktuell laufenden Ownerpaketen kollidieren.
- [ ] Für jeden Dokument-/Architekturtreffer eine explizite Geltungszuordnung
  prüfen: neuer Sollvertrag, tatsächlicher Implementierungsbefund, dritter Zweig
  oder geschützte alte Vertragsfassung. Ungeschützte aktuelle Widersprüche direkt
  korrigieren; geschützte Aussagen im Nachfolger genau abgrenzen. Ein allgemeiner
  Footer oder allein dieser Plan als Link reicht nicht für widersprechende
  aktive Anweisungen in einer anderen Datei.
- [ ] Suchlauf aus Step 1 wiederholen. Jede verbliebene Pflicht zur generellen
  Prefixfreiheit ist entweder gültiger dritter Zweig, geschützter historischer
  Befund oder lokal eindeutig durch den neuen Nachfolger abgelöst.
- [ ] Alle bisherigen Anker erhalten bzw. ihre Links gezielt nachführen;
  gemeinsame Regel nur einmal normativ ausformulieren und aus Consumerdateien
  referenzieren. Maker/Base/prefixfrei in Diagrammen, Beispielen, Matrizen und
  Zusammenfassungen auf dieselbe Bedeutung prüfen.
- [ ] Diff, Links, Schutzstatus und `git diff --check` prüfen. Für bloße
  Dokumentangleichung kein Cargo starten. Änderungen eingebetteter Referenzen
  erfordern später passende Integritäts-/Paketprüfung im zentralen Buildprozess.
- [ ] Abschluss als Dokumentnachfolger melden. Native Auswahl-/Workflowintegration,
  tatsächliche Provideraufrufe, Aktivierung, Upgrade und vollständiges v3 bleiben
  getrennte Arbeit in den vorhandenen Ownerplänen; keinen Sourcebefund als
  Laufzeitnachweis verwenden.

**Grenze dieses Auftrags:** Jetzt wird ausschließlich diese Plandatei gepflegt.
Alle Checkboxen beschreiben spätere Arbeit. Kein Code und keine der referenzierten
Bestandsdateien werden durch diesen Auftrag geändert.
