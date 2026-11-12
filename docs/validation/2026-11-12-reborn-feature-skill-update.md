# Report zur Aktualisierung des reborn-feature Skills

Datum: 12. November 2026. Ausgangsstand:
`3c04cb307ef72d009d819387464fe9f67836fd84` auf `main`.

Alle fünf Vorschläge aus der Skill-Prüfung sind in der
[kanonischen Fassung](../../.agents/skills/reborn-feature/SKILL.md) und ihrer
[Spiegelkopie](../../.claude/skills/reborn-feature/SKILL.md) umgesetzt.
Die Änderung betrifft Entwicklungsanweisungen. Sie aktiviert keine Komponenten
und schließt keine Bob-, Prefixless- oder v3.5-Laufzeitabnahme.

## Umgesetzte Änderungen

| Prüfungsbefund | Änderung | Maßgeblicher Vertrag |
| --- | --- | --- |
| Bob-Reparaturpläne fehlten als direkte Navigation | B1/B2/B3 mit ihren bestehenden Zuständigkeiten ergänzt; gemeinsame Abhängigkeitsreihenfolge verlinkt und `missing4_v3_at_k3.md` als überholte Navigation eingeordnet | [Bob 1](../../Repair_bob_1_v3_at_k3.md#1-scope-precedence-and-single-implementation-ownership), [Bob 2](../../Repair_bob_2_v3_at_k3.md#1-scope-and-ownership), [Bob 3](../../Repair_bob_3_v3_at_k3.md#1-scope-architecture-and-ownership) |
| Beobachtung, terminales Verdikt und Approval waren zu knapp abgegrenzt | Originalsubject und ausgewählten Graphen an dauerhafte private Arbeit binden; Wake als Benachrichtigung behandeln; Verdikt erst nach tatsächlichem nativem Settlement und Custody-ACK; negative, unvollständige, abgebrochene und ungeklärte Ergebnisse unterscheiden; Selbstwarten im Finish-Callback ausschließen | B1-04/B1-08 und B3-06/B3-07 |
| Neue Profile hatten keinen konkreten Reader-Handoff | Gemeinsamen versionierten Schema-/Checksum-Vertrag vor Writeränderungen prüfen; Serializer, verschachtelte Reader, DB-Prüfungen und alle betroffenen Ausführungs-/Restore-/Publikationsconsumer vor Freigabe verbinden; alte Bytes und Abwesenheitssemantik bewahren | [B2-02](../../Repair_bob_2_v3_at_k3.md#3-define-exact-approval-carriage-before-touching-readers) |
| Neuer Prefixless-Konstruktionsvertrag und präzisierte Guard-Zuständigkeit fehlten | Gemeinsamen Vertrag und Simplified-/v3.5-Handoffs verlinkt; sichtbaren Vorschlag von eingereichter Auswahl getrennt; ursprünglichen Kontext vor Effekten binden; Prefix-Qualifikation von Composition-/Simplified-Laufzeitmechanik unterscheiden; fehlende native Unterstützung als offenes Gate erhalten | [Gemeinsamer Vertrag](../reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection), [Simplified](../../simplified_v3.md#open-prefixless-construction-successor--existing-owner-contribution), [v3.5](../../path_to_v3.5.md#open-construction-choice-handoff) |
| Semantische Handlerprüfung und konfigurierbare Betriebsgrenzen waren nicht ausdrücklich genug | V35-08 auch für geänderte Workflow-Handler eingebunden; vorhandenen Review-Record nutzen; notwendige Betriebsgrenzen über bestehende DB/API/WebUI-/Live-ACK-Owner führen und von Protokollgrenzen unterscheiden | [V35-08](../../path_to_v3.5.md#guidance-reconciliation-and-semantic-review), [Bob-Qualitätsregel](../../Repair_bob_1_v3_at_k3.md#mandatory-implementation-and-evidence-quality) |

## Erhaltene Architektur und Quellabgleich

[recipe.md](../../recipe.md), [skills.md](../../skills.md),
[tools.md](../../tools.md) und [toolskills.md](../../toolskills.md) bleiben die
verbindliche Grundlage. Der Skill hält weiterhin Recipe-first, Wiederverwendung,
eine Tool-Nutzung pro Skill, explizit zugeordneten preloadbaren PythonCode,
nicht ausführende ToolSkill-Bindung, typisierte Daten, ursprüngliche Pins und
aktuelle Kernel-Policy fest. Authored Own-Approval und exakte Shipped-Qualifikation
bleiben getrennt; nur echtes No-Match darf Tier 2 erreichen.

Der geprüfte [Retained-Runner](../../crates/brassclaw_engine/src/executor/retained_recipe.rs)
verlangt weiterhin `stop` mit `max_attempts: 1` und weist ungebundene oder
wiederholte Tool-Aufrufe zurück. Diese Supportgrenze bleibt im Skill erhalten.
Der vorhandene [Auswahl-Serializer](../../crates/brassclaw_engine/src/memory/retained_instruction.rs)
schreibt `/1`; vorgeschlagene Nachfolgeformate werden deshalb nicht als bereits
verfügbar festgeschrieben. Der
[Verdikt-Reader](../../crates/brassclaw_reborn_composition/src/pg_validator_q1_verdict.rs)
und der bestehende GlobalMontyDriver bleiben die Ausgangspunkte für die
Beobachtungs-/Settlement-Grenze. Die neue Navigation verlangt Wiederverwendung
dieser Owner und belegt keine allgemeine Approval-Vollständigkeit.

## Prüfung und Grenzen

- Vollständigen Skill-Diff auf Vertragskonsistenz, Zuständigkeiten und erhaltene
  Supportgrenzen geprüft.
- `quick_validate.py` des verwendeten `skill-creator` für beide installierten
  Skill-Verzeichnisse erfolgreich ausgeführt.
- Beide Skill-Dateien mit `cmp` als bytegleich bestätigt.
- Je 28 lokale Markdownlinks einschließlich ihrer Überschriftenanker aufgelöst.
- `git diff --check` nach Skill-Installation erfolgreich; die Abschlussprüfung
  umfasst zusätzlich diesen Report und den Changelog-Eintrag.

Name, Beschreibung und Auswahlbereich des Skills bleiben unverändert. Es gibt
keine Änderungen an Rust, Python-Laufzeitcode, Seeds, Migrationen, Konfiguration
oder den vier Architekturleitfäden. Für diese Entwicklungsdokumentation wurden
keine Cargo-, Datenbank-, Modell- oder Browserläufe gestartet. Die Prüfung ist
eine Dokumentations- und Quellprüfung, keine Laufzeit-, Deployment- oder
vollständige Plattformabnahme. Die vorbestehende `.DS_Store`-Änderung gehört
nicht zum Commit.
