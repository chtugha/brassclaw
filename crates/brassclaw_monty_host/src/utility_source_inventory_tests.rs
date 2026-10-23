//! Native worker acceptance; candidate sources are never evaluated.
use super::*;
use crate::source_inventory::{CompilerRejection, SourceInspection};
use serde_json::json;
use sha2::{Digest, Sha256};

fn policy() -> UtilityLimits {
    UtilityLimits::from_execution(Default::default()).unwrap()
}
fn request(sources: &BTreeMap<String, String>, bounds: VmBounds) -> UtilityRequest {
    UtilityRequest::InspectSources {
        sources: sources.clone(),
        bounds,
    }
}
fn nodes(value: &Value) -> usize {
    let mut count = 0;
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        count += 1;
        match value {
            Value::Array(values) => pending.extend(values),
            Value::Object(values) => pending.extend(values.values()),
            _ => {}
        }
    }
    count
}

#[tokio::test]
async fn complete_source_inventory_retains_real_facts_and_compiler_rejections_without_execution() {
    let worker = crate::process::installed_worker().unwrap();
    let limits = policy();
    let sources = BTreeMap::from([
        (
            "opaque Ü {{vars.value}}".into(),
            "def identity(value):\n    return value\n".into(),
        ),
        (
            "not-a-file-path".into(),
            "host.post_reply(answer='never executed')\nraise RuntimeError('never evaluated')\n"
                .into(),
        ),
        ("invalid-source".into(), "def private_fragment(:\n".into()),
        (
            "unsupported-profile".into(),
            "match value:\n    case _:\n        pass\n".into(),
        ),
        (
            "relative-import".into(),
            "from .private_fragment import value\n".into(),
        ),
        ("empty-source".into(), String::new()),
    ]);
    let UtilityOutput::InspectedSources {
        sources: mut inspected,
    } = execute(&worker, request(&sources, limits.bounds), limits.process)
        .await
        .unwrap()
    else {
        panic!("complete actual inventory required")
    };
    assert_eq!(
        inspected.keys().collect::<Vec<_>>(),
        sources.keys().collect::<Vec<_>>()
    );
    assert!(crate::source_inventory::valid_for(
        &inspected,
        &sources,
        limits.bounds
    ));
    let SourceInspection::Inspected { structure } = &inspected["opaque Ü {{vars.value}}"] else {
        panic!("actual function facts required")
    };
    assert_eq!(
        structure.preload.as_ref().unwrap().functions["identity"],
        vec!["value".to_owned()]
    );
    assert_eq!(
        structure.source_checksum,
        format!(
            "{:x}",
            Sha256::digest(sources["opaque Ü {{vars.value}}"].as_bytes())
        )
    );
    let SourceInspection::Inspected { structure } = &inspected["not-a-file-path"] else {
        panic!("parsing must not execute the module's host call or raise")
    };
    assert_eq!(structure.direct_host_calls.len(), 1);
    assert_eq!(structure.direct_host_calls[0].attribute, "post_reply");
    assert!(structure.preload.is_none());
    assert!(matches!(
        &inspected["empty-source"],
        SourceInspection::Inspected { .. }
    ));
    let SourceInspection::CompileRejected {
        source_checksum,
        category,
    } = &inspected["invalid-source"]
    else {
        panic!("actual compiler rejection required")
    };
    assert!(matches!(category, CompilerRejection::Syntax));
    assert!(matches!(
        &inspected["unsupported-profile"],
        SourceInspection::CompileRejected {
            category: CompilerRejection::Unsupported,
            ..
        }
    ));
    assert!(matches!(
        &inspected["relative-import"],
        SourceInspection::CompileRejected {
            category: CompilerRejection::Import,
            ..
        }
    ));
    assert_eq!(
        *source_checksum,
        format!("{:x}", Sha256::digest(sources["invalid-source"].as_bytes()))
    );
    let mut changed = sources.clone();
    changed.insert("invalid-source".into(), "changed source".into());
    assert!(!crate::source_inventory::valid_for(
        &inspected,
        &changed,
        limits.bounds
    ));
    inspected.remove("empty-source");
    assert!(!crate::source_inventory::valid_for(
        &inspected,
        &sources,
        limits.bounds
    ));
    let UtilityOutput::InspectedSources { sources: empty } = execute(
        &worker,
        request(&BTreeMap::new(), limits.bounds),
        limits.process,
    )
    .await
    .unwrap() else {
        panic!("empty generic inventory is explicit, not a validation verdict")
    };
    assert!(empty.is_empty());
}

#[tokio::test]
async fn inventory_limits_are_aggregate_and_never_publish_a_partial_success() {
    let worker = crate::process::installed_worker().unwrap();
    let limits = policy();
    let two = BTreeMap::from([
        ("one".into(), "x=1\n".into()),
        ("two".into(), "x=1\n".into()),
    ]);
    let mut bounded = limits.bounds;
    bounded.max_source_bytes = 7;
    let error = execute(&worker, request(&two, bounded), limits.process)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProcessFailure::ValueLimit);
    assert!(
        error.exit_status.is_none(),
        "reject total source size before spawn"
    );
    assert!(error.diagnostic.is_none());
    let UtilityRequest::InspectSources {
        sources: original, ..
    } = &*error.request
    else {
        panic!("original complete request retained")
    };
    assert_eq!(*original, two);
    for bad in [json!({"bad":null}), json!({"":"x=1"})] {
        assert!(!crate::source_inventory::source_values_within_bounds(
            bad.as_object().unwrap(),
            limits.bounds,
            limits.process.max_frame_bytes
        ));
    }

    let one = BTreeMap::from([(
        "one".into(),
        "def named(value):\n    other = value\n    return other\n".into(),
    )]);
    let single = execute(&worker, request(&one, limits.bounds), limits.process)
        .await
        .unwrap();
    let node_limit = 2 * nodes(&serde_json::to_value(&single).unwrap());
    let mut bound = limits.bounds;
    bound.max_value_nodes = node_limit;
    assert!(matches!(
        execute(&worker, request(&one, bound), limits.process)
            .await
            .unwrap(),
        UtilityOutput::InspectedSources { .. }
    ));
    let many = (0..4)
        .map(|index| (format!("row-{index}"), one["one"].clone()))
        .collect();
    let error = execute(&worker, request(&many, bound), limits.process)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProcessFailure::Vm(VmFailure::ResourceLimit));
    assert!(
        error.exit_status.unwrap().success(),
        "framed whole-operation rejection"
    );
    let UtilityRequest::InspectSources {
        sources: original, ..
    } = &*error.request
    else {
        panic!("complete request retained after late overflow")
    };
    assert_eq!(original.len(), 4);

    // Die Grenze folgt den tatsächlich serialisierten Ein- und Ausgabedaten,
    // nicht einer geratenen Größenkonstante oder einem Ersatz-Worker.
    let request_size = encode(
        &Request {
            protocol: PROTOCOL,
            command: request(&one, limits.bounds),
        },
        limits.process.max_frame_bytes,
    )
    .unwrap()
    .len();
    let response_size = encode(
        &Reply {
            protocol: PROTOCOL,
            output: Some(single),
            failure: None,
            diagnostic: None,
            stdout: String::new(),
        },
        limits.process.max_frame_bytes,
    )
    .unwrap()
    .len();
    assert!(response_size > request_size);
    let frame = ProcessLimits {
        max_frame_bytes: response_size - 1,
        ..limits.process
    };
    assert!(frame.max_frame_bytes >= request_size);
    let error = execute(&worker, request(&one, limits.bounds), frame)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProcessFailure::Vm(VmFailure::ResourceLimit));
    assert!(error.exit_status.unwrap().success());
    assert!(matches!(
        execute(&worker, request(&one, limits.bounds), limits.process)
            .await
            .unwrap(),
        UtilityOutput::InspectedSources { .. }
    ));
}
