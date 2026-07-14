// Phase-4 v1-script-runtime removal deleted every dispatched-capability
// `#[tokio::test]` in this file because each one routed work through the
// now-removed `ScriptBackend`. The dispatcher has no Mcp adapter wired yet,
// so an `#[tokio::test]` here that exercised `invoke_capability` would only
// fail at runtime for an unrelated reason. Phase 5 should reintroduce the
// E2E gate suite against a Mcp-backed adapter.
