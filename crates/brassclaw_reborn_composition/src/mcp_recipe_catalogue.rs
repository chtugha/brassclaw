//! Read-only MCP discovery owned by the normal-chat catalogue. Declarations
//! live in checksummed retained Recipe documents, not legacy INSERT fields.
//! Parsing a declaration is not qualification. Only the installation catalogue
//! owner currently publishes generations; authored activation has no adapter yet.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
};

use brassclaw_engine::executor::{
    retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram,
};
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    value_contract::{InputContract, strict_json},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MAX_COMMAND_BYTES: usize = 64 * 1024;
const MAX_ENTRIES: usize = 1024;

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum McpDiscoveryError {
    #[error("MCP Recipe catalogue is unavailable")]
    Unavailable,
    #[error("MCP Recipe command contract is invalid")]
    Contract,
    #[error("MCP Recipe catalogue contains conflicting identities or commands")]
    Conflict,
    #[error("MCP Recipe requires the qualified preloadable Skill runner and normal-match evidence")]
    UnsupportedRunner,
    #[error("MCP command qualification is missing, stale or unsuccessful")]
    Unqualified,
    #[error("MCP command was not advertised or its contract has changed")]
    StaleCommand,
}

/// This is an opt-in *declaration*, never an approval or permission. Its exact
/// bytes are part of the Recipe revision subject reviewed by the catalogue owner.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandDeclaration {
    format: String,
    pub(crate) name: String,
    purpose: String,
    pub(crate) skill_uuid: Uuid,
    variant_key: String,
    pub(crate) export_name: String,
    pub(crate) sentence: String,
    variables: Vec<CommandVariable>,
    formatting_rules: String,
    pub(crate) examples: Vec<String>,
    pub(crate) negative_examples: Vec<String>,
    result_description: String,
    error_description: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CommandVariable {
    name: String,
    /// Zero-based percent-slot index, matching the Recipe's variable_patterns.
    position: usize,
    /// The current capture adapter transports strings verbatim. JSON decoding
    /// needs a separately reviewed Recipe step; it is not done by MCP ingress.
    encoding: String,
}

#[derive(Clone)]
pub(crate) struct CommandContract {
    pub(crate) declaration: CommandDeclaration,
    task_inputs: Value,
    inputs: InputContract,
    literals: Vec<String>,
    pub(crate) checksum: [u8; 32],
}

fn identifier(name: &str) -> bool {
    let mut chars = name.bytes();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

impl CommandContract {
    pub(crate) fn parse(raw: &Value, task_inputs: &Value) -> Result<Self, McpDiscoveryError> {
        let invalid = || McpDiscoveryError::Contract;
        // strict_json also bounds nested values. The enclosing retained revision
        // parser has already rejected duplicate keys in the original bytes.
        let bytes = raw.to_string();
        if bytes.len() + task_inputs.to_string().len() > MAX_COMMAND_BYTES {
            return Err(invalid());
        }
        let value = strict_json(&bytes, REVISION_LIMITS).map_err(|_| invalid())?;
        let declaration: CommandDeclaration =
            serde_json::from_value(value).map_err(|_| invalid())?;
        if declaration.format != "mcp-call-skill-recipe/1"
            || declaration.skill_uuid.is_nil()
            || !identifier(&declaration.name)
            || declaration.name.len() > 64
            || !identifier(&declaration.export_name)
            || matches!(
                declaration.export_name.as_str(),
                "and"
                    | "as"
                    | "assert"
                    | "async"
                    | "await"
                    | "break"
                    | "class"
                    | "continue"
                    | "def"
                    | "del"
                    | "elif"
                    | "else"
                    | "except"
                    | "finally"
                    | "for"
                    | "from"
                    | "global"
                    | "if"
                    | "import"
                    | "in"
                    | "is"
                    | "lambda"
                    | "nonlocal"
                    | "not"
                    | "or"
                    | "pass"
                    | "raise"
                    | "return"
                    | "try"
                    | "while"
                    | "with"
                    | "yield"
                    | "host"
                    | "inputs"
                    | "result"
            )
            || declaration.variant_key.trim().is_empty()
            || declaration.sentence.is_empty()
            || declaration.sentence.len() > MAX_COMMAND_BYTES
            || declaration.variables.len() > 64
            || [
                &declaration.purpose,
                &declaration.formatting_rules,
                &declaration.result_description,
                &declaration.error_description,
            ]
            .into_iter()
            .any(|text| text.trim().is_empty())
            || declaration.examples.is_empty()
            || declaration.examples.len() > 64
            || declaration.negative_examples.is_empty()
            || declaration.negative_examples.len() > 64
        {
            return Err(invalid());
        }
        let inputs = InputContract::from_json(&task_inputs.to_string(), REVISION_LIMITS)
            .map_err(|_| invalid())?;
        let literals: Vec<String> = declaration.sentence.split('%').map(str::to_owned).collect();
        let count = declaration.variables.len();
        if literals.len() != count + 1
            || (count > 0 && literals[0].is_empty() && literals[count].is_empty())
            || (count > 1 && literals[1..count].iter().any(String::is_empty))
        {
            return Err(invalid());
        }
        let mut names = BTreeSet::new();
        let schemas = task_inputs.as_object().ok_or_else(invalid)?;
        for (position, variable) in declaration.variables.iter().enumerate() {
            let schema = schemas.get(&variable.name).ok_or_else(invalid)?;
            if variable.position != position
                || variable.encoding != "verbatim"
                || !identifier(&variable.name)
                || !names.insert(variable.name.as_str())
                || schema.get("type").and_then(Value::as_str) != Some("string")
                || schema.get("required") != Some(&Value::Bool(true))
                || schema.get("nullable") == Some(&Value::Bool(true))
                || schema.get("default").is_some()
            {
                return Err(invalid());
            }
        }
        // There is no hidden source of MCP task inputs. Optional inputs may use
        // existing consumer defaults, but required inputs must all be captured.
        if inputs.required_names().any(|name| !names.contains(name)) {
            return Err(invalid());
        }
        let checksum = Sha256::digest(
            json!({"command":raw,"task_inputs":task_inputs})
                .to_string()
                .as_bytes(),
        )
        .into();
        let contract = Self {
            declaration,
            task_inputs: task_inputs.clone(),
            inputs,
            literals,
            checksum,
        };
        let mut examples = BTreeSet::new();
        for example in &contract.declaration.examples {
            if !examples.insert(example) || contract.bind(example).is_err() {
                return Err(invalid());
            }
        }
        for example in &contract.declaration.negative_examples {
            if !examples.insert(example) || contract.bind(example).is_ok() {
                return Err(invalid());
            }
        }
        Ok(contract)
    }

    /// Format acceptance only. Ordinary chat still owns intent matching, typed
    /// IBS input binding and all execution. Never interpret values as Python.
    pub(crate) fn bind(&self, command: &str) -> Result<Value, McpDiscoveryError> {
        let invalid = || McpDiscoveryError::Contract;
        if command.is_empty() || command.len() > MAX_COMMAND_BYTES {
            return Err(invalid());
        }
        let count = self.declaration.variables.len();
        if count == 0 {
            if command != self.literals[0] {
                return Err(invalid());
            }
            return self
                .inputs
                .bind(&json!({}))
                .map(Value::Object)
                .map_err(|_| invalid());
        }
        let mut remainder = command
            .strip_prefix(&self.literals[0])
            .and_then(|text| text.strip_suffix(&self.literals[count]))
            .ok_or_else(invalid)?;
        let mut values = serde_json::Map::new();
        for (index, variable) in self.declaration.variables.iter().enumerate() {
            let raw = if index + 1 == count {
                remainder
            } else {
                let separator = &self.literals[index + 1];
                let offset = remainder.find(separator).ok_or_else(invalid)?;
                let raw = &remainder[..offset];
                remainder = &remainder[offset + separator.len()..];
                raw
            };
            // Verbatim transport has no escaping. Reject separator text inside
            // values rather than pretend quotes/backslashes change extraction.
            if raw.is_empty() || self.literals[1..count].iter().any(|s| raw.contains(s)) {
                return Err(invalid());
            }
            values.insert(variable.name.clone(), Value::String(raw.to_owned()));
        }
        self.inputs
            .bind(&Value::Object(values))
            .map(Value::Object)
            .map_err(|_| invalid())
    }

    fn tool(&self) -> Value {
        let declaration = &self.declaration;
        let description = format!(
            "Purpose: {}\nCommand sentence: {}\nVariables (zero-based positions): {}\nVariable contracts: {}\nFormatting: {}\nTransport: insert each value verbatim; nonempty values only; delimiter text inside values is forbidden. No Python source is accepted as an execution payload.\nExamples: {}\nResult: {}\nErrors: {}",
            declaration.purpose,
            declaration.sentence,
            serde_json::to_string(&declaration.variables).expect("serializable command variables"),
            self.task_inputs,
            declaration.formatting_rules,
            json!(declaration.examples),
            declaration.result_description,
            declaration.error_description
        );
        json!({"name":declaration.name,"description":description,
            "inputSchema":{"type":"object","properties":{"command":{"type":"string","minLength":1,"maxLength":MAX_COMMAND_BYTES}},"required":["command"],"additionalProperties":false}})
    }
}

/// Immutable advertised generation. It contains no component bodies, execution
/// handles, DB access, model requests or Tool dispatchers. Capturing this value
/// pins advertising only, not the later ordinary chat task's selection.
pub struct McpRecipeDiscoverySnapshot {
    generation: Uuid,
    entries: BTreeMap<String, CommandContract>,
    qualification_checksum: Option<[u8; 32]>,
}
impl McpRecipeDiscoverySnapshot {
    pub(crate) fn validate_command(
        &self,
        name: &str,
        command: &str,
    ) -> Result<[u8; 32], McpDiscoveryError> {
        let contract = self
            .entries
            .get(name)
            .ok_or(McpDiscoveryError::StaleCommand)?;
        contract.bind(command)?;
        Ok(contract.checksum)
    }
    pub(crate) fn same_contracts(&self, other: &Self) -> bool {
        self.entries.len() == other.entries.len()
            && self.entries.iter().all(|(name, contract)| {
                other
                    .entries
                    .get(name)
                    .is_some_and(|other| other.checksum == contract.checksum)
            })
    }
    pub fn generation(&self) -> Uuid {
        self.generation
    }
    pub fn qualification_checksum(&self) -> Option<[u8; 32]> {
        self.qualification_checksum
    }
    pub fn tools_list(&self) -> Value {
        json!({"tools":self.entries.values().map(CommandContract::tool).collect::<Vec<_>>()})
    }
}

/// Facade-shaped read-only view. Production construction/publication stays
/// crate-private with the qualification owner, never an HTTP/client input.
pub struct McpRecipeDiscovery {
    current: RwLock<Option<Arc<McpRecipeDiscoverySnapshot>>>,
}
impl McpRecipeDiscovery {
    pub(crate) fn new() -> Self {
        Self {
            current: RwLock::new(None),
        }
    }
    pub fn snapshot(&self) -> Result<Arc<McpRecipeDiscoverySnapshot>, McpDiscoveryError> {
        self.current
            .read()
            .map_err(|_| McpDiscoveryError::Unavailable)?
            .clone()
            .ok_or(McpDiscoveryError::Unavailable)
    }

    /// Recheck the advertised contract immediately before ordinary chat send.
    /// A changed/withdrawn entry is stale, even while an old snapshot is retained.
    /// An unrelated addition permits the unchanged command to proceed.
    pub fn validate_advertised_command(
        &self,
        advertised: &McpRecipeDiscoverySnapshot,
        name: &str,
        command: &str,
    ) -> Result<(), McpDiscoveryError> {
        let current = self.snapshot()?;
        let before = advertised
            .entries
            .get(name)
            .ok_or(McpDiscoveryError::StaleCommand)?;
        let after = current
            .entries
            .get(name)
            .ok_or(McpDiscoveryError::StaleCommand)?;
        if before.checksum != after.checksum {
            return Err(McpDiscoveryError::StaleCommand);
        }
        before.bind(command)?;
        Ok(())
    }

    /// Called only after the installation owner verified exact packaged bytes,
    /// retained native artifacts and retained its catalogue generation. Authored
    /// rows/drafts cannot call into this boundary via a public constructor.
    #[cfg(test)]
    pub(crate) fn publish_installed(
        &self,
        expected: Option<Uuid>,
        generation: Uuid,
        programs: &[&InspectedRetainedProgram],
    ) -> Result<(), McpDiscoveryError> {
        self.publish(expected, generation, programs, None)
    }

    /// A coherent installed catalogue can be ready while no public command has
    /// completed ordinary-chat qualification. Expose an empty list explicitly;
    /// validate declarations, but never treat activation as advertising evidence.
    pub(crate) fn publish_unadvertised_installed(
        &self,
        expected: Option<Uuid>,
        generation: Uuid,
        programs: &[&InspectedRetainedProgram],
    ) -> Result<(), McpDiscoveryError> {
        Self::contracts(generation, programs)?;
        self.publish(expected, generation, &[], None)
    }

    pub(crate) fn publish_qualified_installed(
        &self,
        expected: Option<Uuid>,
        generation: Uuid,
        programs: &[&InspectedRetainedProgram],
        qualified: &crate::mcp_command_qualification::QualifiedCommands,
    ) -> Result<(), McpDiscoveryError> {
        self.publish(expected, generation, programs, Some(qualified))
    }

    fn publish(
        &self,
        expected: Option<Uuid>,
        generation: Uuid,
        programs: &[&InspectedRetainedProgram],
        qualified: Option<&crate::mcp_command_qualification::QualifiedCommands>,
    ) -> Result<(), McpDiscoveryError> {
        let mut current = self
            .current
            .write()
            .map_err(|_| McpDiscoveryError::Unavailable)?;
        if current.as_ref().map(|snapshot| snapshot.generation) != expected {
            return Err(McpDiscoveryError::Conflict);
        }
        let result = Self::contracts(generation, programs).and_then(|mut entries| {
            if let Some(proof) = qualified {
                proof.verify(generation, &entries)?;
                for entry in entries.values_mut() {
                    entry.checksum = Sha256::digest(
                        [entry.checksum.as_slice(), proof.checksum().as_slice()].concat(),
                    )
                    .into();
                }
            } else if !entries.is_empty() {
                return Err(McpDiscoveryError::UnsupportedRunner);
            }
            Ok(McpRecipeDiscoverySnapshot {
                generation,
                entries,
                qualification_checksum: qualified.map(|proof| proof.checksum()),
            })
        });
        match result {
            Ok(snapshot) => {
                *current = Some(Arc::new(snapshot));
                Ok(())
            }
            // Do not keep advertising a known invalid generation after refresh.
            Err(error) => {
                *current = None;
                Err(error)
            }
        }
    }

    pub(crate) fn contracts(
        generation: Uuid,
        programs: &[&InspectedRetainedProgram],
    ) -> Result<BTreeMap<String, CommandContract>, McpDiscoveryError> {
        if generation.is_nil() || programs.len() > MAX_ENTRIES {
            return Err(McpDiscoveryError::Contract);
        }
        let mut declared = BTreeMap::<String, CommandContract>::new();
        for selected in programs {
            let inputs = selected.program().inputs();
            let instruction = inputs.instruction();
            let recipe = &instruction.snapshot().revisions()[&instruction.recipe().uuid];
            let Some(raw) = recipe.draft().document().get("mcp_call") else {
                continue; // absence is explicitly private, never name inference
            };
            let task = &recipe.draft().document()["input_layouts"]
                [&instruction.variant().variant_key]["task_inputs"];
            let mut contract = CommandContract::parse(raw, task)?;
            if contract.declaration.variant_key != instruction.variant().variant_key
                || instruction
                    .variant()
                    .variable_patterns
                    .iter()
                    .any(|variable| variable.pattern.is_some())
                || !instruction
                    .variant()
                    .intent_examples
                    .contains(&contract.declaration.sentence)
                || instruction
                    .variant()
                    .variable_patterns
                    .iter()
                    .map(|v| &v.name)
                    .ne(contract.declaration.variables.iter().map(|v| &v.name))
            {
                return Err(McpDiscoveryError::Contract);
            }
            let RetainedProgram::Tools(program) = selected.program() else {
                return Err(McpDiscoveryError::Contract);
            };
            let usages: Vec<_> = program
                .bindings()
                .values()
                .filter(|b| b.skill().uuid == contract.declaration.skill_uuid)
                .collect();
            if usages.len() != 1 {
                return Err(McpDiscoveryError::Contract);
            }
            // Conservatively pin the complete exact execution selection too.
            // Equal sentence/schema text is not proof that changed code or a
            // changed binding implements the advertised usage compatibly.
            let selection = instruction
                .retained_selection()
                .map_err(|_| McpDiscoveryError::Contract)?;
            contract.checksum = Sha256::digest(
                [
                    contract.checksum.as_slice(),
                    selection.checksum().as_slice(),
                ]
                .concat(),
            )
            .into();
            for example in &contract.declaration.examples {
                let captured = contract.bind(example)?;
                let actual = inputs
                    .bind_variant_example(&contract.declaration.sentence, example, &json!({}))
                    .map_err(|_| McpDiscoveryError::Contract)?;
                if actual != captured {
                    return Err(McpDiscoveryError::Contract);
                }
            }
            // Duplicate names, duplicate templates and demonstrated overlaps
            // reject the entire generation; never pick the first row. Proving
            // ordinary-match uniqueness requires the catalogue's routing audit.
            if declared.contains_key(&contract.declaration.name)
                || declared.values().any(|previous| {
                    previous.declaration.sentence == contract.declaration.sentence
                        || previous
                            .declaration
                            .examples
                            .iter()
                            .any(|text| contract.bind(text).is_ok())
                        || contract
                            .declaration
                            .examples
                            .iter()
                            .any(|text| previous.bind(text).is_ok())
                })
            {
                return Err(McpDiscoveryError::Conflict);
            }
            declared.insert(contract.declaration.name.clone(), contract);
        }
        Ok(declared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn declaration() -> Value {
        json!({"format":"mcp-call-skill-recipe/1","name":"read_interval","purpose":"Read a specified line interval from a file.","skill_uuid":Uuid::from_u128(1),"variant_key":"selected","export_name":"read_interval",
            "sentence":"read file %; interval %","variables":[{"name":"path","position":0,"encoding":"verbatim"},{"name":"interval","position":1,"encoding":"verbatim"}],
            "formatting_rules":"Exact literals; values are nonempty strings. Do not use separator text inside values.","examples":["read file /tmp/a; interval 1:4"],"negative_examples":["write file /tmp/a; interval 1:4"],"result_description":"The selected interval.","error_description":"Invalid input stops before effects."})
    }
    fn task() -> Value {
        json!({"path":{"type":"string","required":true,"checks":[]},"interval":{"type":"string","required":true,"checks":[]}})
    }

    #[tokio::test]
    async fn qualified_mcp_declaration_without_execution_evidence_cannot_publish_export() {
        use brassclaw_engine::memory::{
            retained_instruction::{WorkflowClass, compile_retained_recipe},
            retained_tools::prepare_retained_tool_program,
        };
        use brassclaw_skills::{
            component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot},
            global_bootstrap_components::{GlobalBootstrapUsage, UsageComponentIds, usage_drafts},
        };
        let ids = UsageComponentIds {
            recipe: Uuid::from_u128(10),
            python_code: Uuid::from_u128(11),
            tool_skill: Uuid::from_u128(12),
            tool: Uuid::from_u128(13),
            skill: Uuid::from_u128(14),
            formatter: None,
        };
        let mut drafts = usage_drafts(ids, GlobalBootstrapUsage::PostReply).unwrap();
        let mut raw = declaration();
        raw["skill_uuid"] = json!(ids.skill);
        raw["name"] = json!("publish_reply");
        raw["purpose"] = json!("Publish the supplied literal reply to the owning chat.");
        raw["export_name"] = json!("publish_turn_reply");
        raw["sentence"] = json!("reply %");
        raw["variables"] = json!([{"name":"answer","position":0,"encoding":"verbatim"}]);
        raw["examples"] = json!(["reply literal answer"]);
        raw["negative_examples"] = json!(["not a reply"]);
        let root = drafts
            .iter_mut()
            .find(|draft| draft.uuid() == ids.recipe)
            .unwrap();
        let mut envelope: Value = serde_json::from_str(root.exact_bytes()).unwrap();
        envelope["document"]["mcp_call"] = raw;
        *root = ComponentRevisionDraft::from_json(&envelope.to_string()).unwrap();
        // Structural draft only: no claimed Tool implementation or approval.
        // Discoverability must not follow from preparing and inspecting it.
        drafts.push(ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.tool,"class_code":0,
            "document":{"capability_id":"host.post_reply","callable":"host.post_reply","input_contract":{"answer":{"type":"string","required":true,"checks":[]}}},
            "dependencies":[],"association":null}).to_string()).unwrap());
        let revisions = drafts
            .into_iter()
            .map(|draft| draft.at_version(1).unwrap())
            .collect();
        let snapshot = Arc::new(RetainedComponentSnapshot::new(&[ids.recipe], revisions).unwrap());
        let program = prepare_retained_tool_program(
            compile_retained_recipe(
                snapshot,
                ids.recipe,
                "selected",
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap();
        let worker = brassclaw_monty_host::process::installed_worker().unwrap();
        let inspected =
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(Arc::new(program)), &worker)
                .await
                .unwrap();
        let discovery = McpRecipeDiscovery::new();
        assert!(matches!(
            discovery.publish_installed(None, Uuid::from_u128(20), &[&inspected]),
            Err(McpDiscoveryError::UnsupportedRunner)
        ));
        assert!(discovery.snapshot().is_err());
        assert!(matches!(
            discovery.publish_installed(None, Uuid::from_u128(21), &[&inspected, &inspected]),
            Err(McpDiscoveryError::Conflict)
        ));
        let revisions = inspected
            .program()
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
            .values()
            .map(|revision| {
                let mut raw: Value = serde_json::from_str(revision.draft().exact_bytes()).unwrap();
                if revision.reference().uuid == ids.recipe {
                    raw["document"]["variants"][0]["variable_patterns"][0]["pattern"] =
                        json!("prefix-(?P<answer>.*)");
                }
                ComponentRevisionDraft::from_json(&raw.to_string())
                    .unwrap()
                    .at_version(
                        revision.reference().version
                            + u64::from(revision.reference().uuid == ids.recipe),
                    )
                    .unwrap()
            })
            .collect();
        let snapshot = Arc::new(RetainedComponentSnapshot::new(&[ids.recipe], revisions).unwrap());
        let program = prepare_retained_tool_program(
            compile_retained_recipe(
                snapshot,
                ids.recipe,
                "selected",
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap();
        let regex_capture =
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(Arc::new(program)), &worker)
                .await
                .unwrap();
        assert!(
            matches!(
                McpRecipeDiscovery::contracts(Uuid::from_u128(22), &[&regex_capture]),
                Err(McpDiscoveryError::Contract)
            ),
            "sample agreement cannot qualify a regex that changes arbitrary captured values"
        );
    }

    #[test]
    fn qualified_mcp_command_contract_preserves_data_and_rejects_bad_layouts() {
        let contract = CommandContract::parse(&declaration(), &task()).unwrap();
        let text = "read file ü'\\{{vars.x}}%; interval 1:4";
        assert_eq!(contract.bind(text).unwrap()["path"], "ü'\\{{vars.x}}%");
        for text in [
            "read file ; interval 1:4",
            " read file a; interval 1:4",
            "read file a; interval 1:4; interval 5:6",
        ] {
            assert!(contract.bind(text).is_err());
        }
        for field in ["source", "approved", "validation_mode"] {
            let mut raw = declaration();
            raw[field] = json!("system_seed");
            assert!(CommandContract::parse(&raw, &task()).is_err());
        }
        let mut raw = declaration();
        raw["variables"][1]["position"] = json!(0);
        assert!(CommandContract::parse(&raw, &task()).is_err());
        let mut raw = declaration();
        raw["export_name"] = json!("host");
        assert!(CommandContract::parse(&raw, &task()).is_err());
        let mut schemas = task();
        schemas["interval"]["type"] = json!("integer");
        assert!(CommandContract::parse(&declaration(), &schemas).is_err());
        let mut raw = declaration();
        raw["negative_examples"] = raw["examples"].clone();
        assert!(CommandContract::parse(&raw, &task()).is_err());
    }

    #[test]
    fn qualified_mcp_discovery_retains_advertising_but_rejects_withdrawn_or_changed_commands() {
        // These are parser/publication tests, not fabricated runtime approval.
        let contract = CommandContract::parse(&declaration(), &task()).unwrap();
        let discovery = McpRecipeDiscovery::new();
        assert!(discovery.snapshot().is_err());
        let original = Arc::new(McpRecipeDiscoverySnapshot {
            generation: Uuid::from_u128(1),
            entries: BTreeMap::from([("read_interval".into(), contract.clone())]),
            qualification_checksum: None,
        });
        *discovery.current.write().unwrap() = Some(original.clone());
        let command = "read file /tmp/a; interval 1:4";
        assert!(
            discovery
                .validate_advertised_command(&original, "read_interval", command)
                .is_ok()
        );
        let mut raw = declaration();
        raw["result_description"] = json!("Changed result contract.");
        let changed = CommandContract::parse(&raw, &task()).unwrap();
        *discovery.current.write().unwrap() = Some(Arc::new(McpRecipeDiscoverySnapshot {
            generation: Uuid::from_u128(2),
            entries: BTreeMap::from([("read_interval".into(), changed)]),
            qualification_checksum: None,
        }));
        assert!(matches!(
            discovery.validate_advertised_command(&original, "read_interval", command),
            Err(McpDiscoveryError::StaleCommand)
        ));
        assert!(matches!(
            discovery.publish_installed(Some(Uuid::from_u128(1)), Uuid::from_u128(3), &[]),
            Err(McpDiscoveryError::Conflict)
        ));
        discovery
            .publish_installed(Some(Uuid::from_u128(2)), Uuid::from_u128(3), &[])
            .unwrap();
        assert_eq!(original.tools_list()["tools"].as_array().unwrap().len(), 1);
        assert!(
            discovery
                .validate_advertised_command(&original, "read_interval", command)
                .is_err()
        );
        assert!(
            discovery
                .publish_installed(Some(Uuid::from_u128(3)), Uuid::nil(), &[])
                .is_err()
        );
        assert!(discovery.snapshot().is_err());
    }
}
