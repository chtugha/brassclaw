//! Independently specified joint-source data. Submitted Tool-using code is
//! inspected, never called; these fixtures confer no component approval.
use super::*;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft,
    global_bootstrap_components::{UsageComponentIds, public_reply_drafts},
};

pub(crate) struct FunctionSourceCase {
    pub(crate) name: String,
    pub(crate) candidate: Uuid,
    pub(crate) partner: Uuid,
    pub(crate) drafts: Vec<ComponentRevisionDraft>,
    pub(crate) primary_class: i16,
    pub(crate) rules: Vec<Value>,
}
impl FunctionSourceCase {
    pub(crate) fn subject_data(&self) -> Result<Value, RebornBuildError> {
        let entry = |draft: &ComponentRevisionDraft| -> Result<Value, RebornBuildError> {
            Ok(
                json!({"reference":{"uuid":draft.uuid(),"class_code":draft.class_code(),
                "version":1,"checksum":evidence::checksum(draft.exact_bytes())},
                "revision_bytes":draft.exact_bytes(),
                "revision":strict_json(draft.exact_bytes(),REVISION_LIMITS).map_err(invalid)?}),
            )
        };
        let find = |id| {
            self.drafts
                .iter()
                .find(|draft| draft.uuid() == id)
                .ok_or_else(|| invalid("function fixture root missing"))
        };
        let original = json!({"format":"function-source-fixture-data/1","name":self.name,
            "components":self.drafts.iter().map(|draft| draft.exact_bytes()).collect::<Vec<_>>()})
        .to_string();
        Ok(json!({"format":"installation-validator-subject/2",
            "subject_id":Uuid::new_v5(&self.candidate,original.as_bytes()),
            "subject_bytes":original,"subject_checksum":evidence::checksum(&original),
            "candidate":entry(find(self.candidate)?)?,
            "function_partner":{"role":"subject","component":entry(find(self.partner)?)?,"base_comparison":null},
            "dependencies":self.drafts.iter().filter(|draft| ![self.candidate,self.partner].contains(&draft.uuid()))
                .map(entry).collect::<Result<Vec<_>,_>>()?,"base_comparison":null}))
    }
}

pub(crate) fn function_source_cases() -> Result<Vec<FunctionSourceCase>, RebornBuildError> {
    let mut cases = Vec::new();
    for skill_class in [1, 2, 3] {
        for code_primary in [false, true] {
            for case in ["valid", "blank_prose", "foreign_host", "unreachable_host"] {
                let name = format!(
                    "skill-{skill_class}-{}-{case}",
                    if code_primary {
                        "code-primary"
                    } else {
                        "skill-primary"
                    }
                );
                let id = |role: &str| {
                    packaged_component_id(&format!(
                        "validator:function-source:fixture:{name}:{role}"
                    ))
                };
                let ids = UsageComponentIds {
                    recipe: id("recipe"),
                    tool: id("tool"),
                    tool_skill: id("binding"),
                    skill: id("skill"),
                    python_code: id("code"),
                    formatter: None,
                };
                let mut drafts = public_reply_drafts(ids).map_err(invalid)?;
                drafts.retain(|draft| draft.uuid() != ids.recipe);
                let mut revised = Vec::new();
                for draft in drafts {
                    let mut value =
                        strict_json(draft.exact_bytes(), REVISION_LIMITS).map_err(invalid)?;
                    if draft.uuid() == ids.skill {
                        value["class_code"] = json!(skill_class);
                        if case == "blank_prose" {
                            value["document"]["body"] = json!(" \n\t");
                        }
                    }
                    if draft.uuid() == ids.python_code {
                        let source = value["document"]["content"]
                            .as_str()
                            .ok_or_else(|| invalid("function fixture source missing"))?;
                        if case == "foreign_host" {
                            value["document"]["content"] =
                                json!(source.replace("host.post_reply", "host.foreign_tool"));
                        } else if case == "unreachable_host" {
                            value["document"]["content"] = json!(format!(
                                "{source}\ndef unreachable_foreign(inputs):\n    return host.foreign_tool(answer=inputs['answer'])\n"
                            ));
                            value["document"]["preload"]["private_functions"]["unreachable_foreign"] =
                                json!(["inputs"]);
                            value["document"]["name"] =
                                json!("Inspect unreachable Host sites without executing any usage");
                        }
                    }
                    revised.push(
                        ComponentRevisionDraft::from_json(&value.to_string()).map_err(invalid)?,
                    );
                }
                // This is source fixture metadata, never a registered Tool or
                // permission. No candidate is dispatched in this gallery.
                revised.push(ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1", "uuid":ids.tool,"class_code":0,
                    "document":{"capability_id":"host.post_reply","callable":"host.post_reply"},"dependencies":[],"association":null}).to_string()).map_err(invalid)?);
                revised.sort_by_key(|draft| draft.uuid());
                let mut rules = brassclaw_skills::validator_components::FUNCTION_SOURCE_RULE_IDS
                    .iter()
                    .map(|rule| json!({"rule_id":rule,"ok":true,"errors":[]}))
                    .collect::<Vec<_>>();
                if case == "blank_prose" {
                    rules[3] = json!({"rule_id":"skill_interface","ok":false,"errors":["skill.body: nonblank usage prose required"]});
                    rules[4] = json!({"rule_id":"skill_call_targets","ok":false,"errors":["skill_call_targets: valid original graph and Skill interface required"]});
                } else if case == "foreign_host" {
                    rules[4] = json!({"rule_id":"skill_call_targets","ok":false,"errors":["function publish_turn_reply: observed Host target differs from the declared Skill usage"]});
                }
                cases.push(FunctionSourceCase {
                    name,
                    candidate: if code_primary {
                        ids.python_code
                    } else {
                        ids.skill
                    },
                    partner: if code_primary {
                        ids.skill
                    } else {
                        ids.python_code
                    },
                    drafts: revised,
                    primary_class: if code_primary { 22 } else { skill_class },
                    rules,
                });
            }
        }
    }
    Ok(cases)
}
