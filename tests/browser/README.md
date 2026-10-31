# Native WebUI browser acceptance

These scripts render the original settings components in Chromium against the
native composition test's real router, facade, PostgreSQL and global Monty.
The fixtures exercise Tool allow/block, revision conflicts, lost committed
acknowledgements, MCP byte controls, effective settings revisions and EN/DE help.
Ordinary ingress authentication is covered separately by native ingress tests.

Use Node 24 or newer and install the locked development dependency:

```bash
npm ci --prefix tests/browser
tests/browser/node_modules/.bin/agent-browser install
export PATH="$PWD/tests/browser/node_modules/.bin:$PATH"
cargo build -p brassclaw_monty_host --bin monty_worker --locked
```

On Linux, `agent-browser install --with-deps` also installs Chromium's system
dependencies. Run the two scripts serially through the existing native fixture:

```bash
for script in instance-tool-policy.mjs monty-vm-resource-limits.mjs; do
  BRASSCLAW_DISABLE_OS_KEYCHAIN=1 RUST_MIN_STACK=16777216 \
    BRASSCLAW_TOOL_POLICY_BROWSER_ACCEPTANCE="$PWD/tests/browser/$script" \
    cargo test -p brassclaw_reborn_composition --locked \
      --features skills-db,test-support,migrate-from-libsql --lib \
      runtime::tests::native_global_runtime_retains_one_root_across_match_and_no_match \
      -- --exact --test-threads=1 --nocapture
done
```

The `native-settings-browser` job in `reborn-tests.yml` runs these checks on
Linux. Browser-script and dependency changes participate in Reborn test-scope
detection. Windows/macOS release and full installed-instance acceptance remain
separate gates.


## Original manual credential setup

The manual setup fixture renders the real Chat, hook, API and token card through
`webui_v2_app`, including Bearer protection, actual static modules, PostgreSQL Auth
custody and the runtime's actual continuation dispatcher. It deliberately drops
the first successful setup acknowledgement and the first successful credential
publication acknowledgement at the existing owner boundary. Reload retains the
same original IDs/first expiry; a changed token must fail; explicit Continue saved
setup sends no token and resumes the original durable auth gate once.

```bash
BRASSCLAW_MANUAL_TOKEN_BROWSER_ACCEPTANCE="$PWD/tests/browser/manual-token-original.mjs" \
  cargo test -p brassclaw_reborn_composition --locked \
    --features skills-db,test-support,migrate-from-libsql --lib \
    runtime::auth_interaction_tests::local_dev_runtime_auth_interactions_use_flow_record_source \
    -- --exact --test-threads=1 --nocapture
```

The test uses fixture-only Bearer/token sentinels, and stores/logs correlation
metadata rather than raw credentials. Its turn worker is stopped before building
the blocked state through the real typed submit/claim/block ports; the global Monty
service stays alive. This is browser/HTTP/Auth/continuation acceptance, not evidence
of a Recipe requesting a credential or executing again after auth. A run without
the environment variable exercises the existing native read-model test only; it
does not count as browser acceptance. The browser driver emits a required explicit
completion marker which the Rust fixture verifies.

## Original Extension credential setup

The Extension fixture renders the actual ConfigureModal, manual controller and API
through the ordinary protected WebUI bundle and native PostgreSQL Auth owner. It
uses the bundled NEAR AI manual requirement and an explicitly bound account from
the native lifecycle case. Middleware drops only successful acknowledgements after
real setup/publication. Reload and changed raw input preserve the original binding,
pair and first expiry; reference-only Continue confirms the result. A deliberate
new setup is then canceled after its setup acknowledgement was lost. No fake chat,
run/gate, saved result or issuer/model request is introduced.

```bash
BRASSCLAW_EXTENSION_MANUAL_BROWSER_ACCEPTANCE="$PWD/tests/browser/extension-manual-original.mjs" \
  cargo test -p brassclaw_reborn_composition --locked \
    --features skills-db,test-support,migrate-from-libsql --lib \
    runtime::tests::local_dev_webui_bundle_uses_local_lifecycle_facade_for_setup_extension \
    -- --exact --test-threads=1 --nocapture
```

The driver requires `native-extension-original-browser: passed`; an env-free native
lifecycle pass does not count as browser execution. It inspects safe correlation,
field presence and privacy booleans, never stores/logs raw token bodies or their
hashes. Native assertions read the actual completed/canceled originals, retained
account binding and single account. This is setup/transport acceptance, not Recipe
auth/re-execution, Extension Tool egress or installed multi-platform release proof.
