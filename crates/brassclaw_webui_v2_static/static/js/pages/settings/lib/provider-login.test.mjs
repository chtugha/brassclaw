import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
const source = readFileSync(new URL("./provider-login.js", import.meta.url), "utf8")
  .split("\n").filter(line => !line.startsWith("import ")).join("\n").replaceAll("export ", "");
const id = "00000000-0000-4000-8000-000000000001";
const receipt = () => ({ mutation_id: id, operation: "upsert", provider_id: "nearai" });
function rejected(status) { const error = new Error("controlled request failure"); error.status = status; return error; }
function fixture(read = async () => receipt(), reconcile = async () => ({ mutation_receipt: receipt() })) {
  const calls = []; let now = 0;
  const context = { Date: { now: () => now }, setTimeout: (resolve, ms) => { now += ms; resolve(); },
    readProviderMutation: async value => { calls.push(["read", value]); return read(value); },
    reconcileProviderMutation: async value => { calls.push(["reconcile", value]); return reconcile(value); } };
  vm.createContext(context);
  vm.runInContext(source + "\nglobalThis.wait = waitForProviderLogin;", context);
  return { calls, wait: (isCurrent) => context.wait(id, "nearai", 5000, 2000, isCurrent), raw: context.wait };
}
test("an already active provider cannot acknowledge a new login; only its original receipt can", async () => {
  let attempt = 0;
  const f = fixture(async () => { if (++attempt === 1) throw rejected(404); return receipt(); });
  assert.equal(await f.wait(), true);
  assert.deepEqual(f.calls, [["read", id], ["read", id], ["reconcile", id]]);
});
test("foreign identity, operation and provider never reach reconciliation", async () => {
  for (const changed of [{ mutation_id: "foreign" }, { operation: "select" }, { provider_id: "foreign" }]) {
    const f = fixture(async () => ({ ...receipt(), ...changed }));
    await assert.rejects(f.wait(), /acknowledgement differs/);
    assert.deepEqual(f.calls, [["read", id]]);
  }
});
test("transport and confirmed-original reconciliation failures remain errors", async () => {
  const unavailable = fixture(async () => { throw rejected(503); });
  await assert.rejects(unavailable.wait(), error => error.status === 503);
  for (const status of [404, 503]) {
    const f = fixture(undefined, async () => { throw rejected(status); });
    await assert.rejects(f.wait(), error => error.status === status);
    assert.deepEqual(f.calls, [["read", id], ["reconcile", id]]);
  }
});
test("late read after unmount or a newer login does not reconcile or succeed", async () => {
  let resolve; let current = true;
  const f = fixture(() => new Promise(done => { resolve = done; }));
  const waiting = f.wait(() => current);
  current = false; resolve(receipt());
  assert.equal(await waiting, false);
  assert.deepEqual(f.calls, [["read", id]]);
});
test("an absent original expires without any replacement mutation or reconciliation", async () => {
  const f = fixture(async () => { throw rejected(404); });
  assert.equal(await f.wait(), false);
  assert.deepEqual(f.calls, [["read", id], ["read", id], ["read", id]]);
});
test("missing login identity fails before transport; foreign reconciliation cannot succeed", async () => {
  const missing = fixture();
  await assert.rejects(missing.raw(undefined, "nearai", 5000, 2000), /identity is unavailable/);
  assert.equal(missing.calls.length, 0);
  const foreign = fixture(undefined, async () => ({ mutation_receipt: { ...receipt(), mutation_id: "foreign" } }));
  await assert.rejects(foreign.wait(), /reconciliation differs/);
});
