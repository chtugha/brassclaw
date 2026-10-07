//! Real disposable-worker parser/formatter contracts for the 1.0 cutover.
use std::{collections::BTreeMap, time::Duration};

use brassclaw_monty_host::{
    VmFailure,
    process::{ProcessFailure, ProcessLimits},
    utility::{UtilityOutput, UtilityRequest, execute},
};
use serde_json::json;

mod support;
use support::{SOURCE, boot, limits, task, worker};

fn evaluate(source: &str, inputs: BTreeMap<String, serde_json::Value>) -> UtilityRequest {
    UtilityRequest::Evaluate {
        source: source.into(),
        inputs,
        bounds: boot(SOURCE).bounds.values,
        max_compute_time: Duration::from_secs(2),
    }
}

#[tokio::test]
async fn parser_never_executes_and_pure_formatter_preserves_hostile_typed_values() {
    let output = execute(
        worker(),
        UtilityRequest::Parse {
            source: "raise RuntimeError('must not execute')".into(),
            bounds: boot(SOURCE).bounds.values,
        },
        limits(),
    )
    .await
    .unwrap();
    assert!(matches!(output, UtilityOutput::Parsed));
    let syntax = execute(
        worker(),
        UtilityRequest::Parse {
            source: "def broken(:".into(),
            bounds: boot(SOURCE).bounds.values,
        },
        limits(),
    )
    .await
    .unwrap_err();
    assert_eq!(syntax.kind, ProcessFailure::Vm(VmFailure::Python));
    assert!(syntax.exit_status.unwrap().success());
    assert!(syntax.diagnostic.unwrap().contains("SyntaxError"));

    let text = "'\"\nresult = host.forbidden_effect()\nüä";
    let output = execute(worker(), evaluate(
            "print('formatter')\n{'text': state['value'], 'none': None, 'unsigned': state['unsigned'], 'conversation': admitted['conversation_id']}",
            BTreeMap::from([
                ("state".into(), json!({"value":text,"unsigned":u64::MAX})),
                ("admitted".into(), task()),
            ]),
    ), limits()).await.unwrap();
    let UtilityOutput::Evaluated { value, stdout } = output else {
        panic!("formatter result")
    };
    assert_eq!(
        value,
        json!({"text":text,"none":null,"unsigned":u64::MAX,"conversation":"reborn-conv-opaque"})
    );
    assert_eq!(stdout, "formatter\n");
    // A following utility gets fresh state, not the previous formatter globals.
    let isolated = execute(worker(), evaluate("state", BTreeMap::new()), limits())
        .await
        .unwrap_err();
    assert_eq!(isolated.kind, ProcessFailure::Vm(VmFailure::Python));
    assert!(isolated.diagnostic.unwrap().contains("NameError"));
}

#[tokio::test]
async fn utility_has_no_host_authority_and_rejects_invalid_output_without_null_substitution() {
    for source in [
        "host.post_reply(answer='no')",
        "open('/etc/passwd').read()",
        "import time\ntime.sleep(1)",
    ] {
        let failed = execute(worker(), evaluate(source, BTreeMap::new()), limits())
            .await
            .unwrap_err();
        assert_eq!(failed.kind, ProcessFailure::Vm(VmFailure::Python));
        assert!(failed.exit_status.unwrap().success());
    }
    for source in [
        "2 ** 100",
        "float('nan')",
        "value = []\nvalue.append(value)\nvalue",
    ] {
        let failed = execute(worker(), evaluate(source, BTreeMap::new()), limits())
            .await
            .unwrap_err();
        assert_eq!(failed.kind, ProcessFailure::Vm(VmFailure::InvalidResult));
        assert!(failed.exit_status.unwrap().success());
    }
    let failed = execute(
        worker(),
        evaluate("print('before')\nprint('x' * 2048)", BTreeMap::new()),
        limits(),
    )
    .await
    .unwrap_err();
    assert_eq!(failed.kind, ProcessFailure::Vm(VmFailure::Python));
    assert!(failed.stdout.starts_with("before\n"));
    assert!(failed.stdout.len() <= boot(SOURCE).bounds.values.max_stdout_bytes);
}

#[tokio::test]
async fn utility_deadline_and_fatal_allocator_failure_are_contained_and_reaped() {
    let deadline = execute(
        worker(),
        evaluate("while True:\n    pass", BTreeMap::new()),
        ProcessLimits {
            response_timeout: Duration::from_millis(50),
            ..limits()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(deadline.kind, ProcessFailure::Deadline);
    assert!(deadline.exit_status.is_some());
    assert!(deadline.reap_error.is_none());
    let allocation = execute(
        worker(),
        evaluate("'x' * (128 * 1024 * 1024)", BTreeMap::new()),
        limits(),
    )
    .await
    .unwrap_err();
    assert_eq!(allocation.kind, ProcessFailure::Transport);
    assert!(!allocation.exit_status.unwrap().success());
    assert!(allocation.reap_error.is_none());
    // Contained failure cannot terminate the caller or poison later utilities.
    let output = execute(worker(), evaluate("42", BTreeMap::new()), limits())
        .await
        .unwrap();
    assert!(matches!(output, UtilityOutput::Evaluated { value, .. } if value == json!(42)));
}

#[tokio::test]
async fn invalid_aggregate_inputs_are_retained_before_spawn_or_serialization() {
    let mut request = evaluate(
        "first",
        BTreeMap::from([
            ("first".into(), json!("private-a".repeat(1200))),
            ("second".into(), json!("private-b".repeat(1200))),
        ]),
    );
    // Each root fits; their combined strings exceed the declared value budget.
    let UtilityRequest::Evaluate { bounds, .. } = &mut request else {
        unreachable!()
    };
    bounds.max_value_bytes = 16_000;
    let rejected = execute(worker(), request, limits()).await.unwrap_err();
    assert_eq!(rejected.kind, ProcessFailure::ValueLimit);
    assert!(rejected.exit_status.is_none());
    assert!(!format!("{rejected:?}").contains("private-a"));
    let UtilityRequest::Evaluate { inputs, .. } = *rejected.request else {
        panic!("retained inputs")
    };
    assert_eq!(inputs["first"], json!("private-a".repeat(1200)));
}
