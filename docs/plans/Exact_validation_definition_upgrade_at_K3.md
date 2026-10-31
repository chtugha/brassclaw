# Exact validation definition upgrade at K3

**Stand: 31. Oktober 2026. Erstellt am 30. Oktober; am 31. Oktober thematisch aufgeteilt. Status: Dokumentations- und Planänderungsplan; noch nicht durchgeführt.**

Dieser Plan setzt die Betreiberpräzisierung zur eigenständigen Komponentenvalidierung
in konkrete Änderungen der Dokumentation, Architektur und bestehenden Umsetzungspläne
um. Seine Durchführung bearbeitet ausschließlich Dokumenttexte. Sie implementiert
keinen Rust-/Python-/SQL-/Frontendcode, ändert keine Datenbank, führt keine Migration
aus und aktiviert keine Komponente. Erforderliche Implementierungsarbeit wird als
offener Nachfolger in bereits bestehende Pläne eingetragen. Dieser Plan wird dadurch
nicht zu einem zweiten Implementierungsowner.

## 1. Prüfergebnis und Vorrang

**Dieser Plan enthält folgende Freigabe-/Versionsänderungen. Der zusätzlich
autorisierte Scaffoldabbau ist unten dem Freigabescope zugeordnet:**

1. Für neue/geänderte Komponenten einer installierten Instanz eigene
   Funktionsfreigaben statt zusätzlicher Genehmigung jeder exakten Versionskombination;
   ein Skill wird mit seinem implementierenden PythonCode gemeinsam geprüft.
   Die bestehende genaue Skillnutzungsqualifikation für `system_seed` bleibt.
2. Abwärtskompatibilität als Pflicht des Nachfolgers; unveränderte Verbraucher
   behalten ihre Freigaben.
3. Konsistente Aktualisierung über den bestehenden Katalogowner: neue Tasks
   verwenden aktive Nachfolger, laufende Tasks behalten ihre ursprünglichen Versionen.

Alle Aufträge zur vollständigen Workflowprüfung neuer/geänderter Recipes und zur
gemeinsamen Nutzung eines ToolSkills durch mehrere Skills wurden in
[Exact validation definition upgrade at K3-2](Exact_validation_definition_upgrade_at_K3-2.md)
exportiert. Dieser Plan ändert weder Umfang noch Kardinalitäten jener Verträge.
Ihre vorhandenen gültigen Anforderungen bleiben als Eingangsbedingungen bestehen;
das Einführen/Präzisieren dieser beiden Themen ist ausschließlich Auftrag des
neuen Plans. Gemeinsame Zielpfade begründen keinen doppelten Editauftrag.
Die Aufteilung erzeugt keinen pauschalen Vorgänger zwischen den beiden Plänen.

**Zusätzlicher Betreiberauftrag: Scaffoldklasse 50 abbauen.** Klasse 50 hat im
Ziel weder eine eigenständige ausführbare Rolle noch einen besonderen Freigabeweg.
Der vorliegende Plan entfernt dafür die eigene Schutz-/Validierungszuständigkeit;
der zweite Exactplan besitzt die fachliche Zuordnung auf normale Recipes und
gegebenenfalls deren bestehende Child-/Aufrufmechanik. Kein zweiter Migrationsowner.
Klasse 10 und der globale Orchestrator bleiben ausdrücklich getrennt.

**Verbindliche Präzisierung vom 31. Oktober: zwei Herkunftswege.** Alle hier
beauftragten Abschaffungen zusätzlicher Kombinationsprüfungen gelten ausschließlich
für neue/geänderte Komponenten auf einer installierten Instanz (**A**, authored).
Sie gelten ausdrücklich nicht für kontrolliert ausgelieferte Komponenten
(**S**, `system_seed`), auch nicht für qualifizierte ausgelieferte Nachfolger.
Die Seedarchitektur wird nur um eindeutigen Freigabescope und nachgewiesene
Voraussetzungsreihenfolge präzisiert. Ein laufendes System allein bestimmt den
Modus nicht: ein kontrolliert ausgeliefertes Upgrade kann S sein, ein Operator-/
Sempai-Edit an einer ursprünglich ausgelieferten Komponente ist A. Herkunft wird
vertrauenswürdig vom bestehenden Installation-/Authoringowner nachgewiesen,
nicht durch ein wählbares Label oder durch den aufrufenden Validator bestimmt.

Die Betreiberentscheidung korrigiert einen vorhandenen Architekturvertrag. Sie ist
keine bloße Erläuterung des bisherigen Textes:

- `skills.md`, Abschnitt **Exact-version association approval evidence**, verlangt
  bislang einen neuen Freigabebeleg für jede geänderte exakte Kombination, ausdrücklich
  auch bei kompatiblen Verträgen. `toolskills.md`, die Entwicklungspolicy und die
  Root-Agentenanweisungen wiederholen diese Forderung.
- `docs/authored-catalogue-activation.md` verlangt bei einem geteilten Nachfolger
  gemeinsame Recipe-Selektionen; `docs/reborn/component-deployment.md` beschreibt
  derzeit nur Deployment vollständiger Recipe-Subjects. Das ist eine vorhandene
  Implementierungsgrenze, keine passende Dauerarchitektur für unabhängige Updates.
- Der dokumentierte Pure-Review-Nachfolger verlangt teilweise neue Closure-Approvals
  bei Abhängigkeitswechseln. Auch diese Pflicht muss auf die Validierung des
  geänderten Components umgestellt werden, statt unveränderte Verbraucher neu
  freizugeben.
- Die automatische Auswahl neuester aktivierter Versionen bei Taskbeginn und das
  Festhalten bereits ausgewählter Versionen sind bereits richtig beschrieben.
  Exakte Auswahl, Provenienz und Aufbewahrung bleiben notwendig.

Die frühere Tabelle in [Exact V3 definition update at K3](Exact_V3_definition_update_at_K3.md)
enthält 67 Einträge, darunter genau **61 Markdown-Dateien und sechs Rust-Dateien**.
Die 61 Dokumente sind in §5 vollständig übernommen. §6 ergänzt die zusätzlich
betroffenen Vertrags-, Architektur- und Lehrquellen. Die frühere Anzahl ist keine
Obergrenze für diesen Abgleich.

Bei Durchführung ersetzt der neue Sollvertrag die widersprechenden aktuellen
Handlungsregeln des Instanzpfads A, nicht historische Ergebnisse oder den
weiter gültigen Seedpfad S. Der tatsächlich implementierte bisherige authored
Freigabepfad darf weiterhin als Iststand beschrieben werden, muss aber eindeutig
als noch umzustellende Implementierungsgrenze erkennbar sein. Ein Banner allein
genügt nicht, wenn der nachfolgende Text weiterhin eine gegenteilige aktuelle
Authoring- oder Freigabeanweisung gibt.

**Begriff V1:** Die vorhandenen Verfahren heißen Q1 und menschliches Q2.
Es entsteht keine zusätzliche Prüfphase und keine Beschränkung auf die numerische
Revision 1. Hier wird nur der Freigabescope einer eigenen Revision und die
Weitergeltung unveränderter Verbraucher geregelt. Den Prüfungsumfang eines
geänderten Recipes besitzt der exportierte Plan. Ein unverändertes Recipe wird
durch ein kompatibles Update einer Abhängigkeit nicht erneut zum Prüfkandidaten.

### 1.1 Erneut geprüfter Q1-/V1- und Seedstand

V1 bezeichnet im Betreiberauftrag die automatisierte Funktionsvalidierung,
die in den bestehenden Dokumenten Q1 heißt; Q2 bleibt der menschliche Review
für authored Änderungen. Das ist keine neue Phase und keine Revision-1-Regel.

| Beobachteter Stand / Quelle | Bedeutung für die folgenden Änderungen |
| --- | --- |
| [Validator: Baseline und Restarbeit](../../validator_v3.md#remaining-implementation--consolidated-2026-10-22), [verifizierte Verbraucher](../../validator_v3.md#2-verified-baseline-and-affected-consumers) | Legacy-Q1 enthält noch Klassenlookup/Bodyextraktion und einen gemeinsamen dreifeldrigen Nonemptycheck; laut Bestandsaudit kein gewöhnlicher Anwendungscaller dieses alten Runners. Das ist weder fertige achtklassige Funktionsvalidierung noch eine Basis, auf der Kombinationsfreiheit allein schon funktioniert. |
| [Eigene Q1-Teilbausteine](../../validator_v3.md#step-5--implement-reusable-facts-and-checking-components), `pg_retained_q1.rs`, Source-/Definition-/Contract-/Graph-/Coverage-/Subjectobserver | Immutable Subjects/Selections, Quellprüfung, eigene Beobachtungen, eingeschränkte Behaviorproducer und menschlicher exakter Usagereview existieren in belegten Scopes. Ganze Klassenvalidierung, gewöhnliche durable Q1-Auslieferung, allgemeine Pure-/Freigabewriter und umfassender Cutover bleiben offene Gates. Kein neues Statuslabel darf diese Lücken schließen. |
| [Fertiges Q1-Design: Steps 6–9](../../validator_v3.md#step-6--assemble-and-execute-multi-step-validation-recipes), [Klassenrouten](../../validator_v3.md#step-7--author-all-eight-class-specific-validator-recipes) | Vorhandene Validator-Recipes und wiederverwendete pure Checks/Toolnutzungen sequenzieren die tatsächliche Prüfung im globalen Monty. Rust liefert native Fakten, Custody und Speicher. Dieses Design wird für A optimiert, nicht durch einen neuen Rustvalidator, separaten Funktionsrunner oder eine zweite Prüfqueue ersetzt. |
| [Validator-Seedpfad und Reihenfolge](../../validator_v3.md#step-11--bootstrap-migration-upgrade-and-recovery) | Step 11 qualifiziert die erste neue Validatorgeneration nichtzirkulär, bevor Steps 10/12 sie veröffentlichen. Ein noch ungeprüfter Validator darf sich nicht selbst freigeben. Der Seedpfad benötigt keinen bereits vorhandenen produktiven Instanz-Q1 und kein installiertes menschliches Q2. |
| [Sempai v3.5 Step 10](sempai_v3.5.md#step-10--qualify-and-seed-the-successor-shared-library) und [Connections v3.5 Phase F](simplify_connections_v3.5.md#phase-f--integrate-components-settings-and-upgrades) | Beide Pläne unterscheiden schon reviewed shipped `system_seed` mit Integrität/Q1/Behavior von installed authored Q1/Behavior/human Q2. Der noch allgemeine Wortlaut zur exakten Kombinationsqualifikation muss bei Durchführung sauber auf S begrenzt werden. Bestehende Seedowner und gemeinsame Transport-/Storeschnittstellen bleiben. |
| [v3.5-Roadmap V35-03/04/17–20](../../path_to_v3.5.md#ongoing-plan-handoffs-not-parallel-implementations) | Allgemeine Purefreigabe und acht produktive Validatorrouten sind bestehende offene Aufgaben. V35 konsumiert deren Owner, statt selbst einen zweiten Validator zu implementieren. |
| `installed_monty_catalogue.rs` und vorhandene Bootstrapapproval-Owner | Es gibt echte paketbezogene Qualifikationsketten, beispielsweise native Artefaktprüfung und ausgewählte Generation-/Encodingqualifikation vor Compilerqualifikation. Das belegt keine universelle separat freigegebene Tool→ToolSkill→Skill-Reihenfolge für die gesamte Bibliothek. Den fehlenden Teil in bestehenden Ownern ergänzen, vorhandene Ketten nicht nachbauen. |

Es wurden Quellen/Verträge geprüft, keine neuen Laufzeit-/DB-/Cargoabnahmen
ausgeführt. Ältere Statusbelege behalten ihre genaue damalige Grenze.

## 2. Gemeinsamer Sollvertrag für alle Zieltexte

### 2.1 Technische Zuordnung und Freigabescope

Explizite Beziehungen zwischen Skill, PythonCode, ToolSkill und Tool bleiben
technisch erforderlich. Im Instanzpfad A werden sie nicht als zusätzlicher
gemeinsamer Freigabegegenstand für jede Versionskombination behandelt. Im
Seedpfad S bleibt der zusätzliche exakte Nutzungsbeleg für den gerade geprüften
Skill erforderlich; er ist keine Sammelfreigabe aller referenzierten Components.
Die vorhandenen
Binding-/Export- und Kernelgrenzen werden durch diesen Freigabewechsel nicht
aufgehoben. Änderungen an ToolSkill-Kardinalitäten, Nutzungsbeispielen oder
Resolverauswahl für mehrere Skills gehören ausschließlich in den exportierten Plan.

### 2.2 Eigene Funktionsvalidierung auf der installierten Instanz (A)

Dieser Abschnitt gilt für A. Der getrennte Seedpfad steht in §2.8.

1. Jede Komponente ist ein eigener vollständig validierter Gegenstand. Für
   installierte authored Revisionen bleiben Q1, erforderliche Verhaltens-/Semantik-
   nachweise und menschliches Q2 an ihrer eigenen unveränderlichen Revision
   gebunden. Eine neue eigene Revision braucht ihre eigene Validierung.
2. Die Validierung eines Components prüft auch die korrekte Nutzung seiner
   deklarierten Abhängigkeitsschnittstellen und die Übereinstimmung seiner Prosa,
   Exporte, Bindings, Fehler- und Effektverträge. Das ist Teil dieser einen
   Komponentenvalidierung, keine zweite Freigabe aller beteiligten Components.
   **Ein Skill und sein zugehöriger ausführbarer PythonCode bilden dabei einen
   gemeinsamen Funktionsprüfgegenstand**, obwohl sie getrennte Komponenten-UUIDs
   und Revisionen behalten. Ihre Prosa-/Export-/Parameter-/Ergebnis-/Fehlerverträge
   werden zusammen mit echten gültigen/ungültigen Aufrufen geprüft. Das ist keine
   zusätzliche Tool/ToolSkill/Skill/Code-Kombinationsprüfung und erzeugt keinen
   separaten authored Associationapprovalauftrag.
   Sind beide Revisionen neu, wird einmal gemeinsam geprüft und die eigene
   Freigabe beider geänderten Revisionen konsistent gespeichert. Ist nur eine
   Revision neu, wird ihre Funktion im unveränderten Gegenstück geprüft; dessen
   bestehende Freigabe wird verwendet, nicht neu erteilt. Keine Forderung, dass
   jeder der beiden Kandidaten bereits freigegeben sein muss, um den anderen
   prüfen zu können. Reine unabhängige PythonCode-Komponenten behalten ihre eigene
   Klassen-22-Funktionsprüfung; eine reine Helferabhängigkeit wird nicht deshalb
   erneut geprüft, weil ein neuer Skill sie nutzt.
3. Ein unverändertes Recipe behält seine Validierung, wenn eine verwendete
   Komponente einen für dieses Recipe kompatiblen, eigenständig validierten
   Nachfolger erhält. Ebenso behalten unveränderte Skills, ToolSkills, Tools und
   PythonCode ihre eigenen Freigaben. Es gibt keine transitive Q1-/Q2-Kaskade,
   keinen neuen Kombinationsreviewauftrag und kein zusätzliches Zustimmungstor.
4. Historische Recipeprüfungen behalten ihre exakten damals beobachteten Bytes.
   Ein neues Taskmanifest mit neuen Abhängigkeiten darf nicht als damals getestete
   Kombination ausgegeben werden. Die weitere Gültigkeit beruht auf der eigenen
   Freigabe des Nachfolgers und seinem nachgewiesenen Kompatibilitätsvertrag.

#### 2.2.1 Gemeinsames Subject: vorhandenen Single-root-Vertrag gezielt erweitern

U2 ergänzt im bestehenden Submission-/Reviewowner einen strikten versionierten
Nachfolger: vollständige geänderte Rootmenge, exakte Skill-/Codezuordnung,
unveränderte Prüfkontexte und bereits freigegebene Voraussetzungen unterscheiden.
`ReviewSubmissionDraft`/`submit_in_transaction` stagen heute einen Kandidaten;
die vorhandene Skill-Q2-Route schreibt einen exakten Associationapproval.
Keine dieser APIs wird als schon vorhandene gemeinsame Funktionsfreigabe ausgegeben.

- Geänderte Roots werden über die bestehende Autortransaktion mit ihren jeweils
  echten Base-/Versions-CAS und unveränderten physischen IDs koordiniert gestagt.
  Keine gegenseitige Vorabfreigabe verlangen und keine unabhängigen Fremdkandidaten
  in eine Gruppe aufnehmen. Unveränderte Gegenstücke sind Prüfkontext, keine Roots.
- Ein logischer Funktionsreview bindet die vollständige geänderte Rootmenge,
  Regel-/Exportabdeckung und beobachtete Ausführung. Beide Klassen behalten ihre
  eigenen Anforderungen; Wiederverwendung beseitigt doppelte Läufe, keine Regeln.
  Gemeinsamer Code für mehrere Skills benötigt die tatsächlich zugesagten
  Exportverträge; unveränderte Skillrevisionen werden nicht neu freigegeben.
- Vorhandene durable Admission, Work-/Outbox-, Verdict- und menschliche Q2-Owner
  tragen dieselbe Subjectidentität bis zum Ergebnis. Q2 sieht genau diese Bytes;
  die eigenen Freigaben aller geänderten Roots werden atomar gespeichert. Teilpass,
  fremdes Subject oder nachträglich geänderter Partner ergeben keine Freigabe.
  Ungewisser Commit wird mit denselben IDs/Bytes beim vorhandenen Store abgefragt,
  nicht mit neu angelegten Review-/Approvalidentitäten repariert.
- Die spätere Veröffentlichung nutzt ausschließlich U3. Ein gemeinsam geprüfter
  Nachfolger darf nicht mit einem ungeprüften Partner veröffentlicht werden;
  ein zusätzlich kompatibles, bereits genehmigtes Gegenstück braucht jedoch keinen
  neuen Kombinationsreview. Reviewgruppe und lebenslange Laufzeitkopplung sind
  verschiedene Dinge. Keine zweite Queue, kein Gruppenfreigabeservice.

### 2.3 Abwärtskompatibilität ist eine Pflicht des Nachfolgers

1. Ein Nachfolger unter derselben stabilen UUID muss die weiterhin benötigten
   öffentlichen Verträge erhalten, solange andere existierende Komponenten sie
   benötigen. Die Prüfung ist Bestandteil seiner eigenen Q1-/V1-Pflicht vor Q2
   und Aktivierung; ein bloßes `compatible=true`, Semverlabel oder Parserergebnis
   genügt nicht.
2. Die Bedarfsermittlung folgt echten deklarierten direkten und transitiven
   Abhängigkeiten, einschließlich nicht aktuell gerouteter, aber weiterhin gültiger
   Verbraucher. Ein ausgeblendetes Recipe ist nicht automatisch stillgelegt.
   Maßgeblich sind die noch zur neuen Verwendung vorgesehenen genehmigten
   Consumerrevisionen, nicht jede jemals gespeicherte Revision. Ausschließlich
   für alte Tasks aufbewahrte Consumer verbleiben auf ihrem alten Graphen.
   Ungenehmigte Entwürfe schaffen keine neue Blockade für einen ansonsten
   kompatiblen Nachfolger; vor ihrer eigenen späteren Aktivierung müssen sie
   zum dann gültigen Vertrag passen. Bereits ausdrücklich zugesagte Altverträge
   bleiben unabhängig von einem solchen Entwurf geschützt. Unbekannte oder
   unvollständig ermittelte genehmigte Abhängigkeiten dürfen nicht still als
   nicht vorhanden gelten.
3. Zu erhalten sind insbesondere Bedeutungen und Effekte, Exporte/Signaturen,
   zulässige alte Inputs, missing/null/default-Unterschiede, Resultatgarantien,
   Fehler-/Retry-/Idempotenzverhalten, Binding-/Adapter-/ABIverträge und relevante
   Reihenfolge-/Vollständigkeitsgarantien. Gleiche JSON-Typen allein beweisen keine
   semantische Kompatibilität. Der Validatorowner definiert dafür die konkreten
   Anforderungen je Klasse.
4. Für Inputs dürfen bisher gültige Aufrufe nicht ungültig werden; für Outputs
   dürfen bisher zugesicherte Garantien nicht entfallen. Neue optionale Felder
   sind nur dann kompatibel, wenn alte Aufrufe unverändert funktionieren und
   alte Verbraucher die neue Ergebnisform akzeptieren. Bei strikt verbotenen
   Extrafeldern ist selbst eine neue Ergebnisproperty nicht automatisch kompatibel.
5. Ein neues Pflichtargument für einen bisherigen öffentlichen Aufruf ist kein
   kompatibles Update. Es gehört in einen neuen Export/eine neue Schnittstelle
   mit erhaltenem alten Aufruf, in einen kompatiblen Adapter oder gegebenenfalls
   eine neue Komponentenidentität. Ein nur neuer Bootstrap-Producer darf seinen
   eigenen strengeren Eingabevertrag haben; er darf damit alte Consumer nicht
   nachträglich zu neuen Pflichtinputs zwingen.
6. Reale alte Consumerfixtures dürfen in der Nachfolgerprüfung ausgeführt werden.
   Das erzeugt ausschließlich Nachweise für den geänderten Nachfolger, keine
   neue Revision, kein neues Review und keine erneute Genehmigung der Consumer.
7. Ein brechender Kandidat wird nicht als kompatibler Nachfolger aktiviert. Eine
   neue Identität oder eine ausdrücklich geplante Consumerumstellung erlaubt
   neue Funktionen; tatsächlich geänderte Consumer durchlaufen ihre eigene
   Validierung. Kein stilles Entfernen abhängiger Recipes, kein alter Versions-
   fallback und keine automatische Massengenehmigung kaschieren einen Bruch.
8. Kompatibilitätsnachweise müssen nachvollziehbar für die noch benötigten alten
   Verträge gelten, nicht nur für den unmittelbar vorigen Nachfolger. Stilllegung
   oder Migration von Verbrauchern reduziert die Pflicht erst nach belegtem Ende
   ihres Bedarfs an neuen Ausführungen. Referenzen alter Tasks behalten unabhängig
   davon ihren eigenen alten Graphen; sie erzwingen keine dauerhafte Erweiterung
   des neuen Vertrags. Eine Vertragslineage darf unverändert erhaltene alte
   Garantien nachvollziehbar weitertragen, aber ein Kompatibilitätsbruch oder
   eine ausdrücklich entfernte Garantie darf nicht durch transitive Verweise
   verdeckt werden. Retainierte Tasks benötigen zusätzlich ihre alten Artefakte.

### 2.4 Aktivierung, laufende Tasks und Retention

1. Genehmigung und Aktivierung bleiben getrennt. Nach Aktivierung des gültigen
   Nachfolgers wählen neue Tasks automatisch die neueste aktive genehmigte
   Version aller benötigten UUIDs in einem konsistenten Katalogsnapshot. Recipes
   speichern weiterhin stabile UUIDs, keine festgeschriebenen Versionsnummern.
2. Der Aktivierungsowner muss geteilte Komponenten über die bestehenden Katalog-
   und Generationsmechanismen konsistent aktualisieren, ohne erneute Einreichung
   aller konsumierenden Recipe-Subjects. Automatische technische Neuauflösung
   oder abgeleitete neue Manifeste sind keine semantische Revalidierung.
3. Deterministische Prüfungen bei Aktivierung und Assembly bleiben: tatsächliche
   individuelle Freigaben, benötigte Verträge/Kompatibilität, Vollständigkeit,
   eindeutige Bindings, Artefakte und laufende Supportgrenzen. Sie führen keine
   neue Q1-/Q2-Runde aus. Für A genügt die eigene gültige Funktionsfreigabe mit
   benötigter Kompatibilität; für S wird weiterhin der passende genaue
   Skillnutzungsbeleg geprüft. Weder Beleg erteilt Toolberechtigung oder genehmigt
   die anderen beteiligten Komponenten neu.
4. Ein laufender, wartender oder wiederaufnehmbarer Task behält seine vollständig
   ausgewählten Versionen einschließlich Exporte, Abhängigkeiten und Artefakte;
   Child/Retry/Resume desselben Tasks wählen nicht erneut latest. Alte Versionen
   werden durch Ersatz nicht mehr für neue Tasks gewählt. Ihre bisherigen Belege
   und laufenden Auswahlen werden nicht nachträglich ungültig geschrieben.
5. Alte ausführbare Versionen sind für laufende/suspendierte/recoverbare Tasks und
   deren echte Referenzen aufzubewahren. Nach Ende dieser Referenzen können sie
   nur durch einen für genau diese Artefaktklasse nachgewiesenen referenzsicheren
   Löschpfad freigegeben werden. Ein vollständiger allgemeiner Komponenten-GC
   wird hier nicht als implementiert vorausgesetzt. Benötigt ein offener Review,
   eine Integritätsprüfung oder unterstützte Recovery noch Originalbytes, bleiben
   diese geschützt. Kompatibilitätsverträge, Lineage und Auditbelege haben eigene
   Referenzen und dürfen nicht zusammen mit unnötigem ausführbarem Payload
   verschwinden. Solche Belege erlauben keine neue Ausführung der alten Version.
6. Explizite Sperrung/Revocation, aktuelle Toolpolicy, technische Ressourcenlimits,
   Claim-/Attemptfencing und sichere Effektreconciliation bleiben unabhängig.
   Nur echtes No-Match führt zu Tier 2.

### 2.5 Historische Formate, Bootstrap und konkreter Prefixfall

- `skill-association/1` beschreibt weiterhin technische Beziehungen.
  `skill-association-approval/1` bleibt für S ein aktuell zu verwendender genauer
  Nutzungsbeleg und wird nicht allgemein zum historischen Auslaufmodell erklärt.
  Alte authored exakte Reviewrecords bleiben mit ihrer ursprünglichen Bedeutung
  lesbar. A bekommt eine ausdrücklich versionierte Weiterentwicklung auf eigene
  Funktionsfreigaben, gemeinsame Skill-/Codeprüfsubjects und Kompatibilität im
  vorhandenen Review-/Submissionowner. Kein neues Feld in bestehenden /1-Bytes,
  keine neue Tabelle/API als bereits unterstützt ausgeben und keine alten Belege
  auf neue Kombinationen umschreiben.
- Eine individuelle Freigabe darf weiterhin exakte Testabhängigkeiten und den
  realen Validator/Interpreter für die Belegprovenienz festhalten. Diese
  Belegidentität wird nicht zur Pflicht, bei jedem kompatiblen Abhängigkeitswechsel
  den unveränderten Component neu zu genehmigen. Laufzeitpins bleiben exakt.
- Die Formatmigration ist kein Anlass für eine nachträgliche Q1-/Q2-Runde aller
  unveränderten Components. Der bestehende Reviewowner muss eine nachvollziehbare
  Zuordnung bereits ausreichend geprüfter eigener Revisionen aus vertrauenswürdigen
  Altbelegen planen. Fehlende historische Einzelabdeckung wird als konkrete Lücke
  ausgewiesen; sie darf weder durch ein Label erfunden noch als pauschale
  Consumer-Reapprovalpflicht bei jedem künftigen Update fortgeführt werden.
  Fehlt nur der neue Reader oder seine vertrauenswürdige Zuordnung, bleibt das
  ein technischer Migrationsauftrag, keine neue menschliche Entscheidung.
  Fehlt eine damals tatsächlich nie geprüfte Komponenteneigenschaft, ist sie
  dagegen nicht nachträglich durch das Formatupgrade genehmigt. Das bisher
  gültige ausführbare Altprofil bleibt in seinem bestehenden Scope verwendbar;
  nur die davon nicht gedeckte neue Freigabesemantik wartet auf ausreichende
  Evidenz. Keine pauschale Abschaltung unveränderter gültiger Workflows.
- Qualifizierte ausgelieferte first-party Komponenten nutzen weiter das
  nichtzirkuläre `system_seed`-Verfahren mit Integrität/Q1/Behavior. Es entsteht
  kein zusätzliches Q2-Tor auf der installierten Instanz. Operator-/Sempaiänderungen
  bleiben authored. Herkunftslabels ersetzen keine Nachweise.
- Das neue optionale `max_output_tokens` darf den bisherigen gültigen nativen
  Ersatzwert alter Profile erhalten. Missing ist nicht null/zero. Ein neuer
  Bootstrap-Producer benötigt seinen real zugelassenen typisierten Wert. Das
  erlaubt keinen alten Komponentenfallback, keine erfundene Custodian-Konstante
  und keine allgemeine Bootstrap-/Free-Aktivierung. B1/B2-Prompt-/Packetbytes
  bleiben unverändert; I1 bleibt ohne tatsächliche zulässige Eingabe unaktiviert.

Für **S** verbleibt die vorhandene Kombinationsqualifikation beim Installationowner.
Für **A** muss der normale Q1 dagegen keine allgemeine Versionskombinationen-
Erzeugung, -Prüfmatrix oder -Freigabe mehr implementieren. Er liest vorab gültige
Tool-/ToolSkill-/Helferverträge und prüft die Funktion des tatsächlichen Kandidaten.
Der ausführende Validator selbst behält seine eigene bereits qualifizierte
Auswahl einschließlich nötiger Seedbelege; dies ist seine Ausführungsberechtigung,
nicht die Kombinationsprüfung des reviewed authored Kandidaten.

### 2.6 Verträge und parallele Änderungen ohne versteckte Kombinationsfreigabe

Die Kombinationsfreiheit dieses Abschnitts gilt für A. S verwendet die bestehenden
exakten Belege und die Voraussetzungen aus §2.8; technische Vertrags-/Policy-/
Integritätsprüfungen bleiben in beiden Pfaden wirksam.

1. Kompatibilität muss auf ausdrücklich beschriebenen öffentlichen Garantien und
   deklarierten Anforderungen der Consumer beruhen. Dazu gehören relevante
   Beziehungen zwischen Parametern/Ergebnissen und mehreren Abhängigkeiten sowie
   gemeinsame Effekt-/Zustandsannahmen. Getrennt genehmigte Components beweisen
   nicht automatisch einen beliebigen neuen Workflow. Versteckte Consumerannahmen
   sind eine Vertragslücke; sie werden nicht durch einen pauschalen Kompatibilitäts-
   stempel oder durch nachträgliches Ausblenden des Consumers behoben.
2. Der Nachfolgerreview hält fest, welche benötigten Altverträge und welcher
   konsistent gelesene Bedarf seine Nachweise abdecken. Die vorhandenen Revision-/
   Submission-/Katalogstores sind die Quelle; ein ableitbarer Dependencyindex
   darf keine zweite autoritative Consumerregistry werden. Unvollständige Graphen,
   technische Kapazitätsgrenzen oder unzureichende semantische Nachweise ergeben
   einen unvollständigen Kandidatenreview, niemals eine gekürzte positive Prüfung.
3. Die bestehenden Aktivierungstransaktionen müssen Consumerneuzugang, Retirement,
   abhängige Componentupdates und aktuellen Katalogstand gegen den geprüften
   Bedarf abgleichen. Eine zwischenzeitliche Änderung, die vollständig unter die
   bereits nachgewiesenen Garantien fällt, erfordert keine neue Prüfung. Verlangt
   sie einen nicht abgedeckten Vertrag, bleibt die betroffene neue Veröffentlichung
   konfliktbehaftet bzw. unvollständig; der Review des betroffenen Kandidaten wird
   beim vorhandenen Owner vervollständigt. Keine neue Genehmigung des unveränderten
   Consumers, kein per-Aufruf-Review und kein globaler Entwicklungsfreeze.
4. Auch mehrere gleichzeitig veröffentlichte Nachfolger müssen zusammen die
   deklarierten Anforderungen erhalten. Der vorhandene Katalogowner prüft das
   mechanisch anhand der darstellbaren Verträge im vollständigen geplanten Graphen,
   einschließlich transitiver Interfaces und deklarierter relationaler
   Recipebedingungen. Konkrete zukünftige Toolergebnisse kann er nicht vorab
   auswerten: ihre Werte werden weiterhin beim jeweiligen typed Handoff geprüft;
   ihre zugesicherten Bedeutungen gehören in die vorherige Komponenten-/Recipe-
   validierung. Der Katalog führt keinen neuen semantischen Kombinationsreview
   aus. Ein Konflikt darf weder durch unabhängige
   erfolgreiche Einzel-CAS noch durch Weglassen eines Workflows verborgen werden.
5. Bestehende Schemavergleiche müssen in ihren owning Readers ausdrücklich
   berücksichtigt werden. `prepare_retained_tool_program` verlangt heute exakte
   Gleichheit von Code- und Association-Input-/Resultrecords; die aktuelle
   Approvalvererbung verlangt die gleiche Closure. Das Entfernen des Q2-Erfordernisses
   allein würde kompatible Updates deshalb weiter blockieren. U4 muss den neuen
   gerichteten Vertrag zwischen erforderlichem Interface und tatsächlichem
   Anbieter implementieren, mit geprüften alten Export-/Resultprofilen, ohne
   unbekannte Felder generell zu akzeptieren oder Daten still zu konvertieren.
   Echte Integritäts-/Identitätsgleichheit bleibt exakt; historische Formate und
   alte Taskreader behalten ihre ursprüngliche Bedeutung.
6. Eine unabhängige Prüfung beliebiger Pythonprogramme kann semantische
   Äquivalenz nicht allein aus Parserfakten oder einer endlichen Fallliste beweisen.
   Erforderlich sind der vollständige deklarierte Vertrag, passende strukturelle
   Regeln und echte Behavioral-/Semantiknachweise im bestehenden Reviewverfahren.
   Bleibt ein konkret benötigter Vertrag ungeklärt, wird der Kandidat nicht als
   kompatibel freigegeben. Es entsteht kein Anspruch auf automatische Zustimmung.

### 2.7 Umfang des Cutovers

Die technische Umstellung muss Komponentenfreigabe, Kompatibilitätsnachweise,
Aktivierung und alle tatsächlich konsumierenden Reader als zusammenhängendes
Paket berücksichtigen. Der neue Katalog-/Taskformatstand bestimmt eindeutig,
welche validierten Reader und Belege gelten. Ein Modellflag, Operatorfeld oder
unspezifischer Feature-Schalter darf alte exakte Approvalprüfungen nicht einfach
überspringen. Alte gültige Auswahlen bleiben über ihre bestehenden verifizierten
Lesepfade ausführbar; neue Auswahlen werden erst mit vollständigem passenden
Nachfolgerpfad veröffentlicht. Das schreibt kein neues Format oder eine zweite
Aktivierungsengine vor und aktiviert keine bisher fehlende Runtimefähigkeit.

Die Reader müssen A-Funktionsfreigaben und S-Nutzungsbelege anhand tatsächlicher
Provenienz und des expliziten Format-/Vertragstands unterscheiden. Die Herkunft
gehört zur eigenen konkreten Revision beziehungsweise qualifizierten Nutzung,
nicht pauschal zum Task, Recipe oder ganzen Katalog. Gemischte Graphen sind erlaubt:
ein A-Skill darf unveränderte gültige S-Tools/ToolSkills nutzen. Diese werden weder
zu A umetikettiert noch durch seine Freigabe neu genehmigt. Umgekehrt darf ein
S-Nutzungsbeleg nicht ungeprüft für einen abweichenden authored Partner gelten;
Overrideauflösung und benötigte genaue S-Belege bleiben beim vorhandenen Owner.
Ein ausgeliefertes Upgrade überschreibt keine authored Overrides. Die technische
Kompatibilität allein überträgt keinen S-Exaktnutzungsbeleg.

U2/U4 erfassen vor Veröffentlichung die tatsächlichen Leser für Prepare/Assembly,
Aktivierung, Taskretention, Child/Resume/Restore, private Reviewadmission und die
betroffenen Prefix-/MCP-Verbraucher. Nur ein vollständig unterstütztes neues
Profil wird veröffentlicht; unveränderte gültige Alt-/S-Profile bleiben in ihrem
belegten Scope nutzbar. Der schon qualifizierte Validator darf seine bisherige
Auswahl weiter verwenden, um neue A-Subjects zu prüfen: keine Bootstrapabhängigkeit
vom erst zu qualifizierenden neuen Freigabeformat. Es entsteht
keine zweite allgemeine Freigabeengine und kein impliziter Wechsel des Modus.
Native Registration/Artefaktprüfung, geschützte
Orchestratorwurzeln der Klasse 10, qualifizierte private Admission, Prefix-/Free-Auswahl und
öffentliche MCP-Commandverträge behalten ihre jeweiligen eigenen Voraussetzungen.
Der Entfall der zusätzlichen Versionskombinationsfreigabe ersetzt diese Gates nicht.

### 2.8 Seedpfad S: bestehende Kombination, eng begrenzter Scope und Reihenfolge

1. S bleibt kontrollierte Installation/ausgeliefertes Upgrade mit geprüftem
   Paket-/Source-/Nativeartefakt, struktureller/automatisierter und beobachteter
   Verhaltensqualifikation. Diese Prüfungen laufen im vorhandenen nichtzirkulären
   Bootstrap-/Entwicklungsweg, **nicht durch die normale authored Q1-Queue**.
   Wiederverwendete Prüffunktionen sind erlaubt, eine vorgetäuschte normale
   Q1-Ausführung ist es nicht.
2. Die geforderte vorherige Q1/Q2-Qualität von Seed-Tools und Seed-ToolSkills wird
   durch deren bestehende kontrollierte Entwicklungsreview-/Integritäts-/Behavior-
   freigabe nachgewiesen. Nach geltendem Seedvertrag entsteht dadurch weder ein
   zusätzliches externes Maintainer-Q2 noch ein menschlicher Q2-Klick auf der
   installierten Instanz. `q2_ref=null` bleibt für echte S-Belege korrekt; keine
   künstlichen `human_q2`-Records zum Darstellen einer Reihenfolge erzeugen.
   Authored Tool-/ToolSkill-Voraussetzungen müssen dagegen ihre echten vorhandenen
   eigenen Q1/Behavior/human-Q2-Belege besitzen. Herkunft darf nicht wechseln.
3. Abhängigkeiten werden topologisch qualifiziert: zuerst benötigte unabhängige
   Helfer/Prüfprimitive und das tatsächliche Tool samt Registrierung/Artefakt/
   Parametern/Fehlern/Effekten, **danach der zugehörige ToolSkill** samt realem
   Bindingvertrag, **danach Skill und implementierendes PythonCode gemeinsam**.
   Erst anschließend werden konsumierende Recipes/Pakete qualifiziert und gemäß
   vorhandener Startupfolge veröffentlicht. Das ist eine Abhängigkeitsreihenfolge,
   kein zweites Seeding-/Bootsystem und keine starre Totalordnung aller Klassen.
   Die Toolprüfung darf keinen erst danach zu erstellenden eigenen ToolSkill oder
   Skill als bereits genehmigte Voraussetzung verlangen. Die ToolSkillprüfung
   verwendet den vorhandenen vertrauenswürdigen Binding-/Testadapter gegen das
   qualifizierte Tool, nicht den noch ungeprüften künftigen Skill als Prüfer.
   Candidate-/Fixtureausführung bleibt im bestehenden begrenzten Review-/
   Bootstrapkontext, niemals gewöhnliche Aktivierung eines ungeprüften Components.
4. Ein neuer Skill darf die fehlende eigene Freigabe seines Tools, ToolSkills oder
   eines unabhängigen Helfers nicht durch erfolgreiche gemeinsame Ausführung
   ersetzen. Schon genehmigte unveränderte Voraussetzungen werden nur verifiziert
   und wiederverwendet. Der Skill-/Codekandidat selbst benötigt vor seiner
   gemeinsamen Prüfung keine zirkuläre Vorabfreigabe des jeweils anderen.
5. Der anschließende genaue Seed-Kombinationsbeleg bestätigt ausschließlich:
   **dieser Skill mit diesem implementierenden Code funktioniert mit diesen
   bereits freigegebenen Voraussetzungen in diesen exakten Revisionen**.
   Seine Komponentenliste identifiziert die Test-/Auswahlclosure, erteilt aber
   keine neue eigene Freigabe jeder darin genannten Komponente. Derselbe ToolSkill
   kann in mehreren qualifizierten Nutzungen vorkommen, ohne erneut als ToolSkill
   freigegeben zu werden. Keine beliebige Permutationsmatrix aller Components.
6. Bei einem ausgelieferten geänderten Seed-Skill/Code ist dessen genaue
   Nutzungskombination erneut über den vorhandenen Bootstrapowner zu qualifizieren.
   Unveränderte Voraussetzungen behalten ihre eigenen Freigaben. Ändert sich eine
   Voraussetzung, wird zuerst nur deren neue eigene Revision qualifiziert;
   danach werden erforderliche genaue neue Seednutzungsbelege erstellt, ohne
   unveränderte Abhängigkeiten erneut einzeln zu genehmigen. Das ist bewusst
   verschieden vom A-Pfad ohne zusätzlichen Kombinationstest.
7. Vorhandene Seedreader/-writer und Scopebelege bleiben möglichst unverändert.
   Wo ein bisheriger Paketbeleg nur die ganze Nutzung und keine separate
   vorherige Tool-/ToolSkillfreigabe nachweist, ist diese konkrete Evidenzlücke
   beim vorhandenen Owner zu schließen. Kein Generalneubau aller Seedprüfungen,
   keine Umdeutung alter Belegbytes und keine Aktivierung durch ein `source`-Label.
   Fehlt eine darstellbare eigene Voraussetzungsfreigabe, plant U6 beim bestehenden
   Installation-/Reviewformatowner einen eng begrenzten verknüpften Nachweis mit
   tatsächlicher Klassen-/Revisions-/Artefaktidentität, Prüfer und Regel-/Behavior-
   scope. Ein bisheriger Skillnutzungsbeleg allein kann diesen nicht ersetzen.
   Bestehende `/1`-Nutzungsrecords bleiben unverändert; kein Fake-Skill um ein Tool,
   keine neue allgemeine Seed-Datenbank und keine rückwirkend erfundene Freigabe.

### 2.9 Normalen V1/Q1 gezielt auf Funktionsprüfung optimieren

Die folgenden Änderungen sind im offenen Nachfolger von `validator_v3.md`
unterzubringen. A benötigt keine allgemeine Kombinationsprüfungsfähigkeit.
Das Verifizieren vorhandener Dependencyfreigaben, technischer Bindings und
Schema-/Callgraphverträge ist trotzdem Bestandteil einer korrekten Funktionsprüfung.
Ein erfolgreicher Lauf gegen Fakeantworten allein genügt nicht.

| Bestehende Arbeit / Zielpassage | Konkrete Optimierung für A; S bleibt beim Bootstrapowner |
| --- | --- |
| Steps 1–3: Subject, Auswahl, Abhängigkeiten | Herkunft, geänderte Revisionsmenge und Funktionsprüfgegenstand vor Reviewarbeit bestimmen. Für Skill/Code ein vollständiges gemeinsames Subject mit beiden exakten Referenzen und geänderten Roots im bestehenden Submission-/Reviewowner unterstützen. Vorhandene Single-root-Schemas nicht durch erfundene Felder umgehen; nötigen strikten Nachfolger dort planen. Tool/ToolSkill/Helfer müssen bereits gültig sein und werden nicht erneut als Kandidaten eingereiht. |
| Step 5/5a: gemeinsame Quell-/Contract-/Graphchecks | Vorhandene Parser-/Preload-/Schema-/Callgraph-/Coveragebausteine wiederverwenden. Gleiche immutable Quellen/Fakten pro tatsächlichem Regel-/Interpreter-/Inputscope einmal prüfen und zulässige Nachweise referenzieren. Keine doppelte gleiche Sourceprüfung durch Skill- und Klassen-22-Route; veränderte relevante Inputs erzwingen neue Nachweise. Unterschiedliche technische Grenzen oder Truststufen nicht durch einen globalen Erfolgscache vermischen. |
| Steps 6–7: Klassenrouten | Tool prüft seine eigene deklarierte Primitive; ToolSkill sein eigenes reales Binding gegen das vorab gültige Tool; Skillroute prüft zusammen mit Code seine Exporte/Parameter/Resultate/Fehler/Effekte und Prosaübereinstimmung. Klassen-22-Regeln dabei als dieselben wiederverwendeten Teilchecks verwenden, nicht eine zweite vollständige Codeprüfung starten. Standalone-Purecode bleibt eigener Funktionsgegenstand. Bestehende acht Klassenrouten bleiben, keine neue „Kombinationsklasse“. |
| Steps 7–8: tatsächliche Funktionsfälle | Pro deklariertem Export sinnvolle gültige/ungültige/Grenzfälle, missing/null/defaults, echte Ergebnisverarbeitung, Fehler-/Effekt-/Policyverhalten beobachten. Ein gültiges Tool mit ungeeigneter Rückgabeannahme muss den Skill/Codekandidaten ablehnen, nicht die Toolfreigabe ändern. Fremde mutable Zustände, verdeckte Toolcalls und unvollständige Fälle bleiben Fehler. Konkrete unveränderte Dependencyfunktionen nicht erneut vollständig testen. |
| Steps 8–9: Ergebnissammlung und Q2 | Ein Funktionsprüflauf, eine vollständige erforderliche Regelmenge und ein zusammenhängender menschlicher Review der geänderten Revisionen. Sind Skill und Code neu, gelten die gemeinsamen beobachteten Ergebnisse für beide eigenen Freigaben; keine doppelte Testausführung oder zweite menschliche Runde für denselben Gegenstand. Unveränderte Gegenstücke behalten ihre alten Freigaben. Authentische Provenienz, exakte Q2-Ansicht, idempotente Persistenz und atomare Freigabe der koordinierten Änderung über bestehende Owner gewährleisten. |
| Steps 9–10: Verdict-/Approvalwriter und Aktivierung | Keine authored `skill-association-approval/1`-Erzeugung oder erneute Queuearbeit für jede Dependencykombination. Eigene validierte Funktions-/Kompatibilitätsbelege konsumieren. Technische Zuordnung, Nativeartefakt, aktuelle Toolpolicy, generationstreue Auswahl und vollständige Runsettlement bleiben geprüft. Keine öffentlichen Passflags. |
| Steps 11–12: Bootstrap und UI | Seedkombinationen nicht durch A-Q1 routen. Bestehenden Seedpfad S und benötigte genaue Belege erhalten; UI zeigt vorab freigegebene Voraussetzungen getrennt von gemeinsam geprüftem Skill/Code, Freigabebereich und Herkunft. Ein Seedbeleg kann keine fehlende Tool-/ToolSkillfreigabe verdecken. |
| Klassen-21-Route / exportierter Plan | Neues/geändertes Recipe prüft alle von ihm angebotenen unterstützten Kombinationen, Varianten, Bindings und Daten-/Fehler-/Childpfade als eigenen Workflow. Das ist keine allgemeine Kombinationenprüfung anderer Komponenten. Deren vorhandene Freigaben werden nicht neu erteilt. |

Wiederverwendbare Parser-/Schemafakten dürfen nur mit vollständigem relevantem
Quell-/Vertrags-/Regel-/Interpreterprofil referenziert werden. Beobachtete Behavior-
oder Semantikbelege behalten zusätzlich ihre tatsächlichen Subject-, Input-,
Nativeartefakt-, Limit-, Run- und Herkunftsbindungen. Gleiche Rückgabewerte oder
gleicher Code beweisen keinen fremden Reviewlauf. Der vorhandene Subjectobserver
und vollständiges Settlement bleiben pro erforderlichem Review wirksam; Q2 lässt
sich nicht aus einem technischen Erfolgscache ableiten. Neue vertragsrelevante
Inputs verlangen gezielt neue Nachweise, keinen pauschalen Komplettlauf.

Bei gemeinsamem PythonCode für mehrere Skills bestimmt die deklarierte
Export-/Nutzungsvertragsmenge die Tests des geänderten Funktionskandidaten.
Identische benötigte Checks werden nicht pro Consumer wiederholt. Unterschiedliche
benötigte öffentliche Verträge dürfen dabei nicht verloren gehen. Es werden
keine unveränderten Skills zur neuen Genehmigung eingereiht. Ein neuer Skill,
der bereits genehmigten Code nutzt, prüft seine eigene Nutzung mit diesem Code,
nicht nochmals die gesamte Bibliothek. Q1-Basisfunktionen bleiben vollständig;
„Optimierung“ darf weder bloße Syntaxkontrolle noch unbewiesene Semantik bedeuten.

### 2.10 Keine Scaffoldkomponente oder Sondervalidierung der Klasse 50

1. Die bisherige Kategorie „Scaffold“ wird nicht als neue Komponente weiter
   verwendet. Es gibt keinen aktiven Klasse-50-Typ, keinen Scaffoldvalidator,
   keine besondere Scaffold-Q2-/Bootstraproute und keinen mit Klasse 10
   zusammengefassten Schutzstatus im Ziel. Die bestehenden Klassenrouten werden
   nicht um eine neunte Scaffoldroute erweitert.
2. Ein tatsächlicher wiederverwendbarer Ablauf wird als normales Recipe der
   Klasse 21 mit passenden Inputs beziehungsweise explizitem Aufruf eines anderen
   Recipes beschrieben. Er verwendet dessen normale A-Recipevalidierung oder
   S-Paketqualifikation. Er erhält keine Orchestratorrechte durch seine frühere
   Bezeichnung. Reine Navigation darf auf dieses konkrete Recipe verweisen,
   ist aber weder ausführbarer Klasse-50-Alias noch neue Dispatch-/Trustschicht.
3. Vor einer Umstellung tatsächliche Klasse-50-Datensätze und direkte/transitive
   Referenzen in bestehenden Stores, öffentlichen/private Routen, Seeds, Settings,
   Prefix-/Retrievalquellen, Reviewsubjects und Task-/Recoveryauswahlen inventarisieren.
   Die Existenz eines Klassenlabels beweist weder tatsächliche Daten noch eine
   bestehende ausführbare Funktion. Bei leerem Bestand keine Dummyrezepte,
   Platzhaltermigration oder Ersatzregistry erzeugen.
4. Historische immutable Datensätze, Belege und bereits gültig gepinnte Tasks
   werden nicht rückwirkend umklassifiziert oder gelöscht. Datenintegrität und
   echte Taskpins schützen deren ursprüngliche Bytes; das ist kein besonderer
   Scaffold-Freigabestatus. Historische Decoder dürfen den alten Klassencode
   weiterhin lesen. Das Verbot betrifft neue Erstellung/Aktivierung und neue
   Taskselektion; bereits gültige alte Pins behalten ausschließlich ihren alten
   Ausführungsscope. Alte nie qualifizierte Rows werden nicht grandfathered.
   Den allgemeinen `ComponentRevisionDraft`-Decoder deshalb nicht global auf
   Ablehnung von 50 umstellen: historische Readback-/Integritätswege benötigen ihn.
   U11/W-R schließen stattdessen alle tatsächlichen neuen Authoring-, Import-,
   Proposal-, Queue-, Freigabe- und Publikationspfade einschließlich Legacy-APIs.
   Kein verbleibender Alias darf darüber neue geschützte Klasse50-Rows erzeugen.
5. Neue aktive Consumer zeigen auf das konkrete vorhandene passende Recipe oder
   einen nach dem normalen Verfahren geprüften Recipe-Nachfolger. Neue Klasse21-
   Identitäten werden durch den vorhandenen UUID-/Revisionsowner zugeordnet;
   eine alte Klasse50-UUID wird nicht als Identität einer anderen Klasse neu
   vergeben. Geänderte Consumer erhalten ihre eigenen immutable Revisionen; unveränderte
   Abhängigkeiten werden nicht neu geprüft. Fehlende semantische Zuordnung bleibt
   eine konkrete offene Migrationslücke, kein stilles Ausblenden oder No-Match-
   Fallback eines aktiven Workflows. Gültige alte Arbeit erst nach vorhandener
   Drain-/Referenz-/Recoveryregel beenden, nicht durch numerische Umbenennung.
6. Die bisherigen Anweisungen „class-10/50 protected roots“ sind für künftige
   Arbeit auf Klasse 10 zu begrenzen. Orchestrator-/global-root-Schutz und
   Supervisorgrenzen bleiben bestehen. Kein breiter Regex über die Zahlen 10/50,
   keine Aufhebung von Operatorauth, Policy, Claimfencing oder gewöhnlichen
   Recipe-Q1/Q2-/S-Integritätsanforderungen beim Entfernen der Scaffoldrolle.

## 3. Durchführung dieses Plans: ausschließlich Dokument- und Planeingriffe

Die folgenden Schritte sind Dokumentarbeiten. Die in §4 beschriebenen späteren
Code-/DB-/UI-Aufträge werden bei ihrer Durchführung nur in bestehende Pläne
eingetragen und hier ausdrücklich nicht implementiert.

### D1 — Aktuelle Grenzen und alle Zielfundstellen vor dem Edit abgleichen

Die Dateien aus §§5–6 und ihre verlinkten aktuellen Ownerabschnitte lesen. Mit
`rg --files` und gezieltem `rg` zusätzlich alle aktiven Instanzen von
`skill-association-approval`, `exact-combination`, `combination approval`,
`new combination`, `reapproval`, `revalidation`, `Q1`, `Q2`, `backward compatibility`,
`Abwärtskompatibilität`, `approved closure`, `association_approvals` und den
Binding-/Preloaddefinitionen prüfen. Treffer semantisch unterscheiden: technische
Integrität, historische Evidenz und aktuelle Freigabepflichten sind verschieden.
Für jeden aktuellen normativen Treffer zusätzlich A/S unterscheiden. Keine
globale Ersetzung von „Kombinationsfreigabe“: S-Regeln erhalten und nur Scope/
Voraussetzungen nach §2.8 präzisieren; A-Anweisungen nach §2.9 auf Funktionsprüfung
umstellen. Historische Ergebnisse weder abschwächen noch zur neuen Freigabe erklären.

Den Statusindex, die zentrale Buildqueue und den aktuellen Abschlussstand je
Plan erneut lesen. Zum Auditzeitpunkt war BUILD-004 als Building dokumentiert;
BUILD-003/005/006 waren Passed und BUILD-002/007 Proposed. Das ist nur eine
Momentaufnahme, keine feste Reihenfolge oder neue Zuständigkeitsvergabe.
K3 und aktuelle ResultOnly-/Exactarbeiten werden durch diesen Plan nicht geändert.

Jede neue relevante Dokumentdatei bekommt eine eigene Zeile in §6. Keine
willkürliche Beschränkung auf 61 Dateien, kein pauschales Ändern aller Suchtreffer.
Vorhandene gleichwertige Nachfolger referenzieren statt doppelt beauftragen.
Vor jedem tatsächlichen Dokumentedit, auch in D2, Einbettung und aktuell
erfasste Buildinputs prüfen und den betroffenen Abschnitt frisch lesen. Die
Integrations-/Sourcefreigabe aus der Entwicklungspolicy gilt für alle Schritte,
nicht erst für D4. Alte Zeilenpositionen oder Inventarzahlen sind keine Preimages.

### D2 — Die vier verbindlichen Komponentenguides konsistent umstellen

Die Einträge 53/58/59/60 aus §5 zuerst als zusammenhängendes Textpaket bearbeiten.
Definitionen, Beziehungsschemata, Update-/Freigaberegeln, Beispiele, Fehlerfälle
und Checklisten auf §2 umstellen. Die Pflicht zur eigenständigen validierten
Komponente und zu ihrem Kompatibilitätsnachweis wird beim geänderten Component
verankert. A-Funktionsfreigabe und S-Seednutzungsqualifikation ausdrücklich trennen;
den exportierten Workflowprüfungsumfang hier nicht erneut beauftragen.
Für A keine zusätzliche Approvalebene durch anders benannte Closure-/Export-
oder Kompatibilitätsfreigabeaufträge einführen. Für S die bestehende genaue
Skillnutzungsqualifikation mit vorab freigegebenen Voraussetzungen erhalten.
Kein Seedapproval als Sammelfreigabe der Closure auslegen.

Istbeschreibungen existierender Formate/Reader und belegte Grenzen erhalten,
als Übergangsstand kennzeichnen und auf die künftigen bestehenden Owner aus §4
verweisen. Beispiele nicht in erfundene bereits akzeptierte JSONformate umschreiben.

### D3 — Nötige Umsetzung als Nachfolger in die bestehenden Pläne eintragen

§4 in die dort benannten Pläne übertragen, jeweils mit konkreter Arbeit,
Abhängigkeiten und Abnahmekriterien. Ausschließlich neue offene Folgeabschnitte
**hinter dem bei Durchführung tatsächlich laufenden Abschnitt** ergänzen.
K0–K3, abgeschlossene Unterpakete und aktuell implementierte Schritte bleiben
inhaltlich unverändert. Eine alte Fundstelle ist keine Editfreigabe.

Bei fehlender verlässlicher Einzelstepmarkierung keinen Status erfinden:
im bestehenden Plan einen expliziten offenen Nachfolger nach seinem derzeitigen
Implementierungspaket verorten und dessen bestehende spätere Aktivierungs-/
Cutovergates um die neue Pflicht ergänzen, sofern diese Gates noch nicht laufen.
Ist auch das Gate schon abgeschlossen/laufend, den nächsten offenen Folgeabschnitt
desselben Owners benutzen. Kein K8, kein neuer Validatorplan, keine zweite Runtime.

D3 überträgt die konkreten Ergänzungen aus §§2.2.1, 2.7–2.10 und §12 in die
zugeordneten U-Aufträge, nicht nur einen Link auf diesen Audit. U2 besitzt Subject/
Q2, U3 Publikation, U4 Reader, U6 Seedvoraussetzungen, U11/W-R Scaffold. Bereits
abgeschlossene Bausteine aus §12 werden als Eingänge referenziert, nicht neu gebaut.

Für jeden Auftrag eine Eingangskante vom bisherigen Paket und eine Ausgangskante
zum nächsten betroffenen offenen Aktivierungs-/Cutovergate notieren. Ein bloßer
Anhang „später berücksichtigen“ ohne konsumierendes Gate genügt nicht. Unabhängige
laufende Arbeit wird nicht von der vollständigen Dokumentumstellung abhängig.
Die neue A-Funktionsfreigabe und die präzisierten S-Voraussetzungen werden
erst nach ihrem angeschlossenen Implementierungs-/Migrationsnachweis als
unterstützt ausgewiesen; Seedkombinationsfreiheit ist kein Ziel.

### D4 — Architektur, Betriebsverträge und Authoringtexte angleichen

Die restlichen aktuellen Zielfundstellen aus §§5–6 lokal bearbeiten. Spiegel
gleichhalten; Rollen und bestehende technische Guards nicht umschreiben. Vor
Änderungen an eingebetteten Dokumenten tatsächliche `include_str!`- und Release-
Inventarconsumer prüfen. Während eines fremden Builds keine erfassten Inputs
ändern. Unembedded Plantext bleibt unabhängig bearbeitbar.

Format- und Migrationshistorie bleibt erhalten. Veraltete normative Textpassagen
werden für neue Arbeit konkret ersetzt bzw. in historischen Plänen durch einen
lokal klar bezeichneten künftigen Nachfolger abgelöst. Keine alten Passbelege
umetikettieren und keine aktuelle Runtimefähigkeit aus einer neuen Formulierung
ableiten. Nötige eingebettete Source-/Inventar-/Seedänderungen nur in die Ownerpläne
eintragen; in dieser Durchführung keine ausführbaren oder Manifestquellen ändern.

### D5 — Konsistenz und Abschluss der Dokumentlieferung prüfen

Jede Inventarzeile mit durchgeführt, unverändert mit Begründung oder offenem
konkretem Editziel dispositionieren. Links/Anker, Klassenrollen, fachliche Beispiele,
Spiegel und die neuen Eingang-/Ausgangskanten prüfen. Die semantische Resttreffersuche
aus D1 wiederholen: aktuelle Kombinationspflichten müssen S zugeordnet sein;
alte A-Kombinationspflichten dürfen nur klar historischen bzw. noch nicht
umgestellten Iststand beschreiben. S-Nutzungsbelege nicht versehentlich entfernen.
Ein offenes notwendiges Dokumentedit ist kein abgeschlossener Dokumentabgleich.
„Unverändert“ ist nur bei belegter bereits passender Aussage, nicht betroffener
Zuständigkeit oder geschützter Historie ein gültiger Abschluss. Die spätere
technische Umsetzung darf offen bleiben, wenn ihr konkreter Auftrag mit
Ein-/Ausgangskante im zuständigen bestehenden Plan tatsächlich eingetragen ist.

Scoped Diff und `git diff --check` ausführen. Keine Cargoausführung nur für diesen
Plan; notwendige Paket-/Runtimeprüfungen gehören zu den eingetragenen späteren
Owneraufträgen. Dokumentlieferung, Planeintragung, implementierter Support und
Aktivierung als getrennte Ergebnisse berichten. Nichts automatisch stagen,
committen, deployen oder anderen Chats ohne Betreiberautorisierung zusenden.

## 4. Einzutragende Umsetzung bei bestehenden Ownern

**Alle folgenden Aufträge sind Textinhalte für bestehende Pläne.** Ihre technische
Ausführung gehört ausdrücklich nicht zur Durchführung dieses Dokuments.
Die K-/Stepnummern dienen der fachlichen Zuordnung; D3 schützt neu abgeschlossene
oder gerade laufende Teile und bestimmt die tatsächliche Nachfolgerposition.
Die Schritte zur Q1-Optimierung aus §2.9 sind verpflichtender Inhalt von U1/U2,
nicht ein später optionaler Optimierungsauftrag. U6 präzisiert lediglich S-Scope/
Vorbedingungen und ergänzt belegte Lücken, ersetzt aber nicht die vorhandene
Seedarchitektur. Die neuen A-Writer dürfen S weder deaktivieren noch neu definieren.

| Auftrag | Existierender Plan und zulässige Position | Einzutragende spätere Arbeit und verbindliche Ausgangskante |
| --- | --- | --- |
| U1 A-Funktionsreview und Nachfolgerkompatibilität | [validator_v3.md](../../validator_v3.md), neuer offener Nachfolger nach dem laufenden Validatorpaket; fachlich bestehende Steps 2/3/7/8/9/10 | Die vorhandenen Klassenrouten nach §2.9 optimieren: eigene Tool-/ToolSkill-/Purefunktion, gemeinsamer Skill-/Codeprüfgegenstand mit beiden exakten Revisionen, einmalige vollständige Regel-/Behaviorabdeckung und bestehende vorab gültige Voraussetzungen. Keine allgemeine A-Kombinationenmatrix oder zusätzliche Associationfreigabe. Benötigte Altverträge/Kompatibilität bleiben geprüft. Ausgang: offene Validator-Steps 10–12/Produktgates konsumieren A; S wird nur nach U6/§2.8 qualifiziert. |
| U2 A-Funktionsbelege und getrennte S-Provenienz | Derselbe Validator-Nachfolger, bestehende Review-/Storeowner und [pure-component-review](../reborn/pure-component-review.md) | Bestehende sealed Subjects, Submission-/Evidencewriter und Q2-Facade wiederverwenden. Koordiniertes Skill-/Codesubject und eigener Freigabescope der geänderten Revisionen statt zusätzlichem A-Kombinationsbeleg; keine doppelte Klassen-22-Prüfung im Skillreview. S behält genaue skill-association-approval/1-Belege sowie separat nachgewiesene Voraussetzungsfreigaben. Formate/Reader bewusst unterscheiden, historische Bytes erhalten. Ausgang: U3/U4/U6 und vorhandener Validatorcutover. |
| U3 Herkunftsrichtige Katalogaktivierung | [simplified_v3.md](../../simplified_v3.md), offener Nachfolger ab K4 nach aktuellem K3-/Admissionpaket; vorhandene Phase-0a/3a- und §8.2-Owner | Ein vorhandener Katalogowner veröffentlicht kohärent. A: eigene Funktionsfreigaben und Nachfolgerkompatibilität, kein neuer Subject-/Freigabeauftrag unveränderter Consumer. S: zusätzlich exakten Skillnutzungsbeleg und vorherige unabhängige Tool-/ToolSkillfreigaben prüfen. Aktuellen Bedarf und geplanten Graph gegen denselben Snapshot/CAS abgleichen; alte Tasks behalten ihre Pins. Ausgang: offene K4/K6/K7-/reguläre Cutovergates. |
| U4 A-Interfacekompatibilität und modusbewusste Reader | [monty.composition.md](monty.composition.md), neuer offener Nachfolger hinter aktueller Familie; fachlich bestehende offene Identitäts-/Familien-/Deploymentarbeit mit Implementierungsanhängen | Technische Binding-/Exportidentität beibehalten. Nur A: exakte Schema-/Closureapproval-Abhängigkeit durch gerichteten Interfacevertrag und eigene Funktionsfreigaben ersetzen. S: exakte Skillnutzungs-/Artefakt-/Approvalzuordnung weiter prüfen. Kein global gelockerter JSONreader, keine Aufweichung der S-Exaktbindung. Gemeinsame Skill-/Codeänderung ohne konkurrierende Freigabe-/Publikationswriter unterstützen. Ausgang: U3 und offene Familien-/Deploymentgates. |
| U5 Freigabe-/Kompatibilitätsmigration der Bibliothek | Composition und seine vier bestehenden Anhänge, jeweils offener Nachfolger nach der laufenden Familie | Jeden Kandidaten seiner tatsächlichen Herkunft zuordnen. A-Funktionsreviews und kompatible Nachfolger an unveränderten Consumern ohne neue Genehmigung nachweisen. S-Seednutzungen behalten genaue Qualifikation und vorherige Voraussetzungsfreigaben; keine universelle Umschreibung bestehender Seedpakete. Recipeprüfungsumfang und ToolSkillmehrfachnutzung bleiben im exportierten Plan. Ausgang: nächste offene betroffene Familienabnahme. |
| U6 S-Qualifikationsreihenfolge und A/S-Upgrade | Simplifieds noch offene K5/K6 bzw. folgende Phase-6/8-Arbeit, Validator Step 11 als offener Nachfolger | Bestehende Installation-/Bootstrap-/Revision-/Reviewowner verwenden. Bereits qualifiziertes Tool und danach gültigen ToolSkill vor gemeinsamer Skill-/Codeprüfung nach §2.8 nachweisen; fehlende einzelne Voraussetzungsbelege gezielt ergänzen. Exakter S-Nutzungsbeleg genehmigt nicht alle gelisteten Components. S bleibt außerhalb normaler authored Queue, ohne neues installiertes/externes Q2. A-Formatnachfolger getrennt migrieren; Overrides, alte Artefakte und Belegbytes erhalten. Ausgang: offene Validator-Step-11-/K5/K6-/Phase-6/8-Gates. |
| U7 Herkunftsrichtige Lehrquellen und Paketinputs | [prefix_v3_upgrade.md](prefix_v3_upgrade.md), neuer offener Nachfolger nach laufendem Sources-/Exactconsumer; anschließende offene Source-, Teaching-, Paket-/Servingabnahme | Neue Guide-/Teachingrevisionen unterscheiden A-Funktionsvalidierung und S-Seednutzungsqualifikation ausdrücklich. Beim vorhandenen Prefix-/Sourceowner erfassen und nur wirklich betroffene Paketinputs neu qualifizieren. Kein Entfernen bestehender S-Belege, keine B1/B2-Byteänderung oder allgemeine Freeaktivierung. Ausgang: offene Source-/Teaching-/Paketgates. |
| U8 A/S-Vertrag in v3.5-Verbrauchern | [sempai_v3.5.md](sempai_v3.5.md), [simplify_connections_v3.5.md](simplify_connections_v3.5.md) und [path_to_v3.5.md](../../path_to_v3.5.md), jeweils nach ihrem aktuellen Paket | Sempai/Operator erzeugen A-Kandidaten mit eigener Funktionsprüfung und gemeinsamem Skill-/Codegegenstand, nicht Kombinationsreviewkaskaden. Die shipped Sempai-Bibliothek in Step 10 bleibt dagegen S mit genauer Nutzungskombination. Connections Phase F und V35-03/04/06/07/16/17–20/27/30 konsumieren denselben Vertrag; keine zweite Prüfqueue oder Evidenceengine. Ausgang: nächste offene v3.5-Familien-/Produktgates. |
| U9 A/S- und Funktionsscope in UI/API | Validator Steps 9/12 und Simplified §8.2/Composition-MCP im jeweiligen offenen Nachfolger | UI zeigt Herkunft, geänderte eigene Revisionen, bei Skill gemeinsam ausgewählten Code und vorab gültige Voraussetzungen. A-Q2 umfasst denselben Funktionsprüfgegenstand ohne zusätzliche Kombinationsrunde; S zeigt exakten Nutzungsbeleg und Bootstrapprovenienz ohne installierten Q2-Button. Publikation/Taskpins bleiben getrennt. Ausgang: bestehende offene Operator-/MCP-/Produktgates. |
| U10 Referenzen, Retention und Recovery | Simplified Phase 3a/6/8, Composition und Prefix-GC beim nächsten offenen Folgepaket | Vorhandene sichere Referenz-/Löschmechanik pro Artefaktklasse nachweisen und wiederverwenden, fehlende Unterstützung dort ergänzen. Task-/Wait-/Child-/Recoverypins sowie noch benötigte Originale für Review/Integrität schützen; Vertragslineage und Audit getrennt vom freigebbaren Payload erhalten. Kein pauschal als fertig angenommener Komponenten-GC und keine zweite GC-Infrastruktur. Geänderte Toolpolicy bleibt live, keine Whole-Recipe-Replays oder Versionssubstitution nach Crash. Ausgang: vorhandene offene Restart-/Restore-/GC-/Plattformabnahme. |
| U11 Klasse-50-Sondervalidierung entfernen | Bestehender Validator-/Klassenschemaowner im nächsten offenen Nachfolger; fachliche Recipezuordnung ausschließlich beim W-R-Owner des zweiten Exactplans | Keine Scaffold-Q1/Q2-/protected-root-/Bootstraproute implementieren. Aktuelle Klassendefinitionen, Submission-/Queue-/Freigabe- und UIregeln auf §2.10 umstellen; Klasse 10 getrennt schützen. Alte Formate unverändert lesbar halten, neue Klasse-50-Erstellung/Aktivierung im geplanten Cutover beenden. U11 liefert Validierungs-/Trustregeln an W-R, kein zweiter Migrationswriter. Ausgang: betroffene offene Validator-/Composition-/Upgradegates, keine neue neunte Klassenroute. |

### 4.1 Pflichtfälle, die in die existierenden Abnahmematrizen einzutragen sind

Die folgenden Fälle gehören zu U1–U11, nicht zu einem neuen Testframework und
nicht zu auszuführenden Schritten dieses Plans. Vorhandene passende Belege werden
mit ihrer tatsächlichen Reichweite wiederverwendet.
Die Anzahl beispielhafter Skills, Recipes und Testwerte ist keine neue
Produktkapazität oder Freigabeschwelle. Für Ausführung, Graphen und Evidenz gelten
die vorhandenen anerkannten technischen Limits; der Review darf bei Überschreitung
nichts abschneiden. Mehrere Anforderungen dürfen durch denselben geeigneten
Callerfall abgedeckt werden. Kein nur für Tests erfundenes Tool und kein
Fakeadapter ersetzt den tatsächlichen vorhandenen Freigabe-/Katalogpfad.

| Fall | Erwartung und bestehender Owner |
| --- | --- |
| Zwei genehmigte Recipes verwenden dieselbe Skill-UUID | Kompatibler genehmigter Skillnachfolger wird einmal aktiviert. Beide neuen Tasks nutzen ihn automatisch; Recipebytes, -revisionen und -freigaben bleiben gleich. Aktivierung/Composition. |
| Unveränderter Zwischenconsumer | Helper/ToolSkillupdate löst auch transitiv keine neue Freigabe eines unveränderten Skills oder PythonCode aus. Nachfolgerprüfung dokumentiert kompatible Altverträge. Validator. |
| Wartender alter Task plus neuer Task | Alter Task/Child/Resume verwendet alte Exporte/Artefakte, neuer Task die neue genehmigte Auswahl; keine spätere Latestauflösung. Global/Composition. |
| Brechendes Pflichtargument oder entfernte Funktion | Nachfolger unter gleicher UUID wird vor Aktivierung abgelehnt; vorhandene Consumers bleiben genehmigt und unverändert. Validator/Aktivierung. |
| Gleiches Schema, andere Bedeutung/Effekte | Kompatibilitätsprüfung darf nicht allein wegen Typgleichheit bestehen; semantische/behaviorale Nachweise am Nachfolger. Validator. |
| Neues Ergebnisfeld bei strict consumer | Kein pauschales „additiv ist kompatibel“; nur mit belegter Verträglichkeit oder erhaltenem alten Ergebnisprofil. Validator/Composition. |
| Alte nicht geroutete, aber gültige Abhängigkeit | Routingdisable entfernt keinen Kompatibilitätsbedarf. Stilllegung ist explizit und referenzgeprüft. Katalog/Validator. |
| Fehlendes/gefälschtes/stales Kompatibilitäts- oder Komponentenapproval | Aktivierung bleibt ohne neue Nebenwirkungen aus; kein neues Consumer-Q2, kein Downgrade, kein Tier-2-Replay. Validator/Katalog. |
| Reine Komponente mit mehreren Consumern | Eigenständige Pure-Freigabe; kompatibles Update ohne Closure-Neugenehmigung unveränderter Verbraucher. Pure-Review/Composition. |
| Component-/Recipeaktivierung konkurriert | Eine kohärente Generation oder ausdrücklicher CAS-Konflikt; keine gemischten Versionen und keine Approvalkopien als Reparatur. Katalog/PG. |
| Consumerneuzugang zwischen Review und Veröffentlichung | Bereits nachgewiesene Garantien genügen ohne neues Review; ein nicht abgedeckter benötigter Vertrag verhindert ausschließlich die betroffene neue Veröffentlichung. Kein Stale-Snapshot-Pass und kein Consumer-Q2. Validator/Katalog. |
| Mehrere einzeln genehmigte Nachfolger in einer Veröffentlichung | Vollständiger geplanter Graph erfüllt auch deklarierte Beziehungen zwischen Abhängigkeiten; ein Konflikt bleibt Fehler ohne zusätzliche semantische Kombinationsfreigabe. Katalog/Composition. |
| Alte Pflicht bleibt über mehrere Nachfolger erhalten | Benötigte Garantie ist über überprüfbare Lineage erhalten; unterbrochene oder entfernte Garantie wird nicht transitiv als kompatibel akzeptiert. Validator/Katalog. |
| Optionaler Codeinput bei unveränderter alter Skillassociation | Alte gültige Export-/Bindingaufrufe funktionieren im neuen Reader ohne neue Skillrevision; neues Inputprofil bleibt streng geprüft. Geänderte Recordbytes/gefälschte Zuordnung scheitern weiterhin. Composition/Validator. |
| Unvalidierter Entwurf oder nur historischer Consumer | Entwurf blockiert kein sonst kompatibles Update; er muss vor eigener Aktivierung passen. Alter Task nutzt seinen alten Graphen, nicht den neuen Anbieter. Validator/Katalog. |
| Alte Formate, Unknown Commit, Backup/Restore | Originalbytes/IDs behalten; gültige Alt-Tasks lesbar, keine erfundene Migration zu neuer Freigabesemantik und keine Wiederholung fertiger Effekte. Upgrade/Recovery. |
| Neuer Beleg mit falschem alten Reader bzw. umgekehrt | Versionierte Zuordnung wird geprüft, kein übersprungenes Approval durch Featureflag oder unbekanntes Format. Gültiges Altprofil bleibt unterstützt; nicht gedeckter neuer Pfad bleibt unaktiviert. Review/Upgrade. |
| Wechsel des Validatorprogramms nach eigener Komponentengenehmigung | Unveränderte eigene Freigabe bleibt gültig; exakte damalige Validator-/Interpreter-/Belegprovenienz bleibt lesbar. Explizite Revocation ist eine getrennte begründete Operation. Validator/Upgrade. |
| First-party Nachfolger und authored Edit | Seed: nichtzirkuläre Integrität/Q1/Behavior ohne Instanz-Q2. Authored: eigene Q1/Behavior/human Q2. Keine Quellenlabelabkürzung. Installation/Validator. |
| Neuer authored Skill und neuer Code | Ein vollständiger gemeinsamer Funktionsreview über beide echten Revisionen, ohne vorausgesetzte gegenseitige Freigabe; keine zweite vollständige class22-/Skillprüfung und kein separater authored Kombinationsbeleg. Bestehender Validator-/Submission-/Q2-Owner. |
| Nur Skill oder nur zugehöriger Code geändert | Gemeinsame Funktionssicht und tatsächlicher Aufruf geprüft; nur geänderte Revision bekommt neue eigene Freigabe, unverändertes Gegenstück sowie Tools/ToolSkills behalten ihre Belege. Keine Reviewkaskade. |
| Authored Skill verwendet gültiges Tool falsch | Falsche Parameter, falsche Ergebnisannahme oder verschluckter Toolfehler lassen die Skill-/Codefunktion scheitern; Tool-/ToolSkillfreigaben werden weder geändert noch erneut geprüft. |
| Fehlende genehmigte Voraussetzung | A-Funktionskandidat bleibt wegen ausstehender Voraussetzung unvollständig; kein Kombinationstest ersetzt Tool-/ToolSkillfreigabe und kein impliziter Dependency-Approve. |
| Seedqualifikation pro Skillnutzung | Vorherige eigene Toolfreigabe, danach ToolSkillfreigabe, danach gemeinsame Skill-/Codeprüfung samt exaktem Nutzungsbeleg. Derselbe gültige Tool/ToolSkill in zweiter Seednutzung erhält keine neue eigene Freigabe. |
| Erfolgreicher Seedkombinationstest bei ungeprüftem Tool | Keine Freigabe: der Nutzungsbeleg kann fehlende unabhängige Tool-/ToolSkillvoraussetzungen nicht ersetzen. Nur betroffene Lücke beim vorhandenen Bootstrapowner schließen. |
| Prüfung vor Erzeugung des abhängigen Skills | Tool-/ToolSkill-Prüfung gelingt über bestehende vertrauenswürdige native Prüf-/Bindingadapter, ohne vorherigen eigenen Skill zu verlangen. Spätere Skill-/Codeprüfung kann ihre beiden Drafts gemeinsam prüfen; keine Abhängigkeits- oder Genehmigungsschleife. |
| Scaffoldlabel ohne tatsächlichen Bestand | Kein neuer Validator, keine künstlichen Klasse-50-Subjects oder Dummyrecipes. Aktive Klassenerstellung entfällt; historische Dokumente bleiben als solche lesbar. |
| Neuer Scaffold-Erstellungs-/Freigabeversuch | Kein Klasse-50-Funktions-/Rootreview wird erzeugt. Vorhandenes konkretes Recipe verwenden oder dessen normale eigene Erstellung/Prüfung; reine Navigation ist kein neuer ausführbarer Alias. |
| Historische Klasse-50-Referenz und Orchestrator | Unveränderte alte Bytes/Pins bleiben lesbar; zukünftige Consumer werden explizit auf validierte Recipeidentität umgestellt. Klasse-10-/global-root-Schutz bleibt erhalten, keine Autoritätsvererbung an das Recipe. |
| Echter S-Nachfolger versus installierter Edit desselben UUIDs | Shipped Nachfolger behält S-Provenienz/Exaktnutzungsqualifikation; Operator-/Sempaiänderung ist A mit eigener Funktionsprüfung. Kein Schalter kann A als S oder S als authored-Humanpass ausgeben. |
| Q1-Evidenzwiederverwendung | Identische immutable Quellen/Regeln/Interpreter-/Inputscopes können gleiche Fakten nutzen; neue relevante Quelle/Regel/Unvollständigkeit nicht aus Erfolgscache übernehmen. Keine doppelten Skill-/Codevollprüfungen, keine ausgelassenen Pflichtfälle. |
| Optionaler Outputwert und alter gültiger Fallback | Zwei verschiedene positive Werte durch echten Producer/PG/SDK/Guard/Child verfolgen, auch bei deaktivierten Tokenbudgets. Missing erhält zulässigen alten Wert; null/zero/ungültig werden abgelehnt. Bestehender Exact-/Modellowner. |
| I1 ohne reale zugelassene Eingabe | Keine Aktivierung, kein Fake-Custodian-/Getterbeweis, keine allgemeine Bootstrapfreigabe. Exact I1/I2. |
| Live Toolblock nach Versionsaktivierung | Neue und alte Taskauswahl beachten die aktuelle Sperre; Komponentengenehmigung erteilt keine Toolberechtigung. Kernel/Runtime. |
| MCPrefresh und geänderter Commandvertrag | Kompatible Implementierung aktualisiert technische Auswahl ohne neues Recipe-Q2; ein tatsächlich geänderter Commandvertrag wird als eigene Änderung qualifiziert. MCP/Composition. |
| GC konkurriert mit neuem Pin und benötigt weiter Vertragslineage | Auf vorhandener Referenzmechanik kein Pin nach irreversibler Freigabe; Review-/Recoveryoriginale und kompatibilitätsrelevante Metadaten bleiben geschützt. Historische Belege erlauben keinen neuen Altversionstask. Retention/Recovery. |

### 4.2 Eindeutige Ownership und verbundene Lieferreihenfolge

U1/U2 definieren Freigabescope, Reviewanforderungen und vertrauenswürdige
Belegsemantik für A beim bestehenden Validatorowner. Dazu gehören gemeinsames
Skill-/Codesubject, wiederverwendete Klassen-22-Teilchecks und ein Review-/Q2-Lauf
für die koordinierten geänderten Revisionen. S verbleibt beim bestehenden
Installation-/Bootstrapowner; seine Nutzungsbelege werden nach §2.8 gelesen.
Der A-Validator nutzt für die Prüfung den
bereits qualifizierten privaten Runtime-/Containedpfad; er setzt nicht die erst
zu genehmigende neue Komponente als ihren eigenen Prüfer ein. Der vorhandene
Installationsowner qualifiziert notwendige shipped Validatornachfolger
nichtzirkulär. Kein gegenseitiges Warten neuer Katalog-/Validatorimplementierungen.

U3 besitzt die atomare Veröffentlichung/Generation und ihre native Transaktions-
mechanik. U4 liefert die Graph-/Interface-/Exportauflösung und die Verbindung zum
bisherigen Deploymentport. U4 erzeugt keinen eigenen Activator; U3 erzeugt keinen
eigenen fachlichen Validator. U6 integriert ihre vereinbarten Formate und die
Storemigration mit den jeweiligen bestehenden Review-/Katalog-/Installationsownern.
Das sind Teilbeiträge zu einem gemeinsamen Cutover, keine parallelen Writer für
denselben Datensatz oder nebeneinander gültige neue Approvalsysteme.

Vor technischer Freigabe dieses Cutovers müssen die bestehenden Consumer für
Prepare, Activation, Serialize/Restore, Taskcapture, private Validatoradmission,
Child/Wait/Recovery, Prefixquellen und MCP die jeweilige Format-/Freigabesemantik
konsistent lesen. Die konkrete Dateiclosure wird aus aktuellen Callern bestimmt;
insbesondere die alten `association_approvals`-Maps und exakten Inheritancechecks
dürfen nicht unbeachtet bleiben. Es entsteht kein obligatorischer Vollneubau
dieser bereits vorhandenen Reader. Änderungen und Tests werden nur für ihren
nachgewiesenen Anteil benötigt.

U5/U7/U8/U9 konsumieren diese gemeinsame Freigabesemantik in ihren vorhandenen
Familien-/Produktgates; sie führen dieselbe Komponentenvalidierung nicht nochmals
ein. U10 liefert erforderliche Referenzsicherheit vor der ersten betroffenen
physischen Löschung. Fehlender optionaler GC darf einen sicheren, vollständig
aufbewahrenden Updatepfad nicht blockieren. Für jeden konkret neuen Ausführungspfad
bleiben dagegen fehlende Pflichtreader oder Belege ein Aktivierungshindernis.

## 5. Dateiweise Schritte für die 61 bisherigen Dokumente

**Leseschlüssel:** Jede Zeile ist ein eigener Dokumentänderungs-/Prüfschritt und
nennt das konkrete lokale Ziel. „U…“ bezeichnet ausschließlich den einzutragenden
Auftrag aus §4. In allen folgenden Dateizeilen ist der aktive Änderungsscope
vorangestellt: **A** bezeichnet nur Instanzänderungen. Aussagen wie „keine
Kombinationsfreigabe“ oder „ersetzen“ dürfen nie auf den gültigen Seedpfad S
angewandt werden. In jeder tatsächlich geänderten aktuellen normativen Passage
beide Wege unmittelbar unterscheiden; die S-spezifischen lokalen Ergänzungen
stehen zusätzlich in §10. Ein bloßer entfernter Herkunftsbanner genügt nicht. Für Plan- und Journaldateien gilt immer D3: alte abgeschlossene
und aktuell laufende Abschnitte bleiben erhalten, Änderungen nur im offenen
Nachfolger. Alle Pfade sind relativ zur Repositorywurzel; die Links lösen vom
Standort dieses Dokuments auf.

| Nr. | Datei | Konkreter Dokumenteingriff / Abnahme der Zeile |
| --- | --- | --- |
| 01 | [.agents/skills/reborn-feature/SKILL.md](../../.agents/skills/reborn-feature/SKILL.md) | **A; S-Erhalt nach §10:** Entwicklungsrouting auf A-Funktionsreview und vorhandenen S-Bootstrapowner aufteilen; Skill und Code bilden einen eigenen Prüfgegenstand, nicht zwei volle Reviews. Spiegel 03 identisch halten. |
| 02 | [.claude/rules/skills.md](../../.claude/rules/skills.md) | **A; S-Erhalt nach §10:** Aktuelle Authoring-/Approvalregel und Link auf exakte Associationfreigabe auf individuelle Freigabe und Nachfolgerpflicht umstellen; alte Dateibundleloader bleiben ausgeschlossen. |
| 03 | [.claude/skills/reborn-feature/SKILL.md](../../.claude/skills/reborn-feature/SKILL.md) | **A; S-Erhalt nach §10:** Gleichen Dokumentdelta wie 01 anwenden, Metadaten/Invocationpolicy erhalten und Bytegleichheit der Spiegel prüfen. |
| 04 | [AGENTS.md](../../AGENTS.md) | **A; S-Erhalt nach §10:** Checks 3–5, „Review meaning“ und Bindingblöcke lokal nach A/S aufteilen. A-Funktionen einschließlich gemeinsamem Skill/Code ohne extra Kombinationsgate; S-Kombinationsbelege mit früheren Tool-/ToolSkillfreigaben bleiben. Kein Seedlabel als Einzelcomponentfreigabe. |
| 05 | [CLAUDE.md](../../CLAUDE.md) | **A; S-Erhalt nach §10:** Gleiche unmittelbare A/S-Unterscheidung wie 04 in aktiven Beispielen/Regeln; Skill-/Codeprüfung einmal gemeinsam, S-Closurebeleg nicht als Freigabe aller Dependencies lehren. |
| 06 | [README.md](../../README.md) | **A; S-Erhalt nach §10:** Binding-Preloadübersicht um eigenständige Gültigkeit und kompatible Nachfolger ergänzen; neueste aktive Auswahl/alte Taskpins beibehalten, keine vollständige Umsetzung behaupten. |
| 07 | [crates/AGENTS.md](../../crates/AGENTS.md) | **A; S-Erhalt nach §10:** Gemeinsamen Komponenten-/Preloadblock auf §2 routen; Review-/Storeowner von mechanischer Kernel-/Hostprüfung trennen. |
| 08 | [agent_loop/AGENTS.md](../../crates/brassclaw_agent_loop/AGENTS.md) | **A; S-Erhalt nach §10:** Im Bindingblock kompatible neue Auswahl bei neuen Tasks, feste alte Auswahl und keine neue Loopfreigabe pro Componentupdate erklären; Legacy-Rustloop nicht ausbauen. |
| 09 | [agent_loop/CLAUDE.md](../../crates/brassclaw_agent_loop/CLAUDE.md) | **A; S-Erhalt nach §10:** Gleiche aktuelle Auswahl-/Freigabepräzisierung wie 08; laufender Loop-/Effectcustodyvertrag unverändert. |
| 10 | [engine/AGENTS.md](../../crates/brassclaw_engine/AGENTS.md) | **A; S-Erhalt nach §10:** Retained Binding/Export/Approvalpins als technische Auswahl beschreiben; kein Approval jeder Versionskombination als Runneraufgabe. U3/U4 statt neuem Resolverowner nennen. |
| 11 | [engine/CLAUDE.md](../../crates/brassclaw_engine/CLAUDE.md) | **A; S-Erhalt nach §10:** Preload-/Assemblyhinweise an 10 angleichen; strukturelle Laufzeitprüfung von Nachfolger-Q1 abgrenzen, alte Manifestleser als Istgrenze erhalten. |
| 12 | [engine/MONTY.md](../../crates/brassclaw_engine/MONTY.md) | **A; S-Erhalt nach §10:** Aktuellen Preloadkopf ergänzen: exakte Test-/Taskidentität ist keine Reapprovalpflicht. Interpreter-/ABIhistorie und Control-Pin nicht umschreiben. |
| 13 | [extensions/AGENTS.md](../../crates/brassclaw_extensions/AGENTS.md) | **A; S-Erhalt nach §10:** Im Bindingkopf Erweiterungsinstallation von eigener Komponentenfreigabe trennen; keine zusätzliche Kombinationsapprovalebene. Änderungen der gemeinsamen ToolSkillnutzung ausschließlich im neuen Plan. |
| 14 | [extensions/CLAUDE.md](../../crates/brassclaw_extensions/CLAUDE.md) | **A; S-Erhalt nach §10:** Eigene Komponentenfreigaben und Nachfolgerkompatibilität aufnehmen; technische Registrierungs-/Hostgrenzen erhalten. Kardinalitätsänderungen sind ausgelagert. |
| 15 | [host_runtime/AGENTS.md](../../crates/brassclaw_host_runtime/AGENTS.md) | **A; S-Erhalt nach §10:** Native Bindings prüfen weiterhin Callable/Adapter/Artefakt und live Policy; Q1-/Q2-Kaskaden ausdrücklich beim Komponentennachfolger ausschließen. |
| 16 | [host_runtime/CLAUDE.md](../../crates/brassclaw_host_runtime/CLAUDE.md) | **A; S-Erhalt nach §10:** Gleiche Trennung wie 15; kein Host-getter, Metadatum oder Bindingaufruf gilt als fachlicher Kompatibilitätsnachweis. |
| 17 | [pg/AGENTS.md](../../crates/brassclaw_pg/AGENTS.md) | **A; S-Erhalt nach §10:** Unveränderliche Altbelege und versionierte Nachfolgerleser erklären; Datenbanktransaktion ist keine neue Freigabe unveränderter Consumer. U2/U6 konsumieren. |
| 18 | [product_workflow/AGENTS.md](../../crates/brassclaw_product_workflow/AGENTS.md) | **A; S-Erhalt nach §10:** Produktports zeigen individuelle Komponentengültigkeit und Aktivierungsstand; keine neue Produkt-Approvalengine oder Reviewkaskade. |
| 19 | [product_workflow/CLAUDE.md](../../crates/brassclaw_product_workflow/CLAUDE.md) | **A; S-Erhalt nach §10:** 18 für Facade-/Deploymentports konkretisieren: kompatibler Dependencywechsel benötigt keine erneute Recipeeinreichung; aktuelle vollständige-Recipe-Grenze als U4-Gap benennen. |
| 20 | [reborn/AGENTS.md](../../crates/brassclaw_reborn/AGENTS.md) | **A; S-Erhalt nach §10:** Driver/Gateway konsumiert kohärente individuelle Gültigkeit und alte Pins; keine neue Modell-/Taskfreigabe bei kompatiblem Componentupdate. |
| 21 | [reborn/CLAUDE.md](../../crates/brassclaw_reborn/CLAUDE.md) | **A; S-Erhalt nach §10:** Gleiche aktuelle Regeln wie 20; live technische Guards und ursprüngliche Responsecustody bleiben, keine permissive Generalbootstrapregel. |
| 22 | [reborn_cli/AGENTS.md](../../crates/brassclaw_reborn_cli/AGENTS.md) | **A; S-Erhalt nach §10:** Komponentenupdate und Operatorverwaltung auf neue Guides routen; keine Kaskadenprüfung oder zusätzliche Runtime für Managementbefehle. |
| 23 | [composition/AGENTS.md](../../crates/brassclaw_reborn_composition/AGENTS.md) | **A; S-Erhalt nach §10:** Binding-/Recipe-Routingblock: technische Association bleibt, zusätzliche Closuregenehmigung entfällt im Ziel; aktuelle Readergrenze mit U3/U4/U6 verknüpfen. |
| 24 | [composition/CLAUDE.md](../../crates/brassclaw_reborn_composition/CLAUDE.md) | **A; S-Erhalt nach §10:** Katalog-/Deployment-/MCPowner präzisieren: geteilte Nachfolger konsistent veröffentlichen, unveränderte Recipefreigaben wiederverwenden. Keine bereits implementierte Unterstützung behaupten. |
| 25 | [webui_ingress/AGENTS.md](../../crates/brassclaw_reborn_webui_ingress/AGENTS.md) | **A; S-Erhalt nach §10:** Authentische eigene Q2-Entscheidung vom technischen Komponentenaktivierungsrequest trennen; kein per-Consumer-Q2 bei Nachfolgeraktivierung. |
| 26 | [webui_ingress/CLAUDE.md](../../crates/brassclaw_reborn_webui_ingress/CLAUDE.md) | **A; S-Erhalt nach §10:** Gleiche UI-/APIsemantik wie 25; Operatorauth, Origin/CSRF, Bodylimits und immutable Unknown-Commit-Behandlung erhalten. |
| 27 | [skills/AGENTS.md](../../crates/brassclaw_skills/AGENTS.md) | **A; S-Erhalt nach §10:** Skill-/Codefunktion als gemeinsam geprüfte Einheit mit getrennten immutable IDs erklären. Bereits gültige Tool-/ToolSkillabhängigkeiten nicht neu testen. S-separate Voraussetzungsfreigaben und genaue Nutzungsbelege erhalten. |
| 28 | [turns/AGENTS.md](../../crates/brassclaw_turns/AGENTS.md) | **A; S-Erhalt nach §10:** Bindingkopf auf §2 aktualisieren; retained Task-/Child-/Waitauswahl bleibt, ein neuer genehmigter Component löst keine neue Turnfreigabe aus. |
| 29 | [turns/CLAUDE.md](../../crates/brassclaw_turns/CLAUDE.md) | **A; S-Erhalt nach §10:** 28 angleichen; Claimfencing, Originalinput und Taskcompletion weiterhin unabhängig von Komponentenvalidierung erklären. |
| 30 | [webui_v2/AGENTS.md](../../crates/brassclaw_webui_v2/AGENTS.md) | **A; S-Erhalt nach §10:** Review-/Deploydarstellung auf einzelne Revision und Nachfolgerkompatibilität routen; keine erneute Freigabe unveränderter Verbraucher verlangen. |
| 31 | [webui_v2/CLAUDE.md](../../crates/brassclaw_webui_v2/CLAUDE.md) | **A; S-Erhalt nach §10:** Bestehende exakte Association-Q2-Route als Iststand kennzeichnen; U9 als UI/API-Nachfolger nennen. Laufzeit-/Slot-/Subjectrechte nicht aus Modellflags ableiten. |
| 32 | [development-policy.md](../development-policy.md) | **A; S-Erhalt nach §10:** Q1-/Evidenztabelle lokal auf A-Funktionsprüfung, einmalige gemeinsame Skill-/Codechecks und Wiederverwendung gültiger Abhängigkeiten optimieren. S-Integritäts-/Behavior-/Kombinationsnachweise ausdrücklich beim bestehenden Bootstrapowner lassen. |
| 33 | [implementation-status.md](../implementation-status.md) | **A; S-Erhalt nach §10:** Sourcegestützten Q1-Iststand und die A/S-Zieländerung separat verlinken; echte Lücken der Klassenrouten und Seed-Voraussetzungsbelege offen benennen, keine neue Runtimeabnahme behaupten. |
| 34 | [Recipe_Reviewed_v3_plan.md](Recipe_Reviewed_v3_plan.md) | **A; S-Erhalt nach §10:** Nur offenen Nachfolger zur Weitergeltung unveränderter Recipes bei kompatiblen Componentupdates eintragen. Alte Finalisierungs-/Composerhistorie erhalten; Workflowprüfungsauftrag ist ausgelagert. |
| 35 | [monty.composition.implementation.md](monty.composition.implementation.md) | **A; S-Erhalt nach §10:** Nur A-Schema-/Closureapproval-Vererbung auf eigenen Funktions-/Kompatibilitätsvertrag umstellen. S-Exaktbindung bleibt unverändert; gemeinsame Skill-/Codeprüfregeln nicht doppelt implementieren. |
| 36 | [monty.composition.md](monty.composition.md) | **A; S-Erhalt nach §10:** U4/U5 als neuen offenen Nachfolger mit Ausgangskante zu offenen Familien-/Deploymentgates aufnehmen. Individuelle Nachfolgerprüfung statt pauschaler Neuqualifikation jedes geänderten Graphen; bestehende Migrationsergebnisse nicht öffnen. |
| 37 | [monty.composition.recipe-implementation.md](monty.composition.recipe-implementation.md) | **A; S-Erhalt nach §10:** Im offenen RCP-Nachfolger automatische kompatible Abhängigkeitserneuerung ohne neue Freigabe unveränderter Recipes erklären. Bestehende Beispiele/abgeschlossene RCPblöcke erhalten; Workflowprüfungsumfang wird im neuen Plan bearbeitet. |
| 38 | [monty.composition.recipes.md](monty.composition.recipes.md) | **A; S-Erhalt nach §10:** Offenen Nachfolger zur technischen UUID-Zuordnung und Weitergeltung eigener Freigaben ergänzen; keine neue Genehmigung jeder Versionskombination. Keine neue Recipe-Prüfanweisung in diesem Plan. |
| 39 | [monty.composition.skill-recipes.md](monty.composition.skill-recipes.md) | **A; S-Erhalt nach §10:** Im offenen SKR-/MCP-Nachfolger Weiterverwendung vorhandener Freigaben bei kompatibler Aktualisierung erklären; kein zweites Kombinations-Q2. Öffentlichen Workflowprüfungsumfang ausschließlich im neuen Plan bearbeiten. |
| 40 | [prefix_v3_upgrade.md](prefix_v3_upgrade.md) | **A; S-Erhalt nach §10:** U7 in neuen offenen Nachfolger der tatsächlichen Restfolge aufnehmen; komplette Source-/Teachingclosure erfassen. Sourcehash-/Integritätsprüfung nicht als neue Freigabe unveränderter Components missverstehen. |
| 41 | [sempai_v3.5.md](sempai_v3.5.md) | **A; S-Erhalt nach §10:** Im offenen Sempainachfolger A-Vorschlagsprüfung von Step-10-S-Bibliotheksqualifikation unterscheiden. Skill-/Codefunktion einmal prüfen; gültige Abhängigkeiten nur referenzieren, keine A-Kombinationenqueue. |
| 42 | [simplified-v3-implementation.md](simplified-v3-implementation.md) | **A; S-Erhalt nach §10:** Nur neuen offenen Implementierungsauftrag U3/U6 mit Planreferenz anhängen; ursprüngliche exakte Kombinationsreader/-tests als historische tatsächlich implementierte Ergebnisse erhalten. Kein alter Journaleintrag wird zur neuen Architektur umgeschrieben. |
| 43 | [simplify_connections_v3.5.md](simplify_connections_v3.5.md) | **A; S-Erhalt nach §10:** Connections Phase F muss A-Funktions-/S-Seedbelege herkunftsrichtig transportieren. Keine eigene fachliche Prüfengine oder Zusammenfassung unterschiedlicher Trustbelege. |
| 44 | [zencoder-extension-plan.md](zencoder-extension-plan.md) | **A; S-Erhalt nach §10:** Offenen Nachfolger für eigene Komponentenfreigaben und Kompatibilität eintragen. Vorhandene Handler-/Registrierungslücken sichtbar lassen; Recipeprüfungsumfang nicht hier erweitern. |
| 45 | [component-deployment.md](../reborn/component-deployment.md) | **A; S-Erhalt nach §10:** Heutiges vollständige-Recipe-Subject-Deployment als Istgrenze erhalten; neuen Zielabschnitt zu U3/U4/U9 ergänzen: individuelle Nachfolger ohne erneute Einreichung aller abhängigen Recipes. Alte APIbeispiele nicht als neue Formate ausgeben. |
| 46 | [retained-component-inputs.md](../reborn/contracts/retained-component-inputs.md) | **A; S-Erhalt nach §10:** Alte /1-Record-/S-Scopebedeutung und exakte S-Closureprüfung erhalten. A bekommt strikt geplante gemeinsame Skill-/Codesubjects und eigene Funktionsbelege; technische Association bleibt, kein zusätzlicher A-Kombinationsapproval. |
| 47 | [retained-first-party-capability.md](../reborn/contracts/retained-first-party-capability.md) | **A; S-Erhalt nach §10:** Native Identität/Integrität und aktuelle Policy von individueller Freigabe trennen; keine neue Aufgabe „combination approval“ für native Loader formulieren. U6-Nachfolger referenzieren. |
| 48 | [raw-file-snapshot.md](../reborn/raw-file-snapshot.md) | **A; S-Erhalt nach §10:** Letzten Abschnitt auf eigene Freigabe der Read-Usage/Purekomponente und Gültigkeit unveränderter Consumer präzisieren. Vollständige Rohbytes/Custody/Originalhashes erhalten; Recipeprüfung gehört in den exportierten Plan. |
| 49 | [source-inventory-inspection.md](../reborn/source-inventory-inspection.md) | **A; S-Erhalt nach §10:** Eigene Source-Inspection-/Purefreigaben und Nachfolgerkompatibilität abgrenzen; Parserreport bleibt kein Freigabenachweis. Mehrprofil-/ToolSkill-Lehrbeispiel und Workflowprüfung sind ausgelagert. |
| 50 | [recipe-first-architecture-audit.md](../recipe-first-architecture-audit.md) | **A; S-Erhalt nach §10:** Aktuelle Empfehlung zur Freigabe auf §2 ergänzen; alte Beobachtungen nicht nachträglich als neue Ursache/Abnahme umschreiben. Neue Ownerlücken U1/U4 nennen. |
| 51 | [path_to_v3.5.md](../../path_to_v3.5.md) | **A; S-Erhalt nach §10:** Bestehende offene V35-03/04/06/07/16/17–20/27/30 nach A-Funktionen und S-Qualifikation abgleichen. Skills/Code gemeinsam prüfen, Abhängigkeiten nicht neu genehmigen; alte Spezifikation107 unverändert. |
| 52 | [prefix_plan2.md](../../prefix_plan2.md) | **A; S-Erhalt nach §10:** Historischen Plan erhalten; lokalen Verweis auf aktuellen U7-/Validierungsnachfolger ergänzen, damit alter Bindingkopf keine zusätzliche Kombinationsfreigabe lehrt. Keine Implementierung zurück in diesen Altplan verschieben. |
| 53 | [recipe.md](../../recipe.md) | **A; S-Erhalt nach §10:** Ownership-/Approval-/Versionsregeln und Checklisten auf Weitergeltung unveränderter Recipefreigaben und automatisch neueste aktive UUIDauflösung bei neuen Tasks umstellen. Keine Versionsnummern ins Recipe. Prüfungsumfang neuer/geänderter Workflows ausschließlich im neuen Plan. |
| 54 | [sempai-authoring-reference.md](../../scripts/prefix/sempai-authoring-reference.md) | **A; S-Erhalt nach §10:** Authoringreferenz mit zwei realen Wegen: A-Tool/ToolSkill zuvor freigegeben, danach Skill/Code als eigene Funktion; S zusätzlich genauer Nutzungsbeleg nach vorangegangener Qualifikation. Keine pauschale Anweisung alle Kombinationen neu zu prüfen. |
| 55 | [sempai-monty-compatibility.md](../../scripts/prefix/sempai-monty-compatibility.md) | **A; S-Erhalt nach §10:** Preload-/Invocation-/Typkompatibilität vom fachlichen Nachfolgernachweis trennen; feste Taskpins erhalten. Gemeinsame ToolSkillnutzung ist ausgelagert; Interpreter-/ABIreferenz nicht umschreiben. |
| 56 | [sempai-worked-examples.md](../../scripts/prefix/sempai-worked-examples.md) | **A; S-Erhalt nach §10:** Authored Beispiel für gemeinsamen Skill-/Codefunktionsreview ohne Dependencyreapproval und getrenntes S-Beispiel mit Tool→ToolSkill→Skill/Code und exaktem Nutzungsbeleg. Historische Beispiele/Belege nicht umetikettieren. |
| 57 | [simplified_v3.md](../../simplified_v3.md) | **A; S-Erhalt nach §10:** U3/U6/U9/U10 als offenen Nachfolger ab K4 nach aktuellem Paket eintragen; neue Regel löst für künftige Arbeit die betroffenen alten Phase-0a/3a-/§8.2-Zielpassagen ab. K0–K3 und laufende Admission-/Providerarbeit bleiben unangetastet. |
| 58 | [skills.md](../../skills.md) | **A; S-Erhalt nach §10:** Skilldefinition: Implementierung gehört fachlich dazu; getrennte IDs bleiben. A-Prüfung einmal mit zugehörigem Code, keine extra Associationapprovalrunde. S-Format/Qualifikation bleibt gültig, genehmigt aber keine fremden Componentrevisionen. |
| 59 | [tools.md](../../tools.md) | **A; S-Erhalt nach §10:** Individuelle Toolfreigabe und Nachfolgerkompatibilität verankern; zusätzliche Versionskombinationsapprovalpflicht entfernen. Native Artefakt-/ABI-/Livepolicy erhalten; Operations-/ToolSkillgranularität nicht hier ändern. |
| 60 | [toolskills.md](../../toolskills.md) | **A; S-Erhalt nach §10:** Eigene ToolSkillprüfung gegen bereits freigegebenes Tool beschreiben. Diese Freigabe besteht vor A-Funktionstest und vor S-Skillnutzungsqualifikation; sie wird durch späteren Skillbeleg nicht erneut erteilt. |
| 61 | [validator_v3.md](../../validator_v3.md) | **A; S-Erhalt nach §10:** Offene Steps1–3/5–10/11–12 nach §2.9 optimieren: A-Funktionsscope, gemeinsames Skill-/Code-Subject/RuleSet, keine doppelte class22-Route oder extra Kombinationenengine; S-Step11 mit eigener Reihenfolge/Scope erhalten. Recipepfade im zweiten Exactplan. |

## 6. Zusätzliche relevante Dateien aus dem aktuellen Abgleich

Diese Einträge erweitern die 61-Dateien-Basis. Auch hier handelt es sich um
einzelne Dokumentarbeiten, nicht um das Implementieren ihrer referenzierten APIs.

| Nr. | Datei | Konkreter Dokumenteingriff / Abnahme der Zeile |
| --- | --- | --- |
| 62 | [authored-catalogue-activation.md](../authored-catalogue-activation.md) | **A; S-Erhalt nach §10:** Katalogmechanik liest A-Funktionsfreigaben oder S-exakte Skillnutzungsbelege plus vorhandene Voraussetzungsfreigaben herkunftsrichtig. Neue A-Funktions-/Codesubjects nicht durch gleiche historische S-Closure ersetzen. |
| 63 | [pure-component-review.md](../reborn/pure-component-review.md) | **A; S-Erhalt nach §10:** Purecode erhält eigene A-Funktionsvalidierung; genehmigte Purehelpers werden nicht für neue Consumers komplett revalidiert. Skillimplementierung ist gemeinsamer Kandidat seiner Skillfunktion. S-Paket-/Nutzungsqualifikation separat erhalten. |
| 64 | [component-review-submissions.md](../reborn/contracts/component-review-submissions.md) | **A; S-Erhalt nach §10:** Single-root-Istschema ehrlich behalten; koordinierten Skill-/Codeprüfgegenstand im vorhandenen strikten Submissionowner planen, ohne zirkuläre Vorabfreigabe der beiden Kandidaten. Genehmigte Dependencies bleiben Voraussetzungen, keine neuen Reviewroots. |
| 65 | [workflow-review-evidence.md](../reborn/contracts/workflow-review-evidence.md) | **A; S-Erhalt nach §10:** Exakte damalige Recipe-/Variantenbeobachtungen unverändert erhalten; Weitergeltung eigener Freigaben durch nachgewiesene Nachfolgerkompatibilität erklären. Kein neuer Workflowprüfungsumfang und keine Änderung von V106-Belegbytes. |
| 66 | [validator-coverage.md](../reborn/contracts/validator-coverage.md) | **A; S-Erhalt nach §10:** Benötigte A-Funktionsregeln für Skill/Code einmal vollständig sammeln, Klassen-22-Teilchecks wiederverwenden; keine Pflichtregel „generische Kombinationenfreigabe“. S-Observer/Bootstraprecords getrennt belassen. |
| 67 | [structural-presence-check.md](../reborn/contracts/structural-presence-check.md) | **A; S-Erhalt nach §10:** Helperfreigabe und successor-owned compatibility präzisieren; presence check beweist weder Verhalten noch Kompatibilität. Umfang der Recipeprüfung ist exportiert. |
| 68 | [recursive-contract-check.md](../reborn/recursive-contract-check.md) | **A; S-Erhalt nach §10:** Grammatikcheck von gerichteter Kompatibilität alter Inputs/Resultgarantien trennen; zusätzliche Prüfung an U1 übergeben. Missing/null, Extras und Defaults als konkrete Kompatibilitätsfallen erläutern. |
| 69 | [source-inspection.md](../reborn/source-inspection.md) | **A; S-Erhalt nach §10:** Eigene gültige Source-Inspection-Revision und ihre Nachfolgerkompatibilität von bloßem Quellparsererfolg unterscheiden. Gemeinsame Bindingnutzung und Recipeprüfungsbeispiel ausschließlich im neuen Plan ändern; UUIDnavigation bleibt technisch. |
| 70 | [global-monty-task-factory.md](../reborn/contracts/global-monty-task-factory.md) | **A; S-Erhalt nach §10:** Factory konsumiert A-Funktionsfreigaben oder S-Seedbelege nach tatsächlicher Auswahl; eigenes genehmigtes Validatorprogramm ist nicht die Kombinationenprüfung seines authored Kandidaten. |
| 71 | [completed-turn-sempai-review.md](../reborn/contracts/completed-turn-sempai-review.md) | **A; S-Erhalt nach §10:** Reviewvorschlag von eigenständiger Komponentenvalidierung unterscheiden; neue Helper-/Skillrevision erzeugt keine automatischen Reviews unveränderter Consumer. U8, echte No-Match-Eignung und Originalcustody erhalten. |
| 72 | [model-token-limits.md](../reborn/model-token-limits.md) | **A; S-Erhalt nach §10:** Aktuellen nativen Outputwert/Fallback als konkreten kompatiblen Vertrag erklären; Pair-/Prefix-/Guardprüfungen sind technische Anforderungen, keine erneute Skill-/Recipefreigabe. Zwei-Werte-/budgets-off-Fälle zu bestehendem Exactowner routen. |
| 73 | [mcp-recipe-discovery.md](../mcp-recipe-discovery.md) | **A; S-Erhalt nach §10:** Exakte historische Qualification und /1-Approvalrecords als Iststand erhalten; künftige kompatible Nachfolger verwenden bestehende Recipegültigkeit. Refresh/Commandvertragsprüfung versus menschliche Revalidierung trennen; Chatbridge/Requestfenster bleiben. |
| 74 | [monty_host/README.md](../../crates/brassclaw_monty_host/README.md) | **A; S-Erhalt nach §10:** Inspektionsfakten liefern keinen vollständigen Funktions-/Seednutzungsbeleg. A/S unterschieden formulieren, ohne Worker zur eigenständigen Kombinationsprüfengine auszubauen. |
| 75 | [brassclaw-architecture.md](../brassclaw-architecture.md) | **A; S-Erhalt nach §10:** System-/Skills-/Compositionabschnitte und umfassenden Autoritätsanspruch auf die vier gültigen Guides ausrichten; §2-Rollen, eigenständige Gültigkeit und Updateverhalten erklären. Historische Kernel-/Loopbeschreibungen nicht als aktuellen Freigabeowner verwenden. |
| 76 | [agents-v3/README.md](../agents-v3/README.md) | **A; S-Erhalt nach §10:** Gültigen Kopf und Kapitelrouting auf eigene Freigaben, Updatekompatibilität und feste Taskauswahl angleichen. Kardinalitäten und Workflowprüfungsumfang ausschließlich im exportierten Plan lehren. |
| 77 | [01-architecture-overview.md](../agents-v3/01-architecture-overview.md) | **A; S-Erhalt nach §10:** Eigene Komponentenfreigaben, Nachfolgerkompatibilität und Taskauswahl ohne neue zentrale Kombinationsapprovalschicht erklären. Mehrfachnutzung eines Bindings nicht in diesem Plan ändern. |
| 78 | [02-intent-system.md](../agents-v3/02-intent-system.md) | **A; S-Erhalt nach §10:** Genehmigter kohärenter Katalog und neue Taskauswahl gelten nach Komponentenupdate weiter; kein No-Match wegen fehlendem neuem Kombinationsapproval. Echte technische Fehler bleiben Fehler. |
| 79 | [03-recipe-system.md](../agents-v3/03-recipe-system.md) | **A; S-Erhalt nach §10:** Weitergeltung unveränderter Recipes bei kompatiblen Nachfolgern und stabile UUID-Auswahl erklären; keine neuen Speicherfelder erfinden. Workflowprüfungsumfang ist exportiert. |
| 80 | [04-ibs.md](../agents-v3/04-ibs.md) | **A; S-Erhalt nach §10:** IBS prüft technisch individuelle Gültigkeit/Kompatibilität und pinnt newest active im gemeinsamen Snapshot; es führt weder semantischen Review noch neues Kombinations-Q2 aus. |
| 81 | [05-skills-system.md](../agents-v3/05-skills-system.md) | **A; S-Erhalt nach §10:** Skill und Implementierung gemeinsam als eine Funktion validieren, keine unabhängige doppelte Vollprüfung beider IDs. A ohne zusätzliche Kombinationsprüfung, S-Exaktnutzungsbeleg mit bereits qualifizierten Voraussetzungen. |
| 82 | [06-tools-system.md](../agents-v3/06-tools-system.md) | **A; S-Erhalt nach §10:** Eigene Toolfreigabe und Erhalt benötigter Primitive-/Adapterverträge erklären. Tooloperationen und gemeinsame ToolSkillbereiche gehören in den neuen Plan. |
| 83 | [07-pythoncode-system.md](../agents-v3/07-pythoncode-system.md) | **A; S-Erhalt nach §10:** Eigene PythonCodefreigabe und Weitergeltung bei kompatiblen Dependencyupdates darstellen; Preloadisolierung und Source-/Datengrenze erhalten. Exportaufruf-/Mehrfachnutzungsbeispiele sind ausgelagert. |
| 84 | [08-actions-system.md](../agents-v3/08-actions-system.md) | **A; S-Erhalt nach §10:** Gültigen Kopf auf neue Komponentenfreigabe abstimmen; historische Actionschemata nicht zu einer zusätzlichen Combinationapprovalebene aufwerten. Aktuelle Recipes bleiben Workflowowner. |
| 85 | [09-sempai-kohai.md](../agents-v3/09-sempai-kohai.md) | **A; S-Erhalt nach §10:** Sempai schlägt eigene neue/geänderte Components vor; Q1/Q2 verbleibt im Validatorpfad und wird nicht transitiv auf alle Consumer angewandt. Promptkorrektur und Komponentenreview getrennt lassen. |
| 86 | [10-prefix-base-prompt.md](../agents-v3/10-prefix-base-prompt.md) | **A; S-Erhalt nach §10:** Prefix-/Catalogueaktualisierung nach Komponentenaktivierung ist abgeleitete technische Arbeit; keine neue Komponentengenehmigung aller enthaltenen Quellen. Alte Generations-/Taskpins bleiben. |
| 87 | [11-retrieval-system.md](../agents-v3/11-retrieval-system.md) | **A; S-Erhalt nach §10:** Abrufbare Gültigkeit einer eigenen Revision, aktive Auswahl und historische Testclosure trennen; Dependencywechsel invalidiert keine unveränderten Retrievalcomponents. Daten-/Token-/Scopefragen nicht nebenbei verändern. |
| 88 | [12-agent-loop.md](../agents-v3/12-agent-loop.md) | **A; S-Erhalt nach §10:** Laufende Auswahl bleibt fix, neue Tasks nutzen kompatible aktive Nachfolger; Loop erzeugt keine neuen Komponentenfreigaben. Rustloopdarstellungen bleiben historische Umsetzung. |
| 89 | [13-orchestrator-default-py.md](../agents-v3/13-orchestrator-default-py.md) | **A; S-Erhalt nach §10:** Aktuelle Orchestratorregel: Auswahl/Invocation, keine erneute Validierung vor jedem Ausführen. Historisches `host.validate_component` nicht als aktuelle Dispatchpflicht lehren. |
| 90 | [14-validation-queue.md](../agents-v3/14-validation-queue.md) | **A; S-Erhalt nach §10:** Queue nur eigene authored Funktionskandidaten einschließlich koordinierter Skill/Codeänderung; keine abhängigen Reviewaufträge oder S-Seedkombinationen als normale Q1-Jobs. Klassenrouting/Originalidentität behalten. |
| 91 | [15-component-catalog.md](../agents-v3/15-component-catalog.md) | **A; S-Erhalt nach §10:** Individuelle gültige Versionen im Klassensystem erklären; Labels/Pointer bleiben kein Nachweis und es entstehen keine neuen Kompatibilitätsklassen. Kardinalitätspräzisierung ist exportiert. |
| 92 | [16-kernel-composition.md](../agents-v3/16-kernel-composition.md) | **A; S-Erhalt nach §10:** Individuelle Validierung liegt vor Aktivierung; Composition löst technisch auf, Kernel prüft live Toolpolicy. Exakte Artefaktauswahl nicht mit zusätzlicher Kombinationsfreigabe vermischen. |
| 93 | [17-webui-prefix-tab.md](../agents-v3/17-webui-prefix-tab.md) | **A; S-Erhalt nach §10:** UI unterscheidet Komponentenfreigabe, Prefixstatus und aktive Generation; Dependencyupdate fordert keine neue Freigabe alter Recipes/Skills. Bestehende UI-/Generationbeispiele lokal als Ist/Historie markieren. |
| 94 | [DOC_CONVERSION_MECHANISM_DESIGN.md](../agents-v3/DOC_CONVERSION_MECHANISM_DESIGN.md) | **A; S-Erhalt nach §10:** Historischen Mechanismus nicht wieder implementieren; gültigen Architekturkopf/Forwardverweis präzisieren. Falls offene Folgearbeit besteht, U1/U7 konsumieren statt alte Q1/Q2- und Builtinregeln als neuen Owner auszugeben. |
| 95 | [architecture_prefix.md](../../crates/brassclaw_engine/prompts/architecture_prefix.md) | **A; S-Erhalt nach §10:** Aktive Lehrabschnitte zu Klassen, Skill/ToolSkill und Validierung auf §2 prüfen und lokal korrigieren. Tatsächliche Einbettung/Seednutzung vor Änderung erfassen; notwendige neue Source-/Seedbytes nur über U6/U7 planen. Historische SQLkommentare erhalten. |
| 96 | [architecture/AGENTS.md](../../crates/brassclaw_architecture/AGENTS.md) | **A; S-Erhalt nach §10:** Dokumentrouting zu den neuen Komponentenverträgen ergänzen, falls aktuelle Guardhinweise zusätzliche Approvalpflicht suggerieren; ansonsten ausdrücklich ohne Delta dispositionieren. Architekturtests liefern keine menschliche Komponentenfreigabe. |
| 97 | [architecture/CLAUDE.md](../../crates/brassclaw_architecture/CLAUDE.md) | **A; S-Erhalt nach §10:** Kurze Crategrenze gegen 96 prüfen; kein neuer Validator-/Approvalowner in Architekturtests. Nur konkreten Widerspruch lokal ändern, kein unnötiger Vollvertragsduplikat. |
| 98 | [current-architecture-map.md](../reborn/2026-04-25-current-architecture-map.md) | **A; S-Erhalt nach §10:** Datierte Istkarte erhalten; im aktuellen Routinghinweis Komponentenreview von alten Invocationleases und §2 unterscheiden. Keine Rückdatierung des neuen Vertrags in Aprilbeobachtungen. |
| 99 | [product-manager-architecture-guide.md](../reborn/2026-04-27-product-manager-architecture-guide.md) | **A; S-Erhalt nach §10:** Aktuelle Komponenten-/Freigabeorientierung ergänzen: eigene Revision, kompatible Updates, Recipegültigkeit. Historische Risiko-/Invocationapprovalbeispiele nicht als neues Components-Q2 ausgeben. |
| 100 | [Exact_V3_definition_update_at_K3.md](Exact_V3_definition_update_at_K3.md) | **A; S-Erhalt nach §10:** Nur offenen neuen A/S-Folgeauftrag referenzieren; alte Lieferbelege und abgeschlossene Planabschnitte bleiben unverändert. Historische pauschale Combinationtexte werden nicht rückdatiert. |
| 101 | [Exact_V3_definition_update_at_K3-2.md](Exact_V3_definition_update_at_K3-2.md) | **A; S-Erhalt nach §10:** Nach aktuell laufendem I-Paket neuen offenen Anschluss für §2/U1/U3/U6 ergänzen. Alte B1/B2-Bytes, Outputproducer-/ResultOnlybelege und Admissiongates schützen; I1 braucht reale Eingabe, keine pauschale Free-/Bootstrapaktivierung. |
| 102 | [runtime-limits-audit.md](../runtime-limits-audit.md) | **A; S-Erhalt nach §10:** Historische Laufzeit-/Kombinationsbefunde erhalten; neue aktuelle Einordnung nennt A-Funktionsscope und weiter gültige S-Exaktnutzungsbelege getrennt. |
| 103 | [contracts/AGENTS.md](../reborn/contracts/AGENTS.md) | **A; S-Erhalt nach §10:** Autorität von Vertragsdokumentation versus Istcode und dem vier-Guide-Sollvertrag präzisieren. Dokument-only Planänderung nicht durch die allgemeine „update Rust in same branch“-Anweisung in unerlaubte Codearbeit verwandeln; U2/U6 als spätere Owner nennen. |
| 104 | [migration-compatibility.md](../reborn/contracts/migration-compatibility.md) | **A; S-Erhalt nach §10:** Im aktuellen Komponentenbezug hinzufügen: historische Review-/Revisionbytes erhalten, neue Freigabesemantik ausdrücklich migrieren, individuelle Nachfolger ohne Consumerreapproval. Alte Speicher-/Scope-Migrationshistorie bleibt außerhalb dieses Deltas. |
| 105 | [_contract-freeze-index.md](../reborn/contracts/_contract-freeze-index.md) | **A; S-Erhalt nach §10:** Aktuelle Verweiskante zu den vier Guides und diesem Präzisierungsauftrag ergänzen; alter Freeze ist kein veto gegen autorisierte Vertragsänderung. Keine historische V1-Invocationlease als Componentsfreigabe lehren. |
| 106 | [skills-extension.md](../reborn/contracts/skills-extension.md) | **A; S-Erhalt nach §10:** V3-Supersession auf eigene gültige Revision, Nachfolgerkompatibilität und feste Taskauswahl beziehen. Gemeinsames ToolSkillbinding im neuen Plan; historische SKILL.md-Bundlearchitektur nicht wiederbeleben. |
| 107 | [v3.5 Technical specification](../../.zenflow/tasks/strictly-follow-the-following-wo-6326/spec.md) | Bewusst kein Edit: datierte abgeschlossene Planungsspezifikation einschließlich ihrer damaligen Kombinationsregeln unverändert erhalten. Den noch bestehenden Verweis ausschließlich im aktuellen konsumierenden Roadmap-Nachfolger 51 einordnen. Anforderungen/Report ebenfalls nicht umschreiben und keine neue Implementierung in diesen alten Auftrag verlagern. |

### 6.1 Geprüfte historische/andersartige Treffer ohne Umsetzungsauftrag

Folgende Gruppen werden bewusst nicht umgeschrieben; sie sind keine weiteren
aktuellen Vertragsowner:

- `docs/validation/`, unveränderliche JSONreceipts, Hashmanifeste und gespeicherte
  Referenz-/Binaryarchive: historische exakte Belege erhalten. Neue Freigabesemantik
  ist kein Anlass, ihre damaligen „combination approval“-Aussagen umzubenennen.
- `scripts/prefix/sempai-accuracy-*` und `sempai-optimization-*`, einschließlich
  `compiled-*` und `reference-v*`: eingefrorene Experimente; nur neue aktuelle
  Nachfolger aus den kanonischen Lehrquellen über den Prefixowner erzeugen lassen.
- `scripts/prefix/SEMPAI-REFERENCE-AUDIT.md`, `CHANGELOG.md` und
  `docs/development-handoff.md`: alte Audit-/Änderungs-/Handoffeinträge erhalten.
  Ein späterer neuer Eintrag darf nur tatsächlich gelieferte Änderungen melden,
  nicht die Umsetzung dieses Plans vorwegnehmen. Aktuelle Vertragsautorität bleibt
  Statusindex/Guides, nicht ein nachträglich bearbeiteter historischer Eintrag.
- `docs/archive/`, datierte ältere Engine-/Mission-/Retrievalpläne und historische
  `docs/agents-v3/subplan_*`/`mindmap_phase_h.md`: historische Quellen unter den
  aktuellen Guidevorrangblöcken. Keine erneute Aktivierung ihrer alten Steps; neue
  nötige Komponentenarbeit bleibt bei U1–U11. Ein bei D1 entdeckter tatsächlich
  noch aktiver Consumer ist dagegen einzeln in §6 aufzunehmen.
- Andere abgeschlossene `.zenflow/tasks/`-Spezifikationen, Anforderungen,
  Sourceinventare, Lieferreports und `.zenflow-attachments/`: ursprüngliche
  Aufträge und datierte Planungsbelege erhalten. Der weiterhin von der v3.5-Roadmap
  referenzierte Spezifikationspfad ist als konkreter Sonderfall in 107 erfasst.
- Auth-/Outbound-/Replytarget-Revalidierung, CSRF, Netzwerk-/Secret-/Ressourcenpolicy,
  Provider-/Pair-/Prefixdesignprüfung und Invocationfencing: andere Prüfgegenstände.
  Der Entfall der Komponenten-Reapprovalkaskade hebt diese technischen Prüfungen
  nicht auf. Generische `revalidate`-Suchtreffer werden nicht pauschal ersetzt.

### 6.2 Beobachtete Sourceconsumer außerhalb des Dokumentedits

Der Abgleich hat die vorhandenen Implementierungsowner als Übergangspunkte
identifiziert: Skills `association_contract`/`association_review_store`/Revisionen,
Engine `retained_tools`/`retained_reviews`/Preload/Taskselection, Composition
`authored_monty_catalogue`/`component_deployment`/PG-Review-/Bootstrap-/MCPowner.
Das vorhandene `prefix_release_documents.rs` bettet viele der Zielfiles über
`include_str!` ein und liest `scripts/prefix/release-reference-inventory.json`.
Quell- und Inventaränderungen daraus gehören in die jeweils vorhandenen Owner
aus U2–U7; Scaffoldzuordnung/-abbau wird ausschließlich über U11/W-R integriert.

Die sechs früheren Rust-Inventareinträge (`completed_turn_review_components.rs`,
`global_bootstrap_components.rs`, `model_completion_components.rs`,
`prefix_generation_components.rs`, `prefix_source_components.rs`,
`validator_components.rs`) sind keine Editziele dieses Plans. Ihre betroffenen
eigenen Folgearbeitspakete müssen künftige Kompatibilität und Paketidentität
abdecken. Auch dieser Plan selbst wird nicht vorsorglich in Runtime-/Release-
Inventare aufgenommen. Keine gemeinsamen Source-/Cargoänderungen ausführen.

## 7. Abschlusskriterien dieses Dokumentplans

- [ ] §§5–6 vollständig dispositioniert; alle 61 Basisdateien und zusätzliche
  tatsächlich aktuelle Vertrags-/Architekturstellen erfasst. Kein notwendiges
  Dokumentedit ist lediglich als „offen“ abgelegt; begründete No-Editfälle sind
  von tatsächlich gelieferten Änderungen getrennt.
- [ ] Die vier Guides erklären dieselben A-Funktionsfreigaben, Nachfolgerpflichten
  und Versionsauswahlregeln ohne zusätzliche A-Kombinationsapprovalebene sowie
  weiterhin genaue S-Skillnutzungsbelege mit separat freigegebenen Voraussetzungen. Die exportierten Themen
  sind ausschließlich im neuen Plan beauftragt.
- [ ] Ein kompatibles authored Update erzeugt keine neue Revision/Q1/Q2 eines
  unveränderten Consumers. Skill und implementierender Code wurden als eigene
  Funktion gemeinsam geprüft; keine doppelte volle class22-/Skillprüfung.
- [ ] Für S bleibt erforderliche neue exakte Nutzungsqualifikation erhalten;
  vorab gültige Tool-/ToolSkill-/Helferfreigaben werden dabei nicht neu erteilt.
- [ ] Alle benötigten Code-/DB-/UI-/Seed-/Migration-/Recoveryaufträge sind mit
  konkreter Abnahme in vorhandenen Ownerplänen eingetragen, hinter deren aktuell
  laufenden Paketen und vor den jeweils noch offenen konsumierenden Gates.
- [ ] Keine abgeschlossenen oder laufenden Planabschnitte, alten genehmigten
  Revisionen, Belegbytes oder angewandten Migrationen wurden umgeschrieben.
- [ ] Alte Tasks behalten exakte Auswahlen, neue Tasks erhalten die neuesten aktiven
  gültigen Nachfolger; Livepolicy, technische Guards und sichere Retention bleiben.
- [ ] Nachfolgerreview und Aktivierung berücksichtigen den geprüften Bedarf,
  gerichtete Interfacekompatibilität, parallele Änderungen und Lineage.
  Bestehende exakte Schema-/Approvalreader und alle notwendigen Gegenstellen
  sind ausdrücklich im verbundenen technischen Cutoverauftrag erfasst.
- [ ] Keine neue Consumerregistry, kein zweiter Activator/Evidencewriter oder
  pauschaler GC-Neubau und keine durch diesen Plan erfundenen technischen
  Kapazitäten. Nicht belegte GC-Unterstützung bleibt als offene Grenze sichtbar.
- [ ] Bootstrap bleibt nichtzirkulär ohne installiertes zusätzliches Q2;
  authored Änderungen behalten eigene Q1/Behavior/human Q2. I1/Free erhalten
  keine unverdiente Aktivierung durch diese Dokumentpräzisierung.
- [ ] Links, Spiegel, lokaler Diff und Whitespace geprüft. Dokumentlieferung und
  zukünftige Implementierung/Abnahme werden im Abschluss getrennt ausgewiesen.
- [ ] Keine aktive Scaffoldklasse, besondere Klasse-50-Freigabe oder neunte
  Validatorroute geplant. Vorhandene Daten/Taskpins werden über den einzigen
  bestehenden Migrationsowner berücksichtigt; Klasse 10 bleibt getrennt geschützt.

**Liefergrenze bei Erstellung am 30. Oktober 2026:** Nur dieses neue Plandokument
wurde angelegt. Die 107 Datei-Dispositionen sind geplante Arbeiten, keine Behauptung,
dass die Zieltexte, Freigabereader oder der Laufzeitkatalog bereits umgestellt sind.

## 8. Planaudit und Korrekturen — 30. Oktober 2026

Dieses Audit dokumentiert den früheren Zwischenstand. Seine allgemeinen
Kombinationsfreiheitsaussagen sind seit der A/S-Präzisierung ausschließlich auf
A anzuwenden; sie beauftragen keine Abschaffung des S-Verfahrens. Es ist weder
eine Komponentenvalidierung noch ein Runtime-/Migrationsnachweis.

| Befund | Korrektur im Plan |
| --- | --- |
| Heutige exakte Code-/Association-Schemavergleiche und Closurevererbung würden kompatible Updates trotz entfallenem Kombinations-Q2 weiter blockieren. | §2.6 und U4 erfassen den gerichteten Interfacevertrag und alle betroffenen Reader, einschließlich Altformatlesern. Kein pauschales Abschalten von Integritätsprüfungen. |
| Ein neuer Consumer zwischen Review und Aktivierung könnte eine nicht mehr erfüllte alte Schnittstelle benötigen. | §2.6/U1/U3 prüfen den tatsächlichen benötigten Vertrag im vorhandenen Publikations-/CASowner; gedeckte Änderungen brauchen keinen neuen Review, ungedeckte verhindern die betroffene Veröffentlichung. |
| Einzeln genehmigte Updates könnten gemeinsame deklarierte Workflowbedingungen verletzen. | Vollständigen geplanten Graphen einschließlich relationaler Verträge mechanisch prüfen; Semantik-/Behaviorpflicht verbleibt beim Kandidaten. Die Erweiterung der Recipeprüfung ist in den neuen Plan exportiert; hier verbleiben nur Aktivierungs-/Kompatibilitätsfälle. |
| Alle gespeicherten historischen Revisionen und beliebige Entwürfe könnten Kompatibilität endlos blockieren. | §2.3 unterscheidet tatsächlich weiterhin vorgesehene genehmigte Consumer, geschützte zugesagte Verträge, Entwürfe und ausschließlich alte Taskgraphen. |
| Ein „vorhandener sicherer GC“ war ohne allgemeinen Nachweis vorausgesetzt; außerdem könnten Belege/Revieworiginale zu früh gelöscht werden. | §2.4/U10 und §4.1 verlangen artefaktbezogene Referenzsicherheit, schützen Lineage/benötigte Originale und erlauben vorerst vollständige sichere Aufbewahrung. Kein neuer pauschaler GC-Service. |
| Formatmigration hätte vorhandene Kombinationsbelege entweder ungeprüft umetikettieren oder eine nachträgliche Consumer-Q2-Welle erzeugen können. | §2.5/2.7 trennen ausreichende historische Einzelabdeckung, technische Zuordnung und echte Evidenzlücken. Altprofile bleiben in ihrem gültigen Scope erhalten; neue Gates werden nicht durch Flags umgangen. |
| U3 und U4 konnten als zwei Aktivierungsimplementierungen gelesen werden; Validator/Katalog konnten gegenseitige neue Voraussetzungen bilden. | §4.2 legt genau einen Publikationsowner, einen Reviewowner und wiederverwendete Resolver-/Storebeiträge fest; vorhandener privater Prüfruntimepfad und nichtzirkuläres Seedverfahren vermeiden Selbstfreigabe. |
| Schutz eingebetteter Dokumente vor laufenden Builds stand erst bei D4, obwohl D2 vorher editiert. | D1 macht den Source-/Einbettungsabgleich zur Voraussetzung jedes tatsächlichen Edits. Keine Änderung der laufenden Implementierung. |
| Eintrag 107 verlangte zugleich eine historische Originalfassung zu erhalten und darin einen neuen Verweis einzutragen. | Eintrag 107 ist ausdrücklich No-Edit; Einordnung erfolgt nur im aktuellen offenen Roadmap-Nachfolger 51. |
| Ein Inventareintrag mit offenem Editziel hätte als abgeschlossener Dokumentabgleich zählen können. | D5/§7 unterscheiden tatsächliche Lieferung, begründeten No-Editfall und noch offene Dokumentarbeit. Technische Zukunftsarbeit muss im Ownerplan wirklich verankert sein. |
| Beispielzahlen konnten als neue Kapazitätsvorgabe verstanden werden. | §4.1 verwendet bestehende technische Limits und Callerbelege. Das gesamte ls-/Mehrfachbindingbeispiel einschließlich seiner Guards ist in den neuen Plan exportiert. |

**Auditprüfung:** sämtliche lokalen Dateilinks und eindeutigen Inventarzeilen,
Vollständigkeit der 61-Dateien-Basis, geänderte Textstellen und Whitespace prüfen.
Da die Datei noch untracked sein kann, zusätzlich den vollständigen Dateiinhalt
gegen `/dev/null` prüfen; ein gewöhnlicher pfadbegrenzter `git diff --check`
allein erfasst eine ungetrackte neue Datei nicht. Keine Cargo-/Runtimechecks
und keine Änderungen an Implementierungsdateien für dieses Planaudit.

## 9. Themenexport — 31. Oktober 2026

Die Aufträge aus dem früheren §2.1 zur gemeinsamen ToolSkillnutzung und aus
§2.2 Punkt 3 zur Recipeprüfung wurden vollständig in den
[neuen Plan](Exact_validation_definition_upgrade_at_K3-2.md) verschoben.
Die gemischten U1-/U4-/U5-Aufträge, Dateizeilen und Pflichtfälle wurden nach
Verantwortung zerlegt. Im neuen Plan enthält §8 die Herkunftszuordnung.

Die 107 Inventar-IDs bleiben als stabile Dateireferenzen erhalten; sie zählen
weder Änderungsfeatures noch doppelte Editaufträge. Eine gemeinsame Datei wird
jeweils nur für den dem ausführenden Plan gehörenden Abschnitt bearbeitet.
Keine der eigentlichen Dokumentänderungen oder späteren Implementierungen wird
durch diesen Export als durchgeführt ausgewiesen. Bei dieser Aufteilung wurden
ausschließlich die beiden Plandateien bearbeitet.

## 10. Verbindlicher A/S-Abgleich aller Dateien — Präzisierung vom 31. Oktober

Die frühere pauschale Entfernung jeder Kombinationsfreigabe wird hiermit
ausdrücklich auf A beschränkt. Die vorherigen Inventare und Export-IDs bleiben
erhalten; ihre aktuellen A/S-Scopehinweise sind verbindlich. Für **jede tatsächlich
betroffene aktuelle Regel, Plananweisung, Beispiel- oder Checklistenzeile** gilt:

- A: nur eigene Funktion prüfen; Skill und Implementierung zusammen, gültige
  Voraussetzungen verwenden; keine zusätzliche Versionskombinationsprüfung.
- S: bestehende kontrollierte genaue Skillnutzungsqualifikation weiterverwenden;
  der Beleg genehmigt nicht alle genannten Components. Vorherige Tool- und danach
  ToolSkillfreigabe unabhängig nachweisen; Skill/Code anschließend gemeinsam.
- Recipe: vollständiger Workflowprüfungsumfang bleibt beim zweiten Exactplan.
  Offered Kombinationen des Recipes sind sein eigener Prüfgegenstand, kein
  Argument für eine allgemeine A-Kombinationsengine.

Ein einzelner globaler A/S-Banner darf widersprechende aktive Detailanweisungen
nicht stehen lassen. In historischen unveränderten Belegen wird dagegen nichts
umbenannt. Folgende lokale S-/Q1-Ergänzungen sind zusätzlich zu den A-Dateideltas
aus §§5–6 konkret einzutragen; reine Weiterverweise in übrigen Inventardateien
müssen dieselbe Unterscheidung konsumieren.

| Betroffene Basis-ID / Datei | Konkrete lokale Änderung bei Durchführung |
| --- | --- |
| 04/05 Root AGENTS/CLAUDE und 32 [Entwicklungspolicy](../development-policy.md) | Aktuelle Authoring-/Checks-/Evidenzzeilen unmittelbar aufteilen. A-Funktions-Q1 mit gemeinsamer Skill/Codeprüfung; S-Installation einschließlich exakter Nutzung und vorab geprüfter Voraussetzungen erhalten. Kein authored Operator kann den Seedmodus wählen. |
| 53/58/59/60: vier Komponentenguides | Beim Skill ausdrücklich ein gemeinsamer funktionaler Prüfgegenstand trotz zweier UUIDs. Beim Tool dessen eigene qualifizierte Primitive vor abhängiger ToolSkill-/Skillprüfung. Beim ToolSkill eigene vorausgehende Bindingfreigabe. Association ist technische Zuordnung; genauer zusätzlicher Nutzungsapproval nur in S. Keine Sammelfreigabe der Dependencyclosure. |
| 61 [validator_v3.md](../../validator_v3.md) | Bestehende offene Steps1–3/5–10 gemäß §2.9 umstellen; normale A-Q1 braucht keine generische Kombinationsroute. Step7 Skill-/Klassen22-Regeln einmal zusammen ausführen, Pure standalone behalten. Step11 S-Scope/Tool→ToolSkill→Skill+Code-Reihenfolge ergänzen; Step12 UI/Abnahme getrennt. Laufende/geschlossene Teile unverändert, neuer gebundener Nachfolger nach D3. |
| 41 [Sempai v3.5](sempai_v3.5.md) | Offener Step10-Nachfolger bleibt S mit genauer Kombination für jeweiligen Skill, nicht Freigabe aller Dependencies. Offene Vorschlags-/Submission-/Step11/12-Verbraucher nutzen A-Q1 mit gemeinsamen Skill/Codefunktionen und ohne Reviewkaskade. Keine zweite Prüf-/Sempailoop. |
| 43 [Connections v3.5](simplify_connections_v3.5.md) | Offener PhaseF-/Abnahmenachfolger übernimmt beide Belegtypen mit korrektem Scope. Nur Transport-/Store-/Custodymechanik teilen; keine fachliche Kombinationenprüfung in Connections auslagern. |
| 51 [path_to_v3.5.md](../../path_to_v3.5.md) | V35-03/04/17–19 konsumieren A-Funktions-/Pair-Q1 und bestehende Pure-/Evidenceowner; V35-16/20 bewahren S-Bibliotheks-/Compilerqualifikation und ihren engen Scope. Neue offene Änderungen an vorhandene Gates binden, keine zusätzliche Workliste. |
| 23/24/35/36 Compositionguides und Plan | Eigene vorangehende Tool-/ToolSkillfreigaben von skillbezogenem S-Usageapproval trennen. A-Zuordnung braucht keine neue Associationapproval. Keine Verdoppelung des Resolvers oder Seedwriters. |
| 45/46/62 Deployment, retained Inputs, authored Activation | Modus-/Scopebewusste Reader im bisherigen Owner; A-Funktions-/koordinierte Skill-Codebelege und S-Exaktbelege getrennt prüfen, historisches /1 nicht umdeuten. Seedkomponentenliste ist keine selbständige Einzelgenehmigung ihrer Einträge. |
| 63/64/66 Pure Review, Submissions, Coverage | Vollständiges gemeinsames Skill-/Codesubject mit geänderten Roots im vorhandenen strikten Formatowner planen; doppelte RuleSets/Behaviorläufe vermeiden. Vorab gültige Purehelper übernehmen. Seed-Pure-/Compiler-Ausnahmen nicht verbreitern oder durch A-Lücken neu qualifizieren. |
| 30/31/65/90 UI-/Workflow-/Queueguides | Herkunft und Freigabescope anzeigen: A eigene Funktion, Skill+Code zusammen, Recipe eigene Pfadmenge; S exakter Nutzungsbeleg und getrennte vorangehende Voraussetzungen. Kein Q2-Button für S und keine eigenen Freigabeaktionen für bereits gültige Dependencies. |
| 40/54/55/56/95 Prefixplan und aktuelle Lehrquellen | Beide Wege in aktuellen Beispielen lehren; vorhandene Source-/Paketowner für neue Bytes nutzen. Keine pauschale Entfernung des Wortes Kombination, keine B1/B2-Änderung oder zusätzliche Bootstrapfreischaltung. |
| 33/42 Statusindex und Implementierungsjournal | Nur neue datierte Ziel-/Gapzuordnung anhängen: vorhandene strukturelle Q1-Prerequisites sind nicht fertige Funktions-Q1, einzelne Seedketten keine gesamte Bibliotheksreihenfolge. Keine historischen scoped Belege als neue Abnahme ausgeben. |

**Zusätzliche Abschlussbedingungen:** Kein gültiger S-Nutzungsbeleg wurde
abgeschafft oder zum Beleg für ungeprüfte Dependencies erklärt. Kein A-Review
startet eine generische Kombinationsvalidierung oder doppelte volle Skill-/Code-
Prüfung. Die konkrete Q1-Optimierung aus §2.9 ist in offenen bestehenden
Validator-/v3.5-Nachfolgern mit gebundener Abnahme eingetragen. Alle Source-/DB-/
Runtimearbeiten bleiben künftige Arbeit ihrer bisherigen Owner; bei dieser
Präzisierung wurden ausschließlich die zwei Exact-Validierungspläne geändert.

## 11. Scaffoldabbau in den bestehenden Dokument- und Planownern

Dies ist eine zusätzliche Dokument-/Planpräzisierung, keine durch diesen Plan
auszuführende Code-/DBmigration. Neue Aufträge nur hinter laufenden oder bereits
abgeschlossenen Paketen eintragen und vor dem nächsten betroffenen offenen
Aktivierungs-/Cutovergate verbindlich konsumieren lassen. U11 definiert nur
Freigabe-/Trustscope; W-R im zweiten Exactplan besitzt die tatsächliche Recipe-
Zuordnung und vorhandene Migrationsmechanik. Kein zusätzliches Klassensystem,
Scaffoldadapter, zweiter Dispatcher oder eigener Compatibility-Service.

| Basisdateien / vorhandener Owner | Konkreter lokaler Dokumenteingriff bei Durchführung |
| --- | --- |
| 04/05 AGENTS/CLAUDE, 07 crates/AGENTS, 53/58/59/60 vier Guides | „10/50 protected roots“ nicht als Ziel weiterführen. Nur Klasse 10 ist übergeordnetes Steuerungsprogramm; Scaffold ist kein zusätzlicher Skilltyp und kein geschützter Ablauftyp. Normale Ablaufwiederverwendung auf Klasse21/Recipe verweisen; technische Binding-/Kernelregeln erhalten. |
| 61 [validator_v3.md](../../validator_v3.md), offene Restliste/Steps1/7/8/9/11/12 | Neue Folgeanweisungen zu eigenem class50-review/Rootschutz entfernen beziehungsweise auf Klasse10 begrenzen. Keine Scaffoldklasse in Q1-Routing, benötigte Regeln oder Human-Q2 aufnehmen. Alte class50-Submissions nur historisch lesen, neue normale Abläufe über Klassen21-Route validieren. Bestehende acht Routen und geschützte Klasse10 bleiben. |
| 33 Statusindex, 51 [path_to_v3.5.md](../../path_to_v3.5.md), 41/43 v3.5-Pläne | Offene Work-/Restpflichten zum Scaffold-Sonderpfad streichen, soweit tatsächlich vorhanden; durch einmalige vorhandene Recipe-/Klassen-Migrationszuordnung ersetzen. Keine zusätzlichen v3.5-Freigabegates und keine nachträglichen Änderungen historischer Abnahmen. |
| 10/11/12 Engineguides, 23/24/35/36 Compositionguides/-pläne | Code-/Retrieval-/Katalog-/Settingstabellen als Iststand kenntlich halten, neue aktive Klasse50 ausschließen. Keine pauschale Änderung alter Revisions-/Manifestbytes oder Typprüfung durch „50 bedeutet jetzt 21“. W-R konsumieren, keine zweite Auflösungs-/Migrationsfunktion beauftragen. |
| 18/19/25/26/30/31 Produkt-/Ingress-/WebUIguides, 45/46/62/64 Deployment-/Subject-/Inputverträge | Kein eigener aktiver Scaffoldtab/-Editor/-Freigabeflow im Ziel. Echte vorhandene Navigation auf normale Recipeansicht/-verwaltung führen, ohne Klasse50-Dispatchalias. Historisches Readback darf class50 anzeigen; normale Auth-/Subject-/CAS-/Integritätskontrollen erhalten. |
| 75/76/77/79/89/90/91/92/95 Architektur und Klassendokumentation | Klasse50 nur als auslaufende historische Speicherung dokumentieren, keine künftige `reborn_scaffolds`-Tabelle oder separate Schutzklasse planen. Recipeaufruf mit typed Variablen als Wiederverwendung zeigen; globalen Orchestrator davon trennen. Eingebettete neue Guidebytes über vorhandene Owner nachführen. |
| 40/54/56 Prefix-/Teachingquellen und 57/104 Upgrade-/Migrationspläne | Nur neue reale Klassifikation/Recipeverweise in neue Pakete übernehmen. Früherer Scaffoldname begründet keine Toolberechtigung und keine automatische Seedfreigabe eines Ersatzrecipes. Historische Originale/Pins, Overrides und noch benötigte Artefakte bis zum referenzsicheren Ende behalten. |

Ist kein tatsächlicher Scaffoldconsumer vorhanden, genügt der begründete Wegfall
der aktiven Kategorie und ihrer Sondervalidierungspläne; es wird kein funktionsloser
Recipeersatz erfunden. Ist eine konkrete Semantik nicht durch ein vorhandenes
Recipe dargestellt, bleibt nur diese Zuordnung offen, bis sie über den normalen
Recipepfad vollständig spezifiziert und geprüft ist. Kein globales Öffnen oder
Schließen anderer gültiger Abläufe und kein Rückgriff auf einen Rustworkflow.

## 12. Source-/Umsetzungsabgleich und verbindliche Auditkorrekturen — 31. Oktober

Dieser Nachtrag präzisiert die obigen Arbeitsaufträge; er führt sie nicht aus.
Bei Durchführung aktuellen Source-/Queuezustand erneut lesen, keine eingefrorene
Statuskopie als aktuelle Freigabe ausgeben. Die datierten Beobachtungen in §8
bleiben historische Beobachtungen.

| Aktueller belegter Eingang | Einzutragende Konsequenz / Owner |
| --- | --- |
| [Single-root Submissionstore](../../crates/brassclaw_skills/src/review_submission_store.rs), [bisheriges Skill-Q2](../../crates/brassclaw_reborn_composition/src/pg_association_review.rs) | U2/Validator Steps2/3/9 folgen §2.2.1; vorhandene Transaktions-/ID-/Readbackmechanik erweitern, keine vermeintlich schon unterstützte Multi-root-Freigabe aufrufen. |
| [Aktueller retained Resolver](../../crates/brassclaw_engine/src/memory/retained_tools.rs) | U4 korrigiert gezielt A-Approval-/gerichtete Vertragsreader. Exakte Code-/Associationschemas und eindeutiges Code/ToolSkill/Tool/Callable-Tupel sind reale bestehende Gates. Keine pauschale Gleichheitsentfernung, kein zweiter Resolver. T-Arbeit bleibt im zweiten Plan. |
| [Revisionsdecoder](../../crates/brassclaw_skills/src/component_revision.rs), [alte Queue](../../crates/brassclaw_reborn_composition/src/validation_queue.rs) | U11/W-R müssen neue Schreib-/Freigabewege schließen, historische Klasse50-Integrität und zulässige alte Pins erhalten. Ein bloßer UI-Filter oder globaler Decoderban genügt nicht. |
| [Zentrale Queue](../development-build-queue.md): BUILD-004/007/008/009/011/012/013 Passed | Bestehende ResultOnly-/JSON-/gemeinsame Capture-/Kompatibilitätsparser-/native Custody-/Accountingmechaniken und ihre exakten Belege wiederverwenden. BUILD-008 ist neuer als der offene Capturehinweis des BUILD-007-Berichts. Keine Wiederimplementierung dieser bereits integrierten Teilfunktionen; kein Rückschluss auf fertige Q1/Free-/Seedfreigabe. |
| [Gemeinsames Capture](../validation/2026-10-31-shared-model-capture.md), [strikte JSON-Nutzung](../validation/2026-10-31-standalone-strict-json.md) | Den vorhandenen originalen Host/Claim, privaten Snapshotinspektor und gemeinsamen Capture-before-binding-Pfad nutzen; keinen öffentlichen Runcontext, zweite Lineageabfrage, vierten DTO oder parallele Live-VM ergänzen. Vorhandenes `builtin.json`/`parse_strict` wiederverwenden; die noch unapproved Drafts werden durch diese Belege nicht aktiviert. |
| BUILD-002 und BUILD-010 Proposed | Free-spezifische sealed Source-/Nativeedge-/Servingqualifikation bleibt beim bestehenden Prefixowner; Legacy-V2-Bridgeabbau beim bestehenden Bibliotheksowner. U1–U11 übernehmen keinen dieser Aufträge und schaffen keine Ersatzaliases/Parser/Proofstores. Migrationsnummern erst vom Integrationsowner zuteilen lassen. |

Zusätzliche Fälle in die bestehenden U-Abnahmen eintragen:

- Zwei neue Roots, nur neuer Skill, nur neuer Code und geteilter Code mit
  unterschiedlichen zugesagten Exporten: vollständige eigene Regeln, unveränderte
  Partnerfreigaben erhalten; kein doppelter Gesamtprüfungslauf.
- Konflikt an einer Base, vertauschter Partner, Teil-Q1, fremde Q2-Ansicht und
  ungewisser Commit: keine halbe Freigabe, keine neuen Retry-IDs, atomarer Readback.
- A/S-Mischgraph mit gültigen Voraussetzungen: keine Umetikettierung; fremder
  S-Nutzungsbeleg für geänderten authored Partner bleibt ungültig. Ein Tool-
  oder ToolSkillbeleg kann nicht durch einen Skill-Closurepass ersetzt werden.
- Wiederverwendete Fakten bei unverändertem Scope, aber geänderte Behaviorinputs,
  Nativeartefakte oder Subjects: nur genau passende Evidenz zählt; keine cachebasierte
  Q2 oder semantische Zustimmung.
- Neue Klasse50 über alten Import-/Queuepfad wird abgelehnt; historische Revision
  bleibt exakt lesbar, ausschließlich bereits gültiger alter Pin ausführbar.

Abschluss zusätzlich erst, wenn die genannten konkreten Aufträge und Fälle in
zukünftige offene Abschnitte ihrer bestehenden Owner übernommen und von deren
betroffenen offenen Cutovergates konsumiert werden. Dieser Plan selbst bleibt
reine Dokumentarbeit; keine Source-, Cargo-, DB- oder Aktivierungsänderung.

## 13. Ausführungskoordination — privates Textpaket 1, 31. Oktober

Root hat ausschließlich die private Vorbereitung begrenzter Textpatches und die
Pflege dieser beiden Pläne freigegeben. Kanonische Zieltexte integriert Root erst
nach seiner terminalen BUILD-014-Abnahme und Aufhebung des Dependencyfreeze.
Keine weiteren gemeinsamen Dateien wurden durch diesen Chat geändert.

Das erste kohärente normative Paket wurde Root zur Prüfung eingereicht:
`/private/tmp/brassclaw-exact-validation-core-text-20261031-5w2f9j_1`.
Es enthält lokale Textdeltas für vier Komponentenguides, Root AGENTS/CLAUDE,
Pure-/Activation-/Deploymentverträge sowie ausschließlich angehängte offene
Nachfolger in Validator, Simplified, Composition, Sempai und Connections.
Die 14 genauen Pfade und Before-/Afterhashes stehen in `manifest.json`, der
begrenzte Patch in `core-text.patch`, Scope-/Embeddingrisiken und Root-only-
Vorschläge in `README.md`. Patch-SHA256:
`4e6fd8ca94b6a34c3f9f13186b3533823908d2a96d89c085acd39590fea19e79`.

Private Textprüfungen bestanden: Beforehashes passten zum Prüfzeitpunkt,
Patch passt auf private Originalpreimages, fünf Ownerpläne streng append-only,
keine hinzugefügten defekten lokalen Linkziele, Whitespace korrekt und ausführbare
Beispiele sowie genaue /1-Approvalfeldtabelle unverändert. Das ist kein Cargo-,
Runtime-, Q1/Q2-, Installations- oder Freigabenachweis.

**Offen:** Rootprüfung/-integration und dann weitere zugeordnete Textpakete.
Keine der 107 Dateidispositionen ist durch private Vorbereitung als kanonisch
abgeschlossen markiert. Vor Integration Beforehashes erneut prüfen und nötigenfalls
gegen neuere Texte rebasen; keine alten vollständigen Dateien zurückschreiben.
Root-only Koordinations-/Status-/Belegdateien und Prefix-exklusiv zugewiesene Pläne
haben keinen Patch. Einbettende Installation-/Prefix-/Releaseinputs benötigen
später ihre vorhandenen Owner, neue Originalbytes und passende Nachfolgebelege;
B1/B2-/Packet-/Goldenbytes wurden nicht verändert.
