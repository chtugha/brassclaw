//! Declarative composition of the existing budget planner and captured selector.
//! No policy lookup, inference, approval or independent generation workflow.
use super::*;

#[derive(Clone, Copy)]
pub struct BudgetedDesignIds {
    pub budget: Uuid,
    pub receipt: Uuid,
}

fn rebase(reference: &str) -> Result<String, RevisionError> {
    let number = reference
        .strip_prefix("1:")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|value| value.checked_add(1))
        .ok_or(RevisionError::Invalid("captured step reference invalid"))?;
    Ok(format!("2:{number}"))
}

/// Add a budgeted variant to the SAME Recipe. Existing variants and component
/// bodies are retained; the admitted owner must resolve the canonical inputs.
pub fn budgeted_design_selection_drafts(
    ids: DesignSelectionIds,
    captured: CapturedDesignSelectionIds,
    budgeted: BudgetedDesignIds,
    units: Uuid,
    renderer: Uuid,
    digest: UsageComponentIds,
    encoding: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut drafts =
        captured_design_selection_drafts(ids, captured, units, renderer, digest, encoding)?;
    let reserved = drafts
        .iter()
        .flat_map(|draft| std::iter::once(draft.uuid()).chain(draft.dependencies().iter().copied()))
        .collect::<std::collections::BTreeSet<_>>();
    if budgeted.budget.is_nil()
        || budgeted.receipt.is_nil()
        || budgeted.budget == budgeted.receipt
        || reserved.contains(&budgeted.budget)
        || reserved.contains(&budgeted.receipt)
    {
        return Err(RevisionError::Invalid(
            "budget component identities conflict",
        ));
    }
    let budget = crate::prefix_compiler_components::model_budget_draft(budgeted.budget)?;
    let recipe_index = drafts
        .iter()
        .position(|draft| draft.uuid() == ids.recipe)
        .ok_or(RevisionError::Invalid("selection Recipe missing"))?;
    let recipe = drafts.remove(recipe_index);
    let mut document = recipe.document().clone();
    let original = document["step_descriptions"]
        .as_array()
        .and_then(|groups| groups.iter().find(|group| group["desc_idx"] == 1))
        .and_then(|group| group["steps"].as_array())
        .ok_or(RevisionError::Invalid("captured steps missing"))?;
    let component = |number, uuid, goal| json!({"stepnumber":number,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[uuid]});
    let mut steps = vec![component(
        1,
        budgeted.budget,
        "Plan from pinned canonical model limits",
    )];
    for (index, step) in original.iter().enumerate() {
        let mut step = step.clone();
        step["stepnumber"] = json!(index + 2);
        steps.push(step);
    }
    let captured_result = format!("2:{}", steps.len());
    steps.push(component(
        steps.len() + 1,
        budgeted.receipt,
        "Preserve budget and complete captured selection together",
    ));
    let task = |name: &str| json!({"kind":"task_input","reference":format!("{{{{vars.{name}}}}}")});
    let result = |step: &str, path: Vec<&str>| json!({"kind":"result","step_id":step,"path":path});
    let budget_fields = budget.document()["input_contract"]
        .as_object()
        .ok_or(RevisionError::Invalid("budget input contract missing"))?;
    let mut budget_layout = serde_json::Map::new();
    for name in budget_fields.keys() {
        budget_layout.insert(name.clone(), task(name));
    }
    let mut layouts = json!({"2:1":budget_layout});
    for (step, layout) in document["input_layouts"]["captured"]["steps"]
        .as_object()
        .ok_or(RevisionError::Invalid("captured layouts missing"))?
    {
        let mut layout = layout.clone();
        for (name, binding) in layout
            .as_object_mut()
            .ok_or(RevisionError::Invalid("captured bindings invalid"))?
        {
            if binding["kind"] == "result" {
                binding["step_id"] = json!(rebase(
                    binding["step_id"]
                        .as_str()
                        .ok_or(RevisionError::Invalid("captured result invalid"))?
                )?);
            } else if name == "prefix_budget" {
                *binding = result("2:1", vec!["effective_prefix_budget"]);
            } else if name == "policy_ref" {
                *binding = result("2:1", vec!["policy_ref"]);
            }
        }
        layouts[rebase(step)?] = layout;
    }
    let receipt_step = format!("2:{}", steps.len());
    layouts[&receipt_step] = json!({"budget":result("2:1",vec![]),"captured_receipt_bytes":result(&captured_result,vec![])});
    let mut invocations = json!({"2:1":"plan_prefix_model_budget"});
    for (step, export) in document["invocations"]["captured"]
        .as_object()
        .ok_or(RevisionError::Invalid("captured invocations missing"))?
    {
        invocations[rebase(step)?] = export.clone();
    }
    invocations[&receipt_step] = json!("combine_budgeted_prefix_design");
    let mut inputs = document["input_layouts"]["captured"]["task_inputs"]
        .as_object()
        .ok_or(RevisionError::Invalid("captured input contract missing"))?
        .clone();
    inputs.remove("prefix_budget");
    inputs.extend(budget_fields.clone());
    let command = format!(
        "plan captured prefix {}",
        inputs
            .keys()
            .map(|name| format!("{name}=%"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let variables = inputs.keys().map(|name|json!({"name":name,"pattern":null,"description":"Pinned budgeted captured-source input"})).collect::<Vec<_>>();
    document["variants"].as_array_mut().ok_or(RevisionError::Invalid("selection variants missing"))?.push(json!({"variant_key":"budgeted","step_link":"2:1-2:E","intent_examples":[command],"variable_patterns":variables}));
    document["step_descriptions"].as_array_mut().ok_or(RevisionError::Invalid("selection steps missing"))?.push(json!({"desc_idx":2,"label":"Plan canonical capacity and select complete captured knowledge","yaml_source":"","steps":steps}));
    document["input_layouts"]["budgeted"] =
        json!({"format":"recipe-input-layout/1","task_inputs":inputs,"steps":layouts});
    document["invocations"]["budgeted"] = invocations;
    let mut budget_contract = budget.document()["result_contract"].clone();
    budget_contract["required"] = json!(true);
    budget_contract["checks"] = json!([]);
    let receipt = pure(
        budgeted.receipt,
        "combine_budgeted_prefix_design",
        r#"def combine_budgeted_prefix_design(inputs):
    import json
    captured = json.loads(inputs['captured_receipt_bytes'])
    if captured['format'] != 'brassclaw-captured-prefix-design-selection/1':
        raise ValueError('captured selection receipt required')
    selected = json.loads(captured['selection_bytes'])
    budget = inputs['budget']
    if selected['prefix_budget'] != budget['effective_prefix_budget'] or selected['policy_ref'] != budget['policy_ref']:
        raise ValueError('selection differs from original planned policy')
    return json.dumps({'format':'brassclaw-budgeted-prefix-design-selection/1','budget':budget,'captured_receipt_bytes':inputs['captured_receipt_bytes'],'qualification':'planned-reference-only; no accepted design, envelope fit or activation'}, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
"#.into(),
        json!({"budget":budget_contract,"captured_receipt_bytes":string()}),
        json!({"type":"string","checks":[{"kind":"min_length","value":1}]}),
        vec![],
        json!({}),
    )?;
    let mut dependencies = recipe.dependencies().clone();
    dependencies.extend([budgeted.budget, budgeted.receipt]);
    drafts.extend([budget, receipt, ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.recipe,"class_code":21,"document":document,"dependencies":dependencies,"association":null}).to_string())?]);
    Ok(drafts)
}
