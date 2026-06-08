#!/usr/bin/env bash
set -euo pipefail

has_core_code=false
docs_only=true
has_legacy_tests=false
has_reborn_tests=false

is_docs_only_path() {
  local path="$1"
  case "$path" in
    docs/*|.github/ISSUE_TEMPLATE/*|.github/pull_request_template.md)
      return 0
      ;;
    *.md)
      case "$path" in
        */*) return 1 ;;
        *) return 0 ;;
      esac
      ;;
    *)
      return 1
      ;;
  esac
}

is_shared_test_path() {
  local path="$1"
  case "$path" in
    Cargo.toml|Cargo.lock|build.rs|providers.json|Dockerfile)
      return 0
      ;;
    scripts/ci/classify-test-scope.sh|scripts/ci/test-classify-test-scope.sh|scripts/ci/package-feature-flags.sh)
      return 0
      ;;
    .github/workflows/test.yml|.github/workflows/reborn-tests.yml|.github/workflows/reborn-integration.yml|.github/workflows/reborn-e2e.yml|.github/workflows/nightly-deep-ci.yml)
      return 0
      ;;
    crates/brassclaw_common/*|crates/brassclaw_host_api/*|crates/brassclaw_host_runtime/*|crates/brassclaw_loop_support/*)
      return 0
      ;;
    crates/brassclaw_filesystem/*|crates/brassclaw_memory/*|crates/brassclaw_events/*|crates/brassclaw_event_projections/*|crates/brassclaw_event_streams/*)
      return 0
      ;;
    crates/brassclaw_capabilities/*|crates/brassclaw_secrets/*|crates/brassclaw_network/*|crates/brassclaw_runtime_policy/*)
      return 0
      ;;
    crates/brassclaw_authorization/*|crates/brassclaw_run_state/*|crates/brassclaw_approvals/*|crates/brassclaw_resources/*)
      return 0
      ;;
    crates/brassclaw_auth/*|crates/brassclaw_trust/*|crates/brassclaw_turns/*|crates/brassclaw_agent_loop/*|crates/brassclaw_threads/*)
      return 0
      ;;
    crates/brassclaw_prompt_envelope/*|crates/brassclaw_hooks/*|crates/brassclaw_first_party_extensions/*|crates/brassclaw_llm/*)
      return 0
      ;;
    crates/brassclaw_embeddings/*|crates/brassclaw_safety/*|crates/brassclaw_skills/*|crates/brassclaw_oauth/*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_reborn_test_path() {
  local path="$1"
  case "$path" in
    docs/reborn/*|scripts/reborn-e2e-rust.sh|scripts/ci/run-reborn-root-partition.sh|tests/reborn_*|tests/support/reborn/*|tests/e2e/scenarios/test_reborn_*)
      return 0
      ;;
    crates/brassclaw_architecture/*)
      return 0
      ;;
    crates/brassclaw_reborn/*|crates/brassclaw_reborn_*/*)
      return 0
      ;;
    crates/brassclaw_product_*/*|crates/brassclaw_slack_v2_adapter/*|crates/brassclaw_telegram_v2_adapter/*)
      return 0
      ;;
    crates/brassclaw_wasm_product_adapters/*|crates/brassclaw_webui_v2/*|crates/brassclaw_webui_v2_static/*)
      return 0
      ;;
    crates/brassclaw_conversations/*|crates/brassclaw_outbound/*|crates/brassclaw_triggers/*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_code_path() {
  local path="$1"
  case "$path" in
    src/*|crates/*|channels-src/*|tools-src/*|tests/*|migrations/*)
      return 0
      ;;
    Cargo.toml|Cargo.lock|Dockerfile|build.rs|providers.json)
      return 0
      ;;
    scripts/check_no_panics.py|scripts/check_gateway_boundaries.py|scripts/build-wasm-extensions.sh|scripts/check-version-bumps.sh|scripts/reborn-e2e-rust.sh|scripts/ci/*)
      return 0
      ;;
    .github/workflows/*.yml|.github/actions/install-cargo-component/*|.github/dependabot.yml|.github/labeler.yml)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

while IFS= read -r path || [ -n "$path" ]; do
  [ -n "$path" ] || continue

  if ! is_docs_only_path "$path"; then
    docs_only=false
  fi

  if is_code_path "$path"; then
    has_core_code=true
  fi

  if is_shared_test_path "$path"; then
    has_legacy_tests=true
    has_reborn_tests=true
  elif is_reborn_test_path "$path"; then
    has_reborn_tests=true
  elif is_code_path "$path"; then
    has_legacy_tests=true
  fi
done

cat <<EOF
docs_only=${docs_only}
has_core_code=${has_core_code}
has_legacy_tests=${has_legacy_tests}
has_reborn_tests=${has_reborn_tests}
EOF
