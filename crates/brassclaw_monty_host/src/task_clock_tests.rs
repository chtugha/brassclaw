//! Verify task observations against the real retained interpreter's clocks.
use super::*;
use brassclaw_resources::{LiveMontyTaskSettings, MontyTaskLimits, MontyTaskSettingsRevision};
use serde_json::json;

#[test]
fn actual_child_clocks_survive_feeds_and_live_edits_without_charging_adapters() {
    let settings = |revision, seconds| MontyTaskSettingsRevision {
        revision,
        limits: MontyTaskLimits {
            max_compute_time: Duration::from_secs(seconds),
            token_budgets_enabled: false,
        },
    };
    let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
    let budget = SharedMontyTaskBudget::new(live.clone());
    let bounds = VmBounds {
        max_source_bytes: 8192,
        max_compiled_source_bytes: 65536,
        max_feeds: 8,
        max_stdout_bytes: 1024,
        execution_slice: Duration::from_secs(1),
        max_value_depth: 16,
        max_value_nodes: 2048,
        max_value_bytes: 32768,
    };
    let mut vm = RecipeVm::new(budget.clone(), bounds).unwrap();
    let artifact = |code: &str| {
        Arc::new(
            PythonArtifact::new(
                Arc::from(code),
                Sha256::digest(code.as_bytes()).into(),
                BTreeSet::new(),
                bounds,
            )
            .unwrap(),
        )
    };
    let mut previous = budget.check().unwrap().usage;
    for (revision, code) in [
        (1, "saved = inputs['number']\nresult = saved + 1"),
        (2, "result = saved + inputs['number']"),
    ] {
        if revision == 2 {
            live.publish(1, settings(2, 30)).unwrap();
        }
        let result = vm
            .start_step(artifact(code), json!({"number": 41}))
            .unwrap();
        let VmBoundary::Complete(value) = result else {
            panic!("unexpected boundary: {result:?}");
        };
        assert_eq!(value, json!(if revision == 1 { 42 } else { 82 }));
        let VmState::Idle(repl) = &vm.state else {
            panic!("completed feed must retain its interpreter");
        };
        let snapshot = budget.check().unwrap();
        assert_eq!(snapshot.settings.revision, revision);
        assert_eq!(snapshot.usage.compute_time, repl.tracker().elapsed());
        assert_eq!(
            snapshot.usage.preparation_time,
            repl.tracker().preparation_elapsed().unwrap()
        );
        assert!(snapshot.usage.compute_time > previous.compute_time);
        assert!(snapshot.usage.preparation_time > previous.preparation_time);
        assert!(snapshot.usage.adaptation_time > previous.adaptation_time);
        previous = snapshot.usage;
    }
    vm.cancellation().request();
    assert_eq!(
        vm.start_step(artifact("result = 0"), json!({}))
            .unwrap_err()
            .failure,
        VmFailure::ResourceLimit
    );
    assert_eq!(budget.check().unwrap().usage, previous);
}
