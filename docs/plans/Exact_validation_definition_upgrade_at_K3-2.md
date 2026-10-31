# Exact validation definition upgrade at K3-2

**Stand: 31. Oktober 2026. Status: aus dem bisherigen Plan exportiert; noch nicht durchgeführt.**

## 1. Umfang, Herkunft und Arbeitsgrenze

Dieser Plan übernimmt aus
[Exact validation definition upgrade at K3](Exact_validation_definition_upgrade_at_K3.md)
die Schritte für die folgenden beiden Themen; der zusätzlich autorisierte
Scaffoldabbau ist dem Recipeumfang zugeordnet:

1. Vollständige Workflowprüfung neuer oder geänderter Recipes, einschließlich
   ihrer unterstützten Varianten und Abhängigkeiten.
2. Gemeinsame ToolSkills für mehrere Skills, mit eindeutigen technischen Bindings.

Der bisherige Plan besitzt weiterhin ausschließlich die eigenständigen
Komponentenfreigaben, die Abwärtskompatibilität des Nachfolgers und die konsistente
Versionserneuerung durch den bestehenden Katalogowner. Hier wird keine zusätzliche
Freigabeschicht eingeführt und keine jener Arbeiten nochmals implementiert.
Die vorhandene gültige Freigabe-/Runtimebasis ist eine Eingangsbedingung; die
vollständige Durchführung des anderen Dokumentplans ist kein pauschaler Vorgänger.

**Herkunftsregel für alle folgenden Schritte und Dateizeilen:** A bezeichnet neue/
geänderte Komponenten auf einer installierten Instanz, S die vertrauenswürdig
kontrollierte Auslieferung/Upgradequalifikation. A-Q1 prüft eigene Funktionen,
Skills mit ihrem implementierenden PythonCode gemeinsam; es prüft keine zusätzlichen
allgemeinen Versionskombinationen. Für S bleiben bisherige exakte Skillnutzungs-
belege nach bereits freigegebenem Tool und ToolSkill bestehen. Ein solcher Beleg
genehmigt nur die genaue Nutzung des geprüften Skills, nicht die ganze gelistete
Dependencybibliothek. Die verbindlichen gemeinsamen A/S-/Pair-/Reihenfolgeregeln
stehen im [bisherigen Plan §§2.2/2.8/2.9](Exact_validation_definition_upgrade_at_K3.md#28-seedpfad-s-bestehende-kombination-eng-begrenzter-scope-und-reihenfolge).
Dieser Plan führt sie nicht als zweite Implementierung aus. Alle pauschalen
„kein zusätzlicher Kombinationsreview“-Aussagen unten gelten nur für A.
T-Bindingtechnik kann von beiden Wegen verwendet werden.

**Zusätzlicher Betreiberauftrag: keine eigenständige Scaffoldklasse 50.**
Wiederverwendbare parametrisierte Abläufe sind normale Recipes der Klasse21,
gegebenenfalls mit explizitem Aufruf eines anderen vorhandenen Recipes. Der
bisherige Exactplan §2.10/U11 besitzt den Wegfall des Scaffold-Sonderschutzes und
-Validators; dieser Plan besitzt die konkrete Recipezuordnung W-R. Klasse10 und
der globale Orchestrator bleiben eigenständig geschützt. Beide Beiträge werden
über die vorhandenen Owner integriert, nicht als zwei Migrationen implementiert.

Die Durchführung dieses Plans ändert ausschließlich Dokumentation, Architekturtexte
und offene Nachfolger in bestehenden Plänen. Sie erzeugt keinen Rust-/Python-/SQL-/
Frontendcode, keine Migration, keinen neuen Component und keine Aktivierung.
Notwendige technische Arbeiten werden ausschließlich bei vorhandenen Ownern
eingetragen. Laufende/abgeschlossene Schritte, Originalbelege, genehmigte DBbytes
und angewandte Migrationen bleiben unverändert. Dokumentlieferung ist keine
Runtime-/Releaseabnahme. Q1 und menschliches Q2 bleiben die vorhandenen Begriffe;
„V1“ bezeichnet keine neue Prüfphase und keine nur numerische Revision 1.

## 2. Sollvertrag W — Recipe-Workflowprüfung

W1. Für A prüft der normale V1/Q1 ein neues oder inhaltlich geändertes Recipe
als eigenen Workflow vollständig. Für ein ausgeliefertes S-Recipe wird derselbe
fachliche Workflowumfang durch die vorhandene Bootstrap-/Paketqualifikation
nachgewiesen, ohne normalen authored Q1-Queueauftrag. Die Prüfung umfasst alle unterstützten Varianten und Pfade,
Stepordnung/step_link, Inputlayouts, Toolbindungen, ausgewählte Exporte sowie die
direkten/transitiven Abhängigkeiten und deren zulässige Kombinationen. Eine
ununterstützte Kombination darf nicht als ausführbarer Recipepfad gelten.

W2. Prüfen: typed Eingabe-/Ergebnisübergaben, rekursive Schemas, missing/null,
Defaults und Extras, Bedeutung und Effekte, Fehler-/Abbruch-/Retry-/Wait-/Childfälle,
Ergebnis-/Reply-/Historycustody und Completion. Öffentliche Intent-/Commandvarianten
und private trusted Admission haben ihre jeweiligen echten Eingangsbedingungen.
Ein Parserbericht, unvollständiger Testpfad oder terminales Childresultat ersetzt
keine vollständige Workflowevidenz. Erfolgreiche Einzelkomponenten allein beweisen
keine neue fachliche Sequenz.

W3. Alle erlaubten Varianten und Abhängigkeitsprofile müssen durch explizite
Vertrags-/Regelabdeckung und passende echte positive/negative Verhaltensfälle
erfasst sein. Endlose Wertebereiche verlangen keine Enumeration aller Werte oder
aller theoretischen Versionsprodukte. Stattdessen sind relevante Grenzen,
Pfadbedingungen und Beziehungen zwischen Inputs/Outputs/Abhängigkeiten nachzuweisen.
Ungeprüft, interrupted, skipped oder technisch unzureichend bleibt unvollständig.
Keine Fallzahl oder Kapazitätsgrenze wird hier neu erfunden.

W4. Nur die tatsächlich unterstützten Runner-/Occurrence-/Retryprofile dürfen in
die Aktivierung gelangen. Dieser Plan baut keine dynamischen Branch-/Foreach-/
Recoveryfunktionen. Fehlende Unterstützung bleibt beim bestehenden Runtime- oder
V35-13/14-Owner. Das Recipe darf erforderliche Pfade nicht still entfernen, um die
Prüfung zu bestehen. Bestehende No-Match-/Tier-/Effekt-/Kernelregeln bleiben.

W5. Die Prüfung aller angebotenen Recipekombinationen ist die eigene
Workflowvalidierung, keine allgemeine Kombinationsprüfengine für andere
Componentklassen. In A erzeugt sie keine zusätzliche Skillassociationfreigabe.
In S ersetzt sie nicht die vorhandenen exakten Nutzungsbelege seiner Skills.
In beiden Wegen werden vorhandene Einzel-/Funktionsfreigaben der Abhängigkeiten
verwendet und nicht erneut erteilt. Auch wenn Skill und Code zwei gespeicherte
Komponenten sind, wird deren Funktion vor Verwendung im Recipe gemeinsam gemäß
dem bisherigen Plan geprüft; nicht hier nochmals separat als Skill und Code.
Fehlende Voraussetzungsfreigaben bleiben ausstehende eigene Arbeit, kein Erfolg
des Recipes kann sie nachträglich erteilen. Das Prüfmanifest
behält die tatsächlichen exakten damals ausgewählten Revisionen/Belegbytes;
eine spätere kompatible Erneuerung wird nicht als damals beobachteter Testlauf
ausgegeben. Die Regeln dafür besitzt der bisherige Plan.

W6. Vorhandene Funktionsfreigaben ersetzen nicht die echte Workflowausführung:
Ein Recipeprüflauf darf und muss seine tatsächlichen ausgewählten Skill-Exports
aufrufen, um Handoffs, Reihenfolge, Effekte und Completion zu beobachten. Das
startet aber keine erneute eigene Q1-/Q2-Validierung jener Skills, Codekomponenten,
Tools oder ToolSkills. Die funktionalen Teilprüfungen eines neuen Skill-/Codepaars
werden einmal beim vorhandenen Validatorowner durchgeführt; W konsumiert deren
gültige Ergebnisse. Erfolgreicher Recipeablauf genehmigt niemals eine fehlende
Voraussetzungskomponente nachträglich.

W7. Ein als Scaffold bezeichneter wiederverwendbarer Ablauf erhält keine eigene
Klasse, sondern wird mit konkreter Recipeidentität, typed Inputs/Variablen und
vorhandener Recipe-/Childaufrufmechanik beschrieben. Ablauf und angebotene
Parameter-/Variantenübergaben unterliegen W1–W6, keiner zusätzlichen Scaffold-
oder Rootprüfung. Bestehende Kind-/Wait-/Result-/Claimverträge gelten; fehlende
allgemeine Aufrufunterstützung bleibt eine konkrete Aufgabe beim vorhandenen
Runtimeowner, kein Anlass für Scaffoldengine, Spezialtool oder neue VM.
W-R prüft zuerst die tatsächlichen `global_recipe_ports`-/`monty_child_tasks`-
Verbraucher und deren native Admission. Ein Klasse21-UUID in einem bisher nur
PythonCode ausführenden Schritt ist kein unterstützter Recipeaufruf. Keine neue
persistierte Stepform oder hypothetische `host`-API erfinden. Das konkrete Recipe
muss über den vorhandenen qualifizierten Einstieg ausführbar sein; erforderliche
fehlende Aufrufunterstützung wird als offener Nachfolger beim bestehenden Runtime-
owner eingetragen und vor Aktivierung des betroffenen Workflows abgenommen.
Parentclaim, originale Katalog-/Modellauswahl, typed Inputs, Rückgabe, Wait und
Effektzustand bleiben an der tatsächlichen Childgrenze erhalten.

## 3. Sollvertrag T — gemeinsame ToolSkills und eindeutige Bindings

T1. Ein Tool stellt den deklarierten real unterstützten Funktionsumfang einer
technischen Fähigkeit bereit. Optionen/Operationsselektoren erfordern nicht je
einen neuen Rust-Tooltyp. Das bedeutet nicht, dass automatisch jede Funktion
eines externen Programms oder Plugins unterstützt ist. Fachliche mehrstufige
Workflows bleiben Recipes.

T2. Ein ToolSkill beschreibt einen ausreichend vollständigen wiederverwendbaren
Rust-Bindingbereich: Toolidentität, Callable/Adapter, Parameter, Ergebnis-/Fehler-
erwartungen und technische Voraussetzungen. Ein ToolSkill darf mehrere Skills
bedienen. Mehrere ToolSkills sind für sinnvoll trennbare Bereiche eines komplexen
Tools möglich; ein neuer Skill oder eine andere Flagkombination verlangt für
sich allein keinen neuen ToolSkill.

T3. Ein Skill stellt genau eine Funktionsweise eines Tools mit eigener
Nutzungsbeschreibung und preloadbarem Funktionsinterface bereit. Explizit
assoziiertes PythonCode implementiert dieses Interface. Recipes und anderes
PythonCode können den ausgewählten Export unter den unterstützten Bindingregeln
nutzen. Pure Helfer bleiben PythonCode ohne künstliche Skill-/ToolSkillhülle.

T4. Konzeptbeispiel: ein Tool `ls`, ein ausreichender ToolSkill `ls`, drei Skills
für `ls`, `ls -l` und `ls -la`. Unterschiedliche Nutzungsverträge/Exporte teilen
das Binding. Für die reale Umsetzung werden vorhandene Datei-/Shellprimitive
gesucht und verwendet; dieses Beispiel beauftragt kein neues Tool, keinen Fake-
adapter und keine erzwungenen drei Wrapper. Echte Shell-Recipes bleiben Tier 1.

T5. Vorhandene geeignete Code-/Exportidentitäten und Bindingdeskriptoren zuerst
nutzen. Der aktuelle Resolver verlangt genau einen Skill pro ausgewähltem
PythonCode-/ToolSkill-/Tool-/Callabletupel; mehrere Skills mit identischem Tupel
dürfen nicht durch First-Match, Prosa oder einen zusätzlichen Anzeigenamen
auseinandergehalten werden. Verschiedene echte Code-/Exportverträge können
bereits ausreichend unterscheidbar sein. Der gegenwärtige Lookup enthält aber
keinen Exportnamen: unterschiedliche Exporte derselben Code-UUID machen ein
ansonsten identisches Tupel heute nicht eindeutig. Nur bei tatsächlich nötiger
gemeinsamer Code-UUID muss T-A die explizite Skill-/Exportauswahl durch alle
betroffenen Interface-/Invocation-/Retentionsreader desselben Owners führen;
keine künstlichen Codekopien oder zusätzlichen Freigaben als Umgehung. Nur eine konkret nachgewiesene Lücke
rechtfertigt eine Erweiterung des bestehenden Resolver-/Invocationowners.
Keine zweite Registry, kein neuer Skillloader und keine duplizierte Primitive.

T6. Das Interface, die Preloaddeklaration, die Recipe-Invocation, der ausgewählte
Skill und die native Bindingidentität müssen technisch eindeutig zusammenpassen.
Dies ist technische Auflösung; ihre Freigabesemantik und der gerichtete Umgang
mit kompatiblen Schemaänderungen bleiben Auftrag des bisherigen Plans U1/U4.
Keine neuen JSONfelder oder APIs als bereits unterstützt ausgeben.

T7. Unverändert gelten: ein UUID-Verweis je Komponentenschritt, passendes
Rust-ToolSkillbinding vor der ausführbaren Nutzung und getrennte Steps für
unabhängige Toolaufrufe. Ein aufgerufener Export erbt nur seinen ausgewählten
ausdrücklich abgedeckten Bindingkontext, keinen ambienten Toolzugriff. Laden
hat keine Nutzungseffekte; mutable Defaults/Inputs/Ergebnisse bleiben isoliert.
Nicht unterstützte Kettenausnahmen werden nicht freigeschaltet. Live Toolpolicy
und technische Ressourcenlimits gelten weiter vor jedem tatsächlichen Dispatch.

## 4. Durchführung: nur Dokument- und Planänderungen

E1. Die aktuellen Zielfundstellen aus §6, den Statusindex und die Buildqueue
lesen. Aktuelle Owner/Implementierungsgrenzen feststellen; keine historischen
K-Schritt- oder Buildnummern als dauerhaft laufenden Stand voraussetzen. Vor
jedem Edit frischen Zielkontext, Einbettung und erfasste Buildinputs prüfen.
Eingebettete Guides nicht während fremder eingefrorener Prüfung ändern.

E2. In den vier Komponentenguides ausschließlich W-/T-Regeln, dazugehörige
Beispiele und Checklisten präzisieren und dabei jede aktive normative Stelle
unmittelbar nach A/S abgrenzen. A-Recipeprüfung gehört zum normalen Q1;
S-Workflow-/Skillnutzungsqualifikation bleibt beim vorhandenen Bootstrapowner. Bestehende Store-/Runnerformate als Iststand
erhalten. Eigene Freigaben/Kompatibilität/Katalogupdates weder hier neu definieren
noch als zweite Umsetzung in die Guides eintragen.

E3. Die Aufträge aus §5 mit konkreter Arbeit, wiederverwendeten Schnittstellen,
Abnahme und Ein-/Ausgangskanten in bestehende Pläne eintragen. Ausschließlich
offene Nachfolger hinter dem bei Durchführung tatsächlich laufenden Paket;
K0–K3 und später fertiggestellte oder laufende Teilabschnitte nicht umschreiben.
Kein K8, neuer Validatorplan oder zusätzliche Implementierungscheckliste.
Nächste noch offene betreffende Aktivierungs-/Cutovergates müssen die Aufträge
verbindlich konsumieren. Fehlende aktuelle Einzelstepmarkierung nicht erfinden.

E4. Die lokalen Dateischritte aus §6 umsetzen und geschützte historische Dateien
unverändert lassen. Gemischte Zielpassagen mit dem bisherigen Plan abschnittsweise
abgleichen: derselbe Owner erhält einen zusammengeführten Delta, keinen zweiten
Writer/Parser/Reviewauftrag. Source-/Seed-/Manifeständerungen nur in Ownerplänen
beauftragen; hier ausschließlich Dokumenttext ändern.

E5. Alle Dateieinträge, lokalen Links, Spiegel und die Exportzuordnung aus §8
prüfen. Notwendige offene Dokumentedits verhindern einen vollständigen Abschluss;
No-Edit nur bei bereits passender Aussage oder geschützter Historie begründen.
Neue technische Arbeit darf offen bleiben, wenn sie im vorhandenen Plan wirklich
mit konsumierendem Gate eingetragen ist. Scoped Diff/Whitespace prüfen; neue
untracked Dateien zusätzlich gegen `/dev/null` prüfen. Keine Cargoausführung nur
für Prosa, keine Aktivierung, keine automatische Nachricht an andere Chats.

## 5. In vorhandene Pläne einzutragende technische Folgeaufträge

Diese Tabelle wird bei Durchführung nur als Text in die existierenden Pläne
übertragen. Sie ist kein Auftrag, im Rahmen dieses Dokumentplans Code zu erzeugen.

| ID | Bestehender Owner / zulässiger offener Nachfolger | Arbeit, Wiederverwendung und Ausgangskante |
| --- | --- | --- |
| W-A | [validator_v3.md](../../validator_v3.md), nach laufendem Validatorpaket; fachlich bestehende Klassen-21-/Steps-6–10-Arbeit | W1–W7 in die bestehende Recipevalidatorroute integrieren: vollständige Regeln/Varianten-/Behaviorabdeckung, echte Subject-/Validator-/Run-/Interpreterprovenienz, gespeicherte Fehl-/Unvollständigkeitsfälle. Bestehende Coverage-/Observer-/Evidencewriter verwenden. Keine zweite Q1-/Q2-Route. A-Regelchecks für gültige Skill-/Codefunktionen nicht als erneute Candidateprüfung ausführen; S nutzt Bootstrapworkflowevidenz und vorhandene Skillnutzungsbelege. Ausgang: nächstes noch offenes Recipevalidator-/Produktcutovergate. |
| W-B | [monty.composition.md](monty.composition.md) und vier Anhänge, nach aktueller Familie | W-Regeln in künftige neue/geänderte Recipefamilien übernehmen, tatsächliche IBSsyntax/Bindinglayout/Inputrefs und End-to-end-Completion nachweisen. Keine abgeschlossenen RCP-/SKR-Belege umschreiben. Ausgang: offene betroffene Familien-/R09-/Recipeabnahme und V35-05/06. |
| W-R | Vorhandener Composition-/Recipe-/Revisionsmigrationsowner, mit Simplified Phase6/8 beim nächsten offenen Nachfolger | Tatsächlichen Klasse50-Bestand/Consumer inventarisieren, passende vorhandene Klasse21-Recipes verwenden, nötige fachliche Nachfolger normal prüfen und neue Consumerreferenzen explizit zuordnen. Keine aktive Scaffoldklasse/-validator/-registry/-dispatchschicht, keine neue `reborn_scaffolds`-Tabelle. U11 des bisherigen Plans liefert nur Freigabe-/Trustscope. Historische class50-Bytes/Pins unverändert lesen; neue Aktivierung/Erstellung class50 beenden. Ausgang: offene betroffene Recipe-/Katalog-/UI-/Upgradeabnahme durch denselben Migrationsowner. |
| T-A | Derselbe Compositionowner, offener Binding-/Exportnachfolger | T1–T7 und tatsächlich vorhandene Identitäten prüfen. Gemeinsames Binding zuerst mit vorhandenen APIs verwenden; nur belegte Mehrdeutigkeit im bisherigen `retained_tools`-/Preload-/Invocationowner beheben. Keine Code-/Toolkopien oder zweite Registry. Ausgang: offene betroffene Familien-/Bindingabnahme; Freigabescope und Schemavarianz vom bisherigen Plan konsumieren. |
| WT-C | [simplified_v3.md](../../simplified_v3.md), ab nächstem offenen K4-/Phase-0a/3a-Nachfolger | Nur fehlende mechanische gemeinsame Runtime-/Bindingschnittstellen an den existierenden Owner weitergeben. Keine zweite global VM, Scheduler oder autonome Rustworkflowprüfung. U3 des bisherigen Plans bleibt einziger Aktivierungsowner. Ausgang: nächste offene tatsächlich betroffene Runtime-/Integrationsabnahme. |
| WT-D | Validator noch offene Steps 11/12, Simplified noch offene K5/K6-/Phase-6/8-Arbeit | Nur tatsächlich geänderte W-/T-Verträge/Guide-/Seedbytes beim bestehenden Bootstrapowner qualifizieren. S-Kombinationsarchitektur erhalten, Scope pro Skillnutzung und vorherige Tool-/ToolSkillfreigaben nachweisen; Skill/Code gemeinsam. Kein Routing von S durch die normale A-Q1-Queue. Historische Revisionen/Override-/Taskpins erhalten, kein zusätzliches installiertes Q2. Keine neue allgemeine GC-/Recoveryimplementierung aus diesem Plan. Ausgang: betroffene offene Upgrade-/Restoreabnahme. |
| WT-E | [prefix_v3_upgrade.md](prefix_v3_upgrade.md), nach laufendem Sources-/Teachingpaket | Nur geänderte W-/T-Lehrquellen, Beispiele und Paketidentitäten über bestehenden Capture-/Teachingowner übernehmen. Keine zweite Source-/Seedpipeline, keine B1/B2-Byteänderung oder Free-/Bootstrapfreischaltung. Ausgang: betroffene offene Source-/Teaching-/Paketabnahme. |
| WT-F | [sempai_v3.5.md](sempai_v3.5.md), [simplify_connections_v3.5.md](simplify_connections_v3.5.md), [path_to_v3.5.md](../../path_to_v3.5.md), nach ihren aktuellen Paketen | Neue/geänderte Sempai-/Referenzworkflows konsumieren W und gemeinsame Nutzungen T. Bestehende V35-02/05/06/07/16/30 nur in offenen Folgeabschnitten ergänzen; Flow/Occurrences bleiben V35-13/14. Kein zweiter Validator-, Connection- oder Deploymentowner. Ausgang: offene tatsächliche v3.5-Familien-/Produktgates. |
| WT-G | Vorhandene Validator-/Operator-/MCP-Owner im nächsten offenen Folgepaket | Vollständigen eigenen Recipeprüfungsumfang und eindeutige Nutzung im bestehenden UI/API zeigen; öffentliche Commandqualifikation und private Admission getrennt abnehmen. Für A kein Formular/Belegwriter einer zusätzlichen Versionskombinationsfreigabe; für S bestehende genaue Skillnutzungsbelege und Voraussetzungsscope herkunftsrichtig anzeigen, ohne installiertes Q2-Tor. Ausgang: vorhandene betroffene Operator-/Command-/MCP-Abnahme. |

Die W- und T-Beiträge können ihre vorhandenen Mechaniken gemeinsam nutzen. Der
Validatorowner besitzt Prüfsemantik und Evidence; Composition besitzt Nutzungen,
Graphen und technische Bindings; Simplified liefert fehlende gemeinsame Runtime.
Die bestehenden Installation-/Prefixowner besitzen Integrität und Quellenaufnahme.
Es entsteht kein neuer Freigabe-/Publikationsowner. Eine fehlende nicht benötigte
T-Erweiterung blockiert keine bereits eindeutig darstellbare W-Prüfung; umgekehrt
erfordert ein unveränderter gültiger Workflow wegen T keine neue Vollprüfung.

## 6. Dateiweise exportierte Dokumentänderungen

Für sämtliche folgenden Dateizeilen gilt die lokale A/S-Pflicht aus §1:
Workflowregel W wird für A in Q1 optimiert, für S beim Bootstrapowner belassen.
Bestehende S-Nutzungsbelege werden niemals pauschal entfernt oder als eigene
Freigaben aller beteiligten Components beschrieben. Skill/Code ist vorab ein
gemeinsam geprüfter Funktionsgegenstand, nicht zwei erneut zu testende Dependencies.
Die Basis-ID verweist auf die stabile Dateinummer im bisherigen Plan. Dieselbe
Datei kann dort einen anderen Abschnitt besitzen; nur der folgende W-/T-Delta
wird hier geändert. Allgemeine Freigabe-/Kompatibilitäts-/Auswahländerungen bleiben
im bisherigen Plan. Die Liste ist keine Pflicht zu neuen Boilerplateblöcken:
bereits richtige Aussagen werden mit konkreter Begründung unverändert gelassen.

| Basis-ID | Datei | Ausschließlich exportierter Dokumenteingriff |
| --- | --- | --- |
| 01 | [.agents/skills/reborn-feature/SKILL.md](../../.agents/skills/reborn-feature/SKILL.md) | W-/T-Owner im Entwicklungsrouting trennen: Recipeprüfung zum bestehenden Validator, gemeinsame Nutzungen/Resolver zu Composition. Keine neue Runtime-Skilldefinition im Entwicklerskill. Spiegel 03 gleichhalten. |
| 02 | [.claude/rules/skills.md](../../.claude/rules/skills.md) | W-Prüfungsumfang und T-Nutzung vom Entwickler-Skillbegriff abgrenzen; keine Pflicht zu eigenem Recipe/Binding pro Runtime-Skill einführen. |
| 03 | [.claude/skills/reborn-feature/SKILL.md](../../.claude/skills/reborn-feature/SKILL.md) | Gleichen W-/T-Routingdelta wie 01 bytegleich übernehmen; Metadaten und Invocationpolicy erhalten. |
| 04 | [AGENTS.md](../../AGENTS.md) | Nur Workflowprüfungsanforderungen und Binding-/Skill-/ToolSkillkardinalität in aktuellen Binding-/Authoringblöcken angleichen. Versionskombinationsfreigabe bleibt beim bisherigen Plan. |
| 05 | [CLAUDE.md](../../CLAUDE.md) | W-/T-Regeln wie 04 sowie Capabilitybeispiel angleichen; historischen Iststand der Resolver/Beispiele lokal erhalten. |
| 06 | [README.md](../../README.md) | Preloadübersicht für mehrere Skills pro ToolSkill präzisieren; vollständige Recipeprüfung knapp zum Validatorowner verlinken, keine fertige Umsetzung behaupten. |
| 07 | [crates/AGENTS.md](../../crates/AGENTS.md) | Gemeinsamen Komponentenblock W/T und vorhandene Rollen aufnehmen; kein neuer Validator-/Bindingowner. |
| 10 | [engine/AGENTS.md](../../crates/brassclaw_engine/AGENTS.md) | Eindeutige ausgewählte Nutzungs-/Export-/Bindingidentität und bestehende Stepgrenzen lehren; keine spekulative neue Resolverimplementierung beauftragen. |
| 11 | [engine/CLAUDE.md](../../crates/brassclaw_engine/CLAUDE.md) | W-/T-Assemblybeispiele mit bestehenden Readergrenzen abgleichen; technischer Runner ersetzt nicht die vollständige eigene Recipeprüfung. |
| 12 | [engine/MONTY.md](../../crates/brassclaw_engine/MONTY.md) | Bindingkontext und Preload-/Invocationisolierung aus T7 prüfen; Interpreterpin und historische ABIbelege erhalten. |
| 13 | [extensions/AGENTS.md](../../crates/brassclaw_extensions/AGENTS.md) | Tooloperationsumfang, gemeinsame ToolSkillbereiche und mehrere Skillnutzungen im Bindingkopf erklären; kein Tool je Flag. |
| 14 | [extensions/CLAUDE.md](../../crates/brassclaw_extensions/CLAUDE.md) | T-Rollen aus 13 und technische Registrierung/Hostgrenzen angleichen. |
| 15 | [host_runtime/AGENTS.md](../../crates/brassclaw_host_runtime/AGENTS.md) | T-Bindings nur über native Callable-/Adapter-/Artefaktidentität darstellen, Metadaten führen nichts aus; keine Workerprüfengine. |
| 16 | [host_runtime/CLAUDE.md](../../crates/brassclaw_host_runtime/CLAUDE.md) | Gleiche T-Auflösung wie 15; kein Getter/Flag gilt als Nachweis realer Nutzungsbindung. |
| 23 | [composition/AGENTS.md](../../crates/brassclaw_reborn_composition/AGENTS.md) | W-/T-Routing im aktuellen Bindingblock mit den bestehenden Composition-/Validatorownern verknüpfen; fehlende Resolverprofile nicht als unterstützt erklären. |
| 24 | [composition/CLAUDE.md](../../crates/brassclaw_reborn_composition/CLAUDE.md) | Offene W-/T-Familien-/MCP-/Deploygates an bestehende Owner anschließen; keine Aktivierungskopie. |
| 27 | [skills/AGENTS.md](../../crates/brassclaw_skills/AGENTS.md) | Mehrere eigenständige Skillnutzungen mit einem ausreichenden ToolSkill und expliziter Code-/Exportzuordnung darstellen. |
| 30 | [webui_v2/AGENTS.md](../../crates/brassclaw_webui_v2/AGENTS.md) | UI zeigt eigenen Recipeprüfungsumfang und tatsächliche Skill-/ToolSkillzuordnung, ohne neue APIs zu erfinden. |
| 31 | [webui_v2/CLAUDE.md](../../crates/brassclaw_webui_v2/CLAUDE.md) | Gleiche W-/T-Darstellung wie 30; vorhandene Facade-/Operatorgrenzen erhalten. |
| 32 | [development-policy.md](../development-policy.md) | Evidenzwahl für geänderte Recipes um W-Abdeckung präzisieren, gemeinsame Bindingwiederverwendung T lehren; Freigabescope-/Kompatibilitätsänderung nicht doppelt beauftragen. |
| 33 | [implementation-status.md](../implementation-status.md) | Additiven offenen W-/T-Nachfolger und echte Supportlücken verlinken; Dokumentdelta nicht als Runtime-/K-Abschluss ausgeben. |
| 34 | [Recipe_Reviewed_v3_plan.md](Recipe_Reviewed_v3_plan.md) | Offenen W-Nachfolger für neue/geänderte Recipevarianten und T-Verweise eintragen; alte Composer-/Finalisierungshistorie erhalten. |
| 35 | [monty.composition.implementation.md](monty.composition.implementation.md) | „Shared usage and export selection“ im offenen Nachfolger um gemeinsame ToolSkills und eindeutige vorhandene Exportidentitäten präzisieren; W-Abdeckung der tatsächlichen Beispiele eintragen. |
| 36 | [monty.composition.md](monty.composition.md) | W-B/T-A mit konkreter Familien-/Bindingabnahme und Eingang-/Ausgangskanten eintragen; keine bisherigen Migrationsergebnisse neu öffnen. |
| 37 | [monty.composition.recipe-implementation.md](monty.composition.recipe-implementation.md) | Offene RCP-Anweisung auf vollständige Prüfung erstmaliger/geänderter Workflows samt Varianten/Input-/Result-/Fehler-/Childpfaden erweitern. Bestehende Nutzungen und abgeschlossene Blöcke erhalten. |
| 38 | [monty.composition.recipes.md](monty.composition.recipes.md) | Offenen W-Nachfolger für die Erstmigration unvalidierter Workflows und deren tatsächliche Abhängigkeiten ergänzen; T-Nutzungen wiederverwenden. |
| 39 | [monty.composition.skill-recipes.md](monty.composition.skill-recipes.md) | Offene SKR-/MCP-/Referenzabnahme übernimmt W und T; kein eigener Recipezwang pro Skill und keine automatische MCP-Freigabe durch Skill-Export. |
| 40 | [prefix_v3_upgrade.md](prefix_v3_upgrade.md) | WT-E in offenen Sources-/Teachingnachfolger eintragen; neue W-/T-Lehrquellen und echte Paketinputs erfassen, B1/B2 unverändert. |
| 41 | [sempai_v3.5.md](sempai_v3.5.md) | A-Sempai-/Operatorvorschläge über normale eigene Recipe-/Funktions-Q1 prüfen; Step10-shipped Bibliothek bleibt S mit vorab freigegebenen Voraussetzungen und genauer Nutzungskombination. Gemeinsame passende Bindings weiterverwenden, keine neue Kombinationenqueue. |
| 42 | [simplified-v3-implementation.md](simplified-v3-implementation.md) | Nur neuen offenen W-/T-Runtimeanschluss referenzieren; historische Journale/Tests nicht rückwirkend ändern. |
| 43 | [simplify_connections_v3.5.md](simplify_connections_v3.5.md) | PhaseF konsumiert A-Funktions-/Recipe-Q1 oder S-Bootstrap-/Nutzungsbelege nach Herkunft; vorhandene gemeinsame Bindings und Stores, keine eigene fachliche Kombinationenprüfung. |
| 44 | [zencoder-extension-plan.md](zencoder-extension-plan.md) | Offene Zencoder-Recipequalifikation übernimmt W; mehrere passende Nutzungen teilen vorhandene Bindings. Handler-/Registrierungslücken bleiben explizit. |
| 45 | [component-deployment.md](../reborn/component-deployment.md) | Eigene Recipeprüfung W von Prepare/Activation unterscheiden; T-Nutzungszuordnung technisch darstellen. Keine neue Deployment- oder Approvalroute. |
| 46 | [retained-component-inputs.md](../reborn/contracts/retained-component-inputs.md) | Bestehende strikte Tupelauflösung als Iststand erhalten; für gemeinsame Nutzungen eindeutige Auswahl und W-Handoffs beschreiben. Schemavarianz/Freigabeformatänderung ist ausschließlich im bisherigen Plan. |
| 48 | [raw-file-snapshot.md](../reborn/raw-file-snapshot.md) | Eigene vollständige W-Prüfung des konsumierenden Dateireadworkflows präzisieren; Rawcapturebeleg ist kein Workflowabschluss. |
| 49 | [source-inventory-inspection.md](../reborn/source-inventory-inspection.md) | Tool mit zwei Inputprofilen und gemeinsamem ToolSkill als T-Referenz nutzen; öffentlichen Command-/privaten Validatorworkflow mit W und ihren verschiedenen Admissionbedingungen abgrenzen. |
| 50 | [recipe-first-architecture-audit.md](../recipe-first-architecture-audit.md) | Aktuelle W-/T-Routingempfehlung ergänzen; historische Quellbeobachtungen erhalten und keine neue Komponenteneigentümerschaft aus Referenzrecipes ableiten. |
| 51 | [path_to_v3.5.md](../../path_to_v3.5.md) | WT-F an bestehende offene V35-02/05/06/07/16/30 anschließen; W-/T-Anteile von Approval-/Versionsarbeit unterscheiden. Historische Spezifikation 107 unverändert lassen. |
| 52 | [prefix_plan2.md](../../prefix_plan2.md) | Nur zum aktuellen offenen WT-E-Nachfolger routen; historischen Prefixplan nicht zum Implementierungsowner machen. |
| 53 | [recipe.md](../../recipe.md) | W-Prüfungsumfang für neue/geänderte Recipes, alle unterstützten Varianten/Pfade/Abhängigkeiten und T-Bindingregeln in Ownership/Checklisten/Beispielen präzisieren. Reale persisted IBSsyntax erhalten. |
| 54 | [sempai-authoring-reference.md](../../scripts/prefix/sempai-authoring-reference.md) | Authoring um vollständige eigene Recipeprüfung und gemeinsame ToolSkills mit eindeutigen Skill-Exports ergänzen; keine neue Approvalebene. |
| 55 | [sempai-monty-compatibility.md](../../scripts/prefix/sempai-monty-compatibility.md) | T-Export-/Bindingkontext und bestehende Interpreter-/Preloadgrenzen darstellen; CPythonprobe ist kein Nachweis vollständiger Recipeabdeckung. |
| 56 | [sempai-worked-examples.md](../../scripts/prefix/sempai-worked-examples.md) | Vollständiges W-Beispiel und ls-/gemeinsames Bindingbeispiel T ergänzen; vorhandene echte Hilfen/Fixtures wiederverwenden und historische Ergebnisse nicht überschreiben. |
| 57 | [simplified_v3.md](../../simplified_v3.md) | WT-C/WT-D im nächsten offenen K4-/K5-/K6-/Regelplan-Nachfolger verankern; aktuelle K3-/Admissionpakete nicht ändern. |
| 58 | [skills.md](../../skills.md) | Ein Skill = eine Toolnutzung und ein ausreichend breiter ToolSkill für mehrere Nutzungen erläutern; Code-/Exportzuordnung und vorhandene Auflösungsgrenzen deutlich halten. |
| 59 | [tools.md](../../tools.md) | Deklarierten vollständigen Primitiveumfang mit mehreren Operationen und weiterhin Recipe-eigene fachliche Sequenz erklären; keine Toolduplikate pro Flag. |
| 60 | [toolskills.md](../../toolskills.md) | Kardinalität Tool zu einem/mehreren ToolSkillbereichen zu mehreren Skills, konkrete Bindingprofile und T-Eindeutigkeit erklären; Metadaten bleiben nicht ausführbar. |
| 61 | [validator_v3.md](../../validator_v3.md) | A-Klassen-21-Q1 prüft alle angebotenen Recipekombinationen mit vorab freigegebenen Skill-/Codefunktionen; keine zusätzliche Einzel-/Kombinationsreviewkaskade. S-Recipequalifikation verbleibt beim Step11-Bootstrapowner mit genauen Skillnutzungsbelegen. W-A/T-A im offenen Nachfolger eintragen, keine neue Engine. |
| 64 | [component-review-submissions.md](../reborn/contracts/component-review-submissions.md) | W-Subject benötigt alle tatsächlichen Layout-/Varianten-/Dependency-/Sourcebezüge; vorhandene sealed Submission-/Basebytes behalten. Kein neuer Store. |
| 65 | [workflow-review-evidence.md](../reborn/contracts/workflow-review-evidence.md) | W-Abdeckung mit ausgewählter Variante/Stepordnung/Erwartungen und tatsächlichen positiven/negativen Beobachtungen erklären. Flatprofile und historische V106-Belege nicht zu allgemeiner Flowunterstützung aufwerten. |
| 66 | [validator-coverage.md](../reborn/contracts/validator-coverage.md) | W-Abdeckung der erforderlichen Regel-/Pfadmenge in der vorhandenen Coverage-/Collector-/Aggregatekette präzisieren; unvollständige Fälle bleiben unvollständig, keine erfundenen Rule-IDs im alten Format. |
| 67 | [structural-presence-check.md](../reborn/contracts/structural-presence-check.md) | Structural presence ist nur eine W-Teilregel, keine vollständige Recipeprüfung; im vorhandenen Klassenworkflow mit anderen erforderlichen Regeln verwenden. |
| 69 | [source-inspection.md](../reborn/source-inspection.md) | Reale Source-Inspection-Usage/Exports und gemeinsames Binding als T-Referenz nutzen; Command-/private Validatorpfade getrennt nach W qualifizieren. |
| 70 | [global-monty-task-factory.md](../reborn/contracts/global-monty-task-factory.md) | Bestehende Factory-/ResultOnly-/Child-/Matchgrenzen als W-Eingangsbedingungen beschreiben; keine zusätzliche Taskfactory oder neue Trustroute. |
| 71 | [completed-turn-sempai-review.md](../reborn/contracts/completed-turn-sempai-review.md) | Neue/geänderte Completed-turn-/Precall-Recipes jeweils nach eigenem W-Vertrag prüfen und nur passende T-Nutzungen teilen; ihre Zwecke nicht vermischen. |
| 73 | [mcp-recipe-discovery.md](../mcp-recipe-discovery.md) | Eigene W-Command-/Chatbridgequalifikation und T-Exportauflösung beschreiben; Discoverymetadaten/aktive Präsenz ersetzen keine realen Commandfälle. |
| 74 | [monty_host/README.md](../../crates/brassclaw_monty_host/README.md) | Inspektions-/Workerbericht von W-Workflowbeleg unterscheiden; keine neue fachliche Prüfengine im Montyhost. |
| 75 | [brassclaw-architecture.md](../brassclaw-architecture.md) | System-/Skill-/Compositiondarstellung auf W-/T-Ownership und vier Guides ausrichten; historischen Autoritätsanspruch nicht als neuen Owner verwenden. |
| 76 | [agents-v3/README.md](../agents-v3/README.md) | Gültigen Kopf und Kapitelrouting auf W-Prüfung/T-Kardinalitäten angleichen; keine zweite Freigabesemantik lehren. |
| 77 | [01-architecture-overview.md](../agents-v3/01-architecture-overview.md) | Architekturbild um gemeinsamen ToolSkill für mehrere eindeutige Skillnutzungen und Recipe-eigene W-Prüfung präzisieren. |
| 79 | [03-recipe-system.md](../agents-v3/03-recipe-system.md) | Neue/geänderte Recipes mit allen unterstützten Varianten/Pfaden/Abhängigkeiten nach W prüfen; echte persistierte Syntax und Quellenrollen erhalten. |
| 80 | [04-ibs.md](../agents-v3/04-ibs.md) | IBS liefert technische W-Graph-/T-Bindingauflösung, keine semantische Workflowgenehmigung; keine neue Resolverpipeline. |
| 81 | [05-skills-system.md](../agents-v3/05-skills-system.md) | Drei ls-Nutzungen bei gemeinsamem Tool/ToolSkill T und echte Exporte als Lehrbeispiel darstellen; Purecode bleibt eigenständig. |
| 82 | [06-tools-system.md](../agents-v3/06-tools-system.md) | Tooloperationen, ToolSkillbereiche und Skillnutzungen nach T erklären; ein neues Flag ist kein neuer Tooltyp. |
| 83 | [07-pythoncode-system.md](../agents-v3/07-pythoncode-system.md) | Aufruf ausgewählter Skill-Exports aus PythonCode mit T7-konformem Bindingkontext erklären; Source-/Daten- und Isolationsgrenzen erhalten. |
| 84 | [08-actions-system.md](../agents-v3/08-actions-system.md) | Aktuellen W-/T-Kopf abgleichen; historische Actionschemata sind keine alternative Workflow- oder Bindingarchitektur. |
| 85 | [09-sempai-kohai.md](../agents-v3/09-sempai-kohai.md) | W-Prüfung neuer/geänderter Sempai-/Kohai-Recipes und T-Wiederverwendung präzisieren; Promptkorrektur und Completed-turn-Review getrennt. |
| 89 | [13-orchestrator-default-py.md](../agents-v3/13-orchestrator-default-py.md) | Orchestrator konsumiert W-geprüfte Workflows und eindeutig gebundene Exporte T; keine neue Laufzeit-Validierung vor jeder Invocation. |
| 90 | [14-validation-queue.md](../agents-v3/14-validation-queue.md) | Eigene Recipeprüfung W von einzelnen Teilregel-/Queueerfolgen unterscheiden; Klassenroute bleibt bestehender Validatorowner. |
| 91 | [15-component-catalog.md](../agents-v3/15-component-catalog.md) | Kardinalität der Tool-/ToolSkill-/Skillkomponenten nach T im vorhandenen Klassensystem darstellen, keine neue Componentklasse. |
| 92 | [16-kernel-composition.md](../agents-v3/16-kernel-composition.md) | W-Review vor Aktivierung und technische T-Auflösung von Kernelpolicy trennen; kein neuer Kernelvalidator. |
| 93 | [17-webui-prefix-tab.md](../agents-v3/17-webui-prefix-tab.md) | W-Prüfungsstatus und T-Nutzungszuordnung in bestehender UI erklären; historische UI-/Generationbeispiele als solche erhalten. |
| 94 | [DOC_CONVERSION_MECHANISM_DESIGN.md](../agents-v3/DOC_CONVERSION_MECHANISM_DESIGN.md) | Nur aktuellen offenen W-/T-Nachfolger verlinken; historischen Docconversionmechanismus nicht wieder implementieren. |
| 95 | [architecture_prefix.md](../../crates/brassclaw_engine/prompts/architecture_prefix.md) | Aktive W-/T-Lehrabschnitte im Architekturprompt lokal korrigieren; Einbettung vor Edit klären. Historische SQL-/Seedbelege erhalten, keine Sourceänderung hier. |
| 99 | [product-manager-architecture-guide.md](../reborn/2026-04-27-product-manager-architecture-guide.md) | Aktuelle Produktorientierung zu eigener W-Recipeprüfung und T-Kardinalitäten ergänzen; alte Invocationapprovalbeispiele nicht zur neuen Komponentenebene erklären. |
| 100 | [Exact_V3_definition_update_at_K3.md](Exact_V3_definition_update_at_K3.md) | Nur offenen W-/T-Nachfolger referenzieren, falls noch passende Folgearbeit besteht; ursprüngliche D-Pakete/Dispositionen/Lieferbelege erhalten. |
| 101 | [Exact_V3_definition_update_at_K3-2.md](Exact_V3_definition_update_at_K3-2.md) | Nach laufendem I-Paket W-/T-Anschluss bei tatsächlichen Bootstrap-/Reviewworkflows eintragen; B1/B2-Bytes, Originalinput-/Admission-/Freegates unverändert. |
| 103 | [contracts/AGENTS.md](../reborn/contracts/AGENTS.md) | Dokumentonly-W-/T-Präzisierung und spätere bestehende technische Owner unterscheiden; allgemeine Rust-Synchronisationsregel erzwingt hier keine Codearbeit. |
| 105 | [_contract-freeze-index.md](../reborn/contracts/_contract-freeze-index.md) | Aktuelle Weiterverweise auf W-/T-Guideowner ergänzen, falls tatsächlich nötig; alten Freeze nicht als zweite Umsetzung verwenden. |
| 106 | [skills-extension.md](../reborn/contracts/skills-extension.md) | Nur aktuelle V3-Supersession um gemeinsame T-Bindings und W-Consumerprüfung präzisieren; historischen SKILL.md-Bundlevertrag nicht wiederbeleben. |
| 107 | [v3.5 Technical specification](../../.zenflow/tasks/strictly-follow-the-following-wo-6326/spec.md) | No-Edit: historische abgeschlossene Planungsspezifikation unverändert. Ihren W-/T-Bezug ausschließlich im aktuellen Roadmap-Nachfolger 51 einordnen. |

Nicht betroffene eigene Freigabe-/Kompatibilitäts-/Versionsdateischritte verbleiben
im bisherigen Plan. Geschützte historische Receipts, Archive, Experimente und
datierte abgeschlossene Spezifikationen werden weder dupliziert noch umgeschrieben.
Ein real neu entdeckter aktueller W-/T-Consumer wird mit konkretem lokalen Delta
in diese Liste aufgenommen. Die Anzahl der Dateizeilen ist keine feste Scopegrenze.

## 7. Einzutragende Abnahmefälle und Abschluss

Die Präzisierungen aus §11 sind bei E3 in die jeweiligen vorhandenen Owneraufträge
W-A/W-B/W-R/T-A/WT-C–G zu übertragen; kein unverbundener späterer Auditauftrag.
Die Fälle werden in die existierenden Matrizen der Owner aus §5 eingetragen;
dieser Dokumentplan führt keine Runtimeprüfungen aus. Reale passende Belege
wiederverwenden, keine künstliche Fallquote und keine neue Testinfrastruktur.

| Fall | Erforderliches Ergebnis / Owner |
| --- | --- |
| Neues oder geändertes Recipe mit mehreren Varianten | Jede unterstützte Variante mit tatsächlicher Stepordnung, Input-/Result-/Dependencyprofil und positiven/negativen Fällen erfasst; eine fehlende Variante kann nicht durch erfolgreichen anderen Pfad bestehen. Recipevalidator/Composition. |
| Ungültiger Handoff oder fehlerhafter späterer Schritt | Missing/null/falscher Typ/ungeprüfte Rückgabe stoppt vor nachfolgendem Effekt; bereits abgeschlossene Effekte werden nicht zur Reparatur wiederholt. Bestehender Runner/Validator. |
| Fehler-/Wait-/Child-/Retry-/Abbruchpfad | Nur tatsächlich unterstützte Pfade werden aktiviert; Originalclaim/Invocation und reale Completioncustody nachgewiesen. Unsupported Branch-/Occurrenceprofile bleiben offen beim bisherigen Owner. |
| Manipulierte, unvollständige oder fremde Workflowbelege | Fehlende Regeln, falsche Variante/Ordnung, vertauschte Subject-/Validator-/Runidentität, skipped/interrupted und ungewisse Ergebnisse erzeugen keinen positiven Workflowbeleg. Existing Coverage-/Evidenceowner. |
| Deklarierte Beziehungen mehrerer Abhängigkeiten | Recipe-W-Prüfung deckt Bedeutung und Relationen ab; erfolgreiche individuelle Components allein genehmigen keine beliebige neue Sequenz. Aktivierungsmechanik bleibt beim bisherigen Katalogowner. |
| Drei unterschiedliche Skillnutzungen mit einem ToolSkill | Reale passende Primitive, eindeutige Exporte/Bindings und korrekte selektorspezifische Ergebnisse ohne dupliziertes Tool/ToolSkill; keine extra Kombinationsapproval. Composition. |
| Gemeinsame Nutzung existiert bereits im aktuellen Resolver | Vorhandene unterstützte Identitäten verwenden, keinen neuen Selector/Adapter/Registry implementieren. Composition. |
| Mehrdeutiges identisches Tupel oder vertauschte Bindung | Keine First-Match-/Namens-/Prosaauflösung; falscher Tool/Callable/Export/Adapter scheitert vor Effekten. Nur konkret nötige owning Resolvererweiterung qualifizieren. |
| Preload, Isolierung und Policy bei gemeinsamer Nutzung | Laden führt kein Tool aus; Input-/Default-/Resultzustand bleibt getrennt. Live Toolblock wirkt auf jede Nutzung auch nach Bindung/Wait. Kernel/Runner. |
| Öffentlicher Command versus privater Validatorpfad | Jeweils reale W-Admission-/Ergebnis-/Completionbedingungen; Skill-Export allein erzeugt keine MCPanzeige oder öffentlichen Match. Composition/MCP/Validator. |
| Neue shipped W-/T-Paketbytes | Nichtzirkuläre Installation-/Integritäts-/Behaviorqualifikation ohne zusätzliches Instanz-Q2; alte originale Bytes/Overrides/Taskpins bleiben. Bestehender Installationowner. |
| A-Recipe mit bereits freigegebenen Skill-/Codefunktionen | Alle angebotenen unterstützten Kombinationen/Varianten tatsächlich ausführen/prüfen, ohne neue Q1-/Q2-Aufträge der verwendeten Components oder generische Associationapprovalrunde. |
| S-Recipe mit qualifizierten Nutzungen | Vorhandene genaue Skillnutzungsbelege sowie vorausgehende eigene Tool-/ToolSkillfreigaben verifizieren; Workflowbeleg ersetzt diese nicht und entsteht außerhalb normaler authored Q1-Queue. |
| Gemeinsamer ToolSkill, mehrere Skill-/Codefunktionen | A: jede neue eigene Funktion einmal gemeinsam mit Code prüfen. S: jeweilige genaue Nutzung qualifizieren. Gültiger gemeinsamer ToolSkill/Tool wird in keinem Fall durch jede Nutzung erneut einzeln genehmigt. |
| Parametrisierter Ablauf mit aufgerufenem Recipe | Konkrete Klasse21-Identitäten und typed Übergaben, eindeutige Ergebnisse/Fehler-/Childcustody. Normale W-Abdeckung, kein class50-Datensatz und kein Scaffold-Sonderreview. |
| Vorhandener Scaffold mit Recipezuordnung | Keine Umklassifizierung alter Bytes. Neue Consumer wählen das tatsächlich validierte Recipe; Navigation ist keine ausführbare neue Klasse50-Aliasroute. Referenz-/Generations-/Unknown-Commit-Kontrollen des vorhandenen Owners bleiben. |
| Leerer oder ungeklärter Scaffoldbestand | Leer: keine Dummymigration/-rezepte erzeugen. Ungeklärt: konkrete Semantik/aktive Referenzen als offene Migrationslücke behandeln, kein stilles Weglassen/No-Match. Klasse10-Schutz bleibt unverändert. |

- [ ] Die beiden exportierten Themen einschließlich des zusätzlich autorisierten
  Recipe-/Scaffoldabbaus sind hier beauftragt; A/S-Scope,
  gemeinsamer Skill-/Codefunktionsgegenstand und Abhängigkeitsreihenfolge werden
  aus dem bisherigen Plan konsumiert, keine zweite Freigabe-/Kombinationsengine.
- [ ] A-Q1 enthält nur eigene Funktionstests sowie bei Recipes sämtliche
  angebotenen unterstützten Kombinationen. S-Kombinationsqualifikation bleibt
  außerhalb der normalen Queue und genehmigt nicht alle vorausgesetzten Components.
- [ ] Alle aktuellen lokalen W-/T-Zielpassagen und gemischten Dateideltas sind
  umgesetzt oder nachvollziehbar No-Edit; notwendige offene Edits sind kein Abschluss.
- [ ] W-/T-Folgearbeit steht im bestehenden Ownerplan hinter dem laufenden Paket
  und vor dem noch offenen konsumierenden Gate, mit tatsächlichen Abnahmefällen.
- [ ] Bestehende Primitive/Bindings/Source-/Review-/Runtimeowner werden verwendet;
  keine erfundene Kapazität, zweite Registry, VM oder Test-/Approvalengine.
- [ ] Links, Spiegel, Exportzuordnung, scoped Diff und Whitespace geprüft.
  Dokumentlieferung, technische Implementierung und Aktivierung getrennt berichten.
- [ ] Scaffoldarbeit ist genau einmal bei W-R verankert: keine neue Klasse50-
  Erstellung/Aktivierung/Sonderprüfung; bestehende echte Referenzen und historische
  Auswahlbytes bleiben erhalten. Normale Recipevalidierung statt Extra-Prüfpfad.

## 8. Exportzuordnung und gemeinsame Bearbeitungsgrenze

| Herkunft aus der bisherigen Fassung | Ausschließlicher neuer Zielauftrag |
| --- | --- |
| §2.1 Punkte 1–3/5/6: Toolumfang, Skillnutzung, ToolSkillbereiche, ls-Beispiel, Binding-/Step-/Preloadgrenzen | T1–T7 und T-A. Die Freigabescopeaussage aus Punkt 4 bleibt im bisherigen Plan. |
| §2.2 Punkt 3 und entsprechende V1-/Prüfungserklärungen | W1–W5 und W-A/W-B. Freigaben, historische Belegprovenienz und Gültigkeit unveränderter Consumers bleiben im bisherigen Plan. |
| U1-Recipeprüfung, U4-gemeinsame Nutzung/Resolvereindeutigkeit und U5-Recipe-/ls-Familienfälle | W-A/W-B/T-A. U4-Schemavarianz/Approvalvererbung/Deployment bleibt im bisherigen Plan. |
| Gemischte Guide-/Plan-/Teaching-/MCP-/UI-Dateizeilen | §6, mit stabiler Basis-ID und ausschließlich W-/T-lokalem Edit. Im bisherigen Inventar wurden diese Anteile entfernt bzw. als ausgelagert abgegrenzt. |
| §4.1 Fälle „Drei ls-Skills…“ und „Neues oder inhaltlich geändertes Recipe“ samt Guard gegen Fakeadapter/Fallquoten | §7 und T4/W3. Keine erneute Consumerfreigabe durch die Aufteilung. |
| Abschluss-/Auditanteile zu W-Prüfung und T-Kardinalitäten/ls-Beispiel | Vorliegende Abschlusskriterien und technische Eigentümerabgrenzung; alte Auditbelege bleiben als datierte Herkunft lesbar. |

Eine Passage mit beiden Themenpaketen wird vor dem Edit nach Verantwortung
zerlegt. Der bestehende technische Owner erhält einen integrierten Delta statt
zwei Implementierungen. Falls eine tatsächlich gemeinsame Schnittstellenänderung
beide Pakete verbindet, deren Beitrag/Consumerabnahme konkret verlinken, nicht
beide Dokumentpläne insgesamt zur Vorbedingung unabhängiger Arbeit machen.

**Liefergrenze am 31. Oktober 2026:** Die Schritte wurden zwischen diesen beiden
Plandateien aufgeteilt. Keine der referenzierten Architektur-/Dokumentänderungen,
Komponentenprüfungen oder technischen Implementierungen wurde damit durchgeführt.

## 9. A/S-Präzisierung und Optimierung der Workflow-Q1

Die Quelle für normalen A-Funktions-Q1, den gemeinsam geprüften Skill-/Codegegenstand
und die erhaltene S-Qualifikationsreihenfolge ist der bisherige Exactplan §§1.1,
2.2, 2.8 und 2.9. Die neue A/S-Regel präzisiert den älteren Export: Es wird
ausdrücklich keine Abschaffung bestehender system_seed-Kombinationsqualifikation
beauftragt. Alle Dateideltas aus §6 müssen ihre aktuelle lokale W-/T-Anweisung
entsprechend präzisieren, nicht nur einen globalen Hinweis ergänzen.

| Bestehende offene Arbeit / Basisdateien | Einzutragende konkrete Optimierung |
| --- | --- |
| W-A, Validator Step7 Klasse21/Step8 und 53 recipe.md | A-Q1 untersucht die ganze unterstützte Recipepfad-/Varianten-/Dependencykombinationsmenge des Kandidaten. Gemeinsame Strukturregeln einmal sammeln, Variantencoverage nicht überspringen. Echte Integration beobachten, aber keine erneuten Komponentenreviews oder separate allgemeine Kombinationsprüfung starten. |
| 61 Validator Steps2/3/5–9 und 46/64/66 Submissions/Inputs/Coverage | Nur notwendige Workflow-/Pfadchecks zusätzlich zu vorhandenen gültigen Funktions-/Bindingbelegen. Vorab neuer Skill samt Code wird einmal als Funktion geprüft, nicht im Recipe nochmals in zwei Komponentenreviews zerlegt. Unvollständige Preconditions lassen Kandidat ausstehend; Recipeerfolg ersetzt keine Freigabe. Paarsubject/-writerarbeit bleibt im bisherigen Plan, kein zweiter Adapter. |
| 35–39 Compositionanhänge und W-B/T-A | Dieselben vorab gültigen Funktionen mit gemeinsamem ToolSkill an unterschiedlichen tatsächlichen Vorkommen binden. Fehlendes/widersprüchliches Binding scheitert technisch; Genehmigung einer Funktion ist keine Genehmigung irgendeiner anderen. Kein Tool-/ToolSkilldup zur Umgehung der Auswahl. |
| 41 Sempai Step10, 43 Connections PhaseF und 51 V35-16/17–20 | S-Bibliotheks-/Compilerqualifikation mit genauer Nutzung bleibt, A-Vorschläge konsumieren normalen Funktions-/Recipe-Q1. Qualifizierter Seed-Skill enthält im Beleg die freigegebenen Voraussetzungen, erteilt deren Einzelgenehmigungen aber nicht. Keine Seedfunktion in die normale authored Queue schieben. |
| 04/05/32/54/56/75–83/90 aktuelle Regeln und Lehrquellen | Unmittelbar A-Funktion, A-Recipeworkflow und S-Bootstrap unterscheiden. Skill/PythonCode trotz zweier IDs gemeinsam prüfen; Kombinationen innerhalb des angebotenen Recipes sind W-Abdeckung, nicht zusätzliche Abhängigkeitsfreigaben. Aktive pauschale Gegenanweisungen lokal berichtigen; historische Originale erhalten. |
| WT-D und offene Validator-Step11-/K5/K6-Nachfolger | Bestehenden Seedwriter weiterverwenden und nur fehlende Voraussetzungs-/Scopebelege ergänzen. Tool zuerst qualifiziert, ToolSkill danach, Skill+Code gemeinsam, anschließend tatsächlicher Consumerworkflow; nichtzirkulär und ohne zusätzliches installiertes/externes Q2. Neue authored Recordformate nicht auf S übertragen. |

Die einmalige Komponentenfreigabe verhindert keine notwendige erneute Beobachtung
derselben Toolfunktion als Teil eines neuen Workflowtests. Wiederholte echte
Integration ist von doppelten eigenen Review-/Freigabeaufträgen zu unterscheiden.
Gemeinsame vollständige Fälle können mehrere Regelanforderungen erfüllen;
unterschiedliche Varianten/Originalinputs/Claim-/Effektbezüge bleiben getrennt.
Weder diese Optimierung noch die gemeinsame Bindingnutzung aktiviert fehlende
Runtime-/Branch-/Retry-/Freeunterstützung. Alle tatsächlichen Änderungen bei
Durchführung bleiben Dokument-/Planeingriffe hinter laufenden Implementierungen.

## 10. Konkrete Dateideltas für Recipe statt Scaffold

Diese Ergänzung präzisiert den eigenen Workflowumfang. Sie baut keinen dritten
Exactplan oder separaten Scaffoldkonverter. Bestehende aktive und historische
Klasse50-Referenzen werden vor Änderungen im bisherigen Migrationsowner ermittelt.
Nur tatsächliche eigene Abläufe in Klasse21 überführen; reine Navigation auf das
konkrete Recipe beschränken. Eine Verknüpfung allein genehmigt das Ziel nicht.

| Basis-ID / Dokumentgruppe | Einzutragender lokaler W-R-Schritt |
| --- | --- |
| 04/05/07/53/58 Root-/Komponentenguides | Wiederverwendung durch parametrisierte Recipe-/Childaufrufe beschreiben. Klasse50 wird nicht mehr erstellt oder separat geschützt; eigene Toolnutzungs-Skills und Klasse10 bleiben getrennt. |
| 34–39 Recipe-/Compositionpläne und Anhänge | Im nächsten offenen Familien-/Katalognachfolger tatsächliche Scaffoldsemantik auf vorhandene oder normale geprüfte Recipeidentität zuordnen, eigene Variablen-/Result-/Fehlerbedingungen unter W prüfen. Keine neuen Wrapperrecipes nur zur Erhaltung eines Labels. |
| 57/61 Simplified-/Validatorplan und 51 v3.5-Roadmap | Offene separate Scaffold-Root-/Validierungsaufgaben durch W-R-Verweise ersetzen; bestehender Klassen21-Validator reicht. Fehlende echte Child-/Aufrufmechanik zum vorhandenen Owner, kein Neubau der Runtime. Alte laufende/abgeschlossene Pakete nicht ändern. |
| 45/46/64/65/70 Deployment-/Submission-/Workflow-/Factoryverträge | Neue Consumer verwenden explizite tatsächliche Recipe-/Variantenauswahl. Historische class50-Subjects/-Selections dürfen unverändert gelesen, aber nicht pauschal als genehmigte Klasse21 interpretiert werden. Normalen Approval-/Generation-/Claim-/Effectvertrag verwenden. |
| 30/31/75/76/77/79/89/90/91/92/93/95 UI-/Architektur-/Klassentexte | Neue aktive Scaffoldkategorie/-Editoren/-Freigaben und geplante separate Tabellen aus dem Ziel entfernen; vorhandene geeignete Links auf Recipeverwaltung richten. Klasse50 nur historisch dokumentieren, Klasse10-Schutz nicht gemeinsam streichen. |
| 40/54/56 Lehr-/Prefixtexte und WT-D-Upgradeowner | Neue Source-/Paketrevisionen enthalten normale Recipebeispiele. Vorhandene S-Qualifikation für tatsächlich neue Ersatzrecipes verwenden; alte Seed-/Task-/Auditbytes nicht überschreiben und keine zusätzliche Scaffoldqualifikation einführen. |

Neue eigene Recipe-/Consumerrevisionen durchlaufen die üblichen A-/S-Verfahren.
Unveränderte Dependencies werden nicht neu genehmigt. Solange tatsächlich gültige
alte Tasks ihre ursprüngliche Auswahl brauchen, bleibt deren vorhandener Lesepfad
erhalten oder sie werden über die bestehende kontrollierte Drain-/Recoveryregel
abgeschlossen. Daraus folgt weder ein permanenter Schutzstatus für Klasse50 noch
ein neuer Scaffoldvalidator. Erst nach echter Referenzklärung darf der vorhandene
GC-/Upgradeowner alte entbehrliche Artefakte freigeben; dieser Dokumentplan löscht nichts.

## 11. Verbindlicher Sourceabgleich und Auditkorrekturen — 31. Oktober

Der bisherige Plan §§2.2.1/2.7/12 besitzt gemeinsame Subjects, revisionsbezogene
A/S-Herkunft und aktuelle wiederzuverwendende Infrastruktur. W/T übernehmen diese
Verträge; sie führen weder eine zweite Freigabegruppe noch einen eigenen A/S-Modus
ein. Gemischte A-Recipes mit gültigen S-Abhängigkeiten sind zulässig. Der Recipe-
Prüfweg ändert die Herkunft seiner verwendeten Revisionen nicht.

| Auftrag | Präzisierung im nächsten offenen Abschnitt des bestehenden Owners |
| --- | --- |
| W-A/W-B: Abdeckung | Unterstützte Kombinationen sind die tatsächlichen angebotenen Varianten und Vertragsprofile, keine kartesische Matrix aller historischen Komponentenversionen. Endliche deklarierte Varianten vollständig zuordnen; offene Wertebereiche mit begründeten Pfad-/Grenz-/Relationstests und vollständiger Regelabdeckung behandeln. Neue dynamische Profile bleiben bei ihren Runtimeownern. Ein kompatibles Dependencyupdate allein erzeugt kein neues Recipe-Q1. |
| W-A: bestehende neue Komponenten | Der eigene Funktionsreview des Skill-/Codegegenstands ist Voraussetzung, dessen echte Ausführung innerhalb W bleibt Workflowbeobachtung. Bei koordinierten Drafts kann begrenzte private Vorprüfung stattfinden; Freigabe/Publikation des Recipes verlangt alle echten eigenen Voraussetzungsfreigaben. Kein zirkuläres Vorab-Recipe oder selbst erteiltes Dependencyapproval. |
| T-A: tatsächliche Eindeutigkeit | Der [aktuelle Resolver](../../crates/brassclaw_engine/src/memory/retained_tools.rs) löst nach Code-UUID/ToolSkill-UUID/Tool-UUID/Callable auf. Schon mögliche Wiederverwendung zuerst nutzen. Falls ein konkretes unterstütztes gemeinsames Codeprofil damit mehrdeutig ist, nur diesen bestehenden Selektionsvertrag einschließlich Persistenz/Readern ergänzen; einen neuen Exportnamen allein als funktionierenden Selector auszugeben wäre falsch. |
| W-R/WT-C: native Childgrenze | [global_recipe_ports](../../crates/brassclaw_reborn_composition/src/global_recipe_ports.rs) und [monty_child_tasks](../../crates/brassclaw_reborn_composition/src/monty_child_tasks.rs) enthalten echte selektierte Child-/Claimmechanik; das beweist keinen beliebigen verschachtelten Recipe-UUID-Schritt. W7 konkret prüfen und vorhandene Unterstützung verwenden. Kein zusätzlicher Aufrufrunner. |
| WT-D/G: Klasse50 | Neue Authoring-/Import-/Approval-/Publikationswege schließen, historische Decoder/Belege und gültige alte Taskpins erhalten. U11 liefert Trustregeln, W-R bleibt alleiniger Migrationsowner. Ein fehlender tatsächlicher Bestand begründet keine Ersatzrezepte; passive Links gewähren keinen ausführbaren Alias. |
| WT-C–F: laufende Integration | Die Passed-Teilbausteine BUILD-004/007/008/009/011/012/013 aus [der zentralen Queue](../development-build-queue.md) exakt wiederverwenden. Free-Sealing/Serving aus BUILD-002 und V2-Bridgeabbau aus BUILD-010 bleiben dort. Keine neue JSONimplementierung, parallele Capture-/Custody-/Budgetprüfung oder allgemeine SDK-/Proofstruktur durch W/T. Die jeweiligen Restgates bleiben offen. |

In die bestehenden Abnahmematrizen gehören zusätzlich: gleicher ToolSkill mit
verschiedenen eindeutigen Code-UUIDs ohne Resolverneubau; gleiche Code-UUID und
verschiedene Exporte bei identischem alten Tupel ohne First-Match; soweit hierfür
ein neuer unterstützter Selector nötig ist, dessen exakte Prepare-/Task-/Child-/
Restoreauflösung. Außerdem fehlende Childselektion, fremder Parentclaim und
nach Parentstart geänderte Katalog-/Modelleinstellungen: keine neue Latestauflösung,
kein erfolgreicher leerer Childreturn, keine Effektreplays. Historische Klasse50-
Readbacks und neue Legacy-Imports getrennt prüfen. Alle Fälle werden im bestehenden
Owner ergänzt, nicht von diesem Dokumentplan ausgeführt.

## 12. Ausführungskoordination — privates Textpaket 1, 31. Oktober

Root hat private begrenzte Textvorbereitung freigegeben, keine direkten Änderungen
kanonischer Zieltexte außerhalb dieser beiden Pläne. Das gemeinsame Paket ist in
[§13 des bisherigen Plans](Exact_validation_definition_upgrade_at_K3.md#13-ausführungskoordination--privates-textpaket-1-31-oktober)
mit Originalpfad und Patchidentität verzeichnet und Root zur Prüfung eingereicht.
Es enthält W-/T-Kernverträge, class50-Recipezuordnung und konkrete offene Owner-
Nachfolger als reine Prosa; keine Runtime-/API-/JSON-/SQL-/Pythoncodeerzeugung.

Die Ownerplananteile sind append-only hinter laufender Arbeit einschließlich014;
Root integriert erst nach seiner terminalen Abnahme und Dependencyfreigabe.
Beforehash-/Patch-/Whitespace-/lokale Linkzielprüfungen bestanden für das private
Paket, keine Runtime- oder Komponentenfreigabe wurde daraus abgeleitet. Alle
81 Dateidispositionen bleiben bis zu tatsächlicher geprüfter Integration offen.
Weitere Textpakete folgen nach Rootrückmeldung; Prefix-exklusive und Root-only
Zielpfade bleiben in dessen alleiniger Integration.
