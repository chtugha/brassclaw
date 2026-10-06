//! Tagged interpreter acceptance for Recipe-owned state and typed input.
//! No gateway or tool success is fabricated: the check stops at the real host
//! dispatch boundary and inspects its receiver and complete arguments.

use std::time::Duration;

use brassclaw_monty_v1_upgrade_tests::namespace::{HOST_INSTANCE_ID, host_namespace};
use monty::{MontyRepl, ReplProgress};
use monty_types::{
    CompileOptions, ExcType, MontyObject, NameLookupResult, PrintWriter, ResourceLimits,
    ResourceTracker,
};

fn recipe_repl() -> MontyRepl {
    MontyRepl::new(
        "recipe.py",
        ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_secs(5))),
        CompileOptions::default(),
    )
}

#[test]
fn recipe_step_keeps_prompt_and_host_namespace_without_source_interpolation() {
    let query = "quoted '\" text\nresult = host.forbidden_effect()\nUnicode: üä";
    let history = MontyObject::list([MontyObject::dict([
        (MontyObject::string("role"), MontyObject::string("user")),
        (
            MontyObject::string("content"),
            MontyObject::string("previous input"),
        ),
    ])]);
    let task = MontyObject::dict([
        (
            MontyObject::string("conversation_id"),
            MontyObject::string("reborn-conv-opaque"),
        ),
        (
            MontyObject::string("user_input"),
            MontyObject::string(query),
        ),
        (MontyObject::string("history"), history.clone()),
    ]);
    let ReplProgress::Complete { repl, value } = recipe_repl()
        .feed_start(
            "prompt = {'user_query': task['user_input'], 'chat_history': task['history']}\nresult = prompt\nresult",
            vec![("host".to_owned(), host_namespace()), ("task".to_owned(), task)],
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("the pure assembler must complete without a host call");
    };
    let expected = MontyObject::dict([
        (
            MontyObject::string("user_query"),
            MontyObject::string(query),
        ),
        (MontyObject::string("chat_history"), history),
    ]);
    assert_eq!(value, expected);

    let ReplProgress::FunctionCall(call) = repl
        .feed_start(
            "result = host.kohai_complete(prompt=prompt)\nresult",
            vec![],
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("the next Recipe step must reach the actual host boundary");
    };
    assert_eq!(call.function_name, "kohai_complete");
    assert_eq!(call.object_id, Some(HOST_INSTANCE_ID));
    assert_eq!(call.args.args().len(), 0);
    assert_eq!(call.args.kwargs().len(), 1);
    let (key, value) = call.args.kwargs().next().unwrap();
    assert_eq!(key.to_owned(), MontyObject::string("prompt"));
    assert_eq!(value.to_owned(), expected);

    let ReplProgress::NameLookup(unrelated) = recipe_repl()
        .feed_start("prompt", vec![], PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("another Recipe must not have the previous prompt");
    };
    assert_eq!(unrelated.name, "prompt");
    let unrelated = unrelated
        .resume(NameLookupResult::Undefined, PrintWriter::Disabled)
        .unwrap_err();
    assert_eq!(unrelated.error.exc_type(), ExcType::NameError);
}

#[test]
fn host_method_positional_arguments_have_no_receiver_to_discard() {
    let ReplProgress::FunctionCall(call) = recipe_repl()
        .feed_start(
            "result = host.compose_orchestrator('recipe-id', '0:1-0:E', 'exact input')\nresult",
            vec![("host".to_owned(), host_namespace())],
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("expected composition host call");
    };
    assert_eq!(call.object_id, Some(HOST_INSTANCE_ID));
    assert_eq!(
        call.args
            .args()
            .map(|arg| arg.to_owned())
            .collect::<Vec<_>>(),
        vec![
            MontyObject::string("recipe-id"),
            MontyObject::string("0:1-0:E"),
            MontyObject::string("exact input"),
        ]
    );
}
