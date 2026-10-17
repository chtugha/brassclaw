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
