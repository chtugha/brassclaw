// Runs the actual login hook control flow with controlled transport, hooks and
// query refresh. React rendering/backend publication are separate native scopes.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
const source = readFileSync(new URL("./useProviderLogin.js", import.meta.url), "utf8")
  .replace(/import[\s\S]*?from "[^"]+";\n/g, "").replaceAll("export ", "");
const tick = () => new Promise(resolve => setImmediate(resolve));
function fixture() {
  const updates = [], opened = [], waits = [], refresh = []; let cleanup; let success = 0, next = 0;
  const context = { React: { useCallback: fn => fn, useRef: initial => ({ current: initial }),
    useEffect: effect => { cleanup = effect(); }, useState: initial => [initial, value => updates.push(value)] },
    useQueryClient: () => ({ invalidateQueries: () => new Promise(resolve => refresh.push(resolve)) }),
    useT: () => value => value, window: { location: { origin: "http://operator.local" }, open: (...args) => opened.push(args) },
    startNearaiLogin: async () => ({ auth_url: "http://issuer.local/authorize", mutation_id: `original-${++next}` }),
    waitForProviderLogin: async (...args) => { waits.push(args); return true; },
    fetchLlmProviders: () => { throw new Error("NEAR login must not poll the active provider name"); },
    startCodexLogin: () => { throw new Error("not this flow"); },
    setTimeout, Date };
  vm.createContext(context);
  vm.runInContext(source + "\nglobalThis.create = useProviderLogin;", context);
  const hook = context.create({ onSuccess: () => { success += 1; } });
  return { hook, updates, opened, waits, refresh, unmount: () => cleanup(), successes: () => success };
}
test("current NEAR login waits for its original receipt and completed query refresh", async () => {
  const f = fixture(), pending = f.hook.startNearai("github");
  await tick();
  assert.equal(f.waits[0][0], "original-1");
  assert.equal(f.waits[0][1], "nearai");
  assert.equal(f.successes(), 0);
  f.refresh[0](); await pending;
  assert.equal(f.successes(), 1);
  assert.equal(f.updates.at(-1), false);
});
test("unmount during query refresh cannot trigger stale success navigation", async () => {
  const f = fixture(), pending = f.hook.startNearai("github");
  await tick(); f.unmount(); f.refresh[0](); await pending;
  assert.equal(f.successes(), 0);
});
test("a newer login during query refresh fences the older completion", async () => {
  const f = fixture(), first = f.hook.startNearai("github");
  await tick(); const second = f.hook.startNearai("google");
  await tick();
  f.refresh[0](); await first;
  assert.equal(f.successes(), 0);
  f.refresh[1](); await second;
  assert.equal(f.successes(), 1);
  assert.deepEqual(f.waits.map(args => args[0]), ["original-1", "original-2"]);
});
