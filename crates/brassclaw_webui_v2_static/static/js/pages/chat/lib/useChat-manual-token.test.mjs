import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
import {
  retainManualTokenOperation, recordManualTokenSetup,
  validateManualTokenCompletion, closeManualTokenOperation,
} from "./manual-token-operation.js";

function source() {
  const original = readFileSync(new URL("../hooks/useChat.js", import.meta.url), "utf8");
  let importing = false;
  const lines = [];
  for (const line of original.split("\n")) {
    if (!importing && line.startsWith("import ")) {
      importing = !line.trimEnd().endsWith(";");
    } else if (importing) {
      importing = !line.trimEnd().endsWith(";");
    } else lines.push(line.replace("export function useChat", "function useChat"));
  }
  return `${lines.join("\n")}\nglobalThis.hook = useChat;`;
}
function storage() {
  const values = new Map();
  return { values, getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value) };
}
function deferred() {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
}
const gate = { kind: "auth_required", challengeKind: "manual_token", runId: "run-original", gateRef: "gate-original", provider: "provider-original", accountLabel: "Original account" };
const expiry = "2099-01-01T00:00:00Z";
const account = randomUUID();
function setupAck(request, overrides = {}) {
  return { identity: { ...request.identity }, invocation_id: request.invocationId,
    interaction_id: request.identity.interaction_id, provider: request.provider,
    label: request.accountLabel, expires_at: expiry, status: "awaiting_user", ...overrides };
}
function completedAck(request, overrides = {}) {
  return { identity: { ...request.identity }, invocation_id: request.invocationId,
    status: "configured", credential_ref: account,
    continuation: { type: "turn_gate_resume", turn_run_ref: gate.runId, gate_ref: gate.gateRef }, ...overrides };
}

// Minimal React lifecycle/control adapter, following the existing useChat tests.
// Requests are controlled at the product API ports; real HTTP/backend acceptance
// belongs to the integration tests, not these controller fixtures.
function controller({ saved = storage(), ports = {} } = {}) {
  let states = [], refs = [], effects = [], stateIndex, refIndex, effectIndex, pending = [];
  let eventPorts, currentThread = "thread-original", value;
  const calls = [], updates = [];
  const React = {
    useState(initial) {
      const index = stateIndex++;
      if (!(index in states)) states[index] = typeof initial === "function" ? initial() : initial;
      return [states[index], next => {
        states[index] = typeof next === "function" ? next(states[index]) : next;
        updates.push({ index, value: states[index] });
      }];
    },
    useRef(initial) {
      const index = refIndex++;
      return refs[index] ??= { current: initial };
    },
    useCallback: fn => fn,
    useEffect(fn, deps) {
      const index = effectIndex++, before = effects[index];
      if (!before || !deps || deps.some((dep, i) => !Object.is(dep, before.deps?.[i]))) {
        pending.push(() => { before?.cleanup?.(); effects[index] = { deps, cleanup: fn() }; });
      }
    },
  };
  const dispatch = async (name, request, fallback) => {
    calls.push({ name, request: { ...request, identity: request.identity && { ...request.identity } } });
    return ports[name] ? ports[name](request) : fallback?.(request);
  };
  const context = vm.createContext({
    AbortController, Date, Error, Map, Math, JSON, React, console, setTimeout, clearTimeout,
    setInterval, clearInterval,
    queryClient: { getQueryData: () => null, invalidateQueries() {} },
    useHistory: () => ({ messages: [], hasMore: false, nextCursor: null, isLoading: false, loadHistory() {}, setMessages() {} }),
    useChatEvents: events => { eventPorts = events; return () => {}; },
    useSSE: () => ({ status: "open" }),
    cancelRunRequest: request => dispatch("cancel", request),
    resolveGateRequest: request => dispatch("resolve", request),
    retainManualTokenOperation: (context, options) => retainManualTokenOperation(context, { ...options, storage: saved, actionId: randomUUID }),
    recordManualTokenSetup: (operation, context, response) => recordManualTokenSetup(operation, context, response, saved),
    validateManualTokenCompletion: (operation, context, response) => validateManualTokenCompletion(operation, context, response, saved),
    closeManualTokenOperation: operation => closeManualTokenOperation(operation, saved),
    requestManualTokenSetup: request => dispatch("setup", request, setupAck),
    submitManualTokenSecret: request => dispatch("raw", request, completedAck),
    continueManualToken: request => dispatch("reference", request, completedAck),
  });
  vm.runInContext(source(), context);
  function render(thread = currentThread) {
    currentThread = thread; stateIndex = refIndex = effectIndex = 0; pending = [];
    value = context.hook(thread); for (const effect of pending) effect(); return value;
  }
  render();
  function open(next = gate) {
    eventPorts.setActiveRun({ runId: next.runId, threadId: currentThread });
    eventPorts.setPendingGate({ ...next });
    render();
  }
  open();
  return { saved, calls, updates, render, open,
    get hook() { return value; },
    get currentGate() { return states[5]; },
    get processing() { return states[4]; },
    unmount() { for (const effect of effects) effect?.cleanup?.(); },
  };
}

test("manual controller retains original IDs before lost setup reply and across reload", async () => {
  const saved = storage();
  const first = controller({ saved, ports: { setup: request => {
    assert.equal(saved.values.size, 1);
    const retained = JSON.parse([...saved.values.values()][0]);
    assert.equal(retained.identity.flow_id, request.identity.flow_id);
    assert.equal(retained.invocationId, request.invocationId);
    throw new Error("deliberate unknown setup reply");
  } } });
  await assert.rejects(first.hook.submitAuthToken(" original token "));
  const second = controller({ saved });
  await second.hook.submitAuthToken(" original token ");
  assert.deepEqual(second.calls[0].request.identity, first.calls[0].request.identity);
  assert.equal(second.calls[0].request.invocationId, first.calls[0].request.invocationId);
  assert.equal(second.calls.find(c => c.name === "raw").request.token, " original token ");
  assert.equal(second.currentGate, null);
  for (const text of saved.values.values()) assert.equal(text.includes("original token"), false);
  first.unmount(); second.unmount();
});

test("raw always reaches raw proof; unknown completion has a separate token-free reference action", async () => {
  const raw = [];
  const app = controller({ ports: { raw: request => { raw.push(request.token); throw new Error("unconfirmed publication"); } } });
  await assert.rejects(app.hook.submitAuthToken("first raw"));
  await assert.rejects(app.hook.submitAuthToken("different raw"));
  assert.deepEqual(raw, ["first raw", "different raw"]);
  assert.equal(app.calls.some(c => c.name === "reference"), false);
  await app.hook.continueAuthToken();
  const reference = app.calls.find(c => c.name === "reference").request;
  assert.equal(Object.hasOwn(reference, "token"), false);
  const identities = app.calls.filter(c => c.name !== "cancel").map(c => JSON.stringify(c.request.identity));
  assert.equal(new Set(identities).size, 1);
  app.unmount();
});

test("reference without an original and storage failure send no request", async () => {
  const missing = controller();
  await assert.rejects(missing.hook.continueAuthToken());
  assert.equal(missing.calls.length, 0);
  const broken = controller({ saved: { getItem: () => null, setItem: () => { throw new Error("storage unavailable"); } } });
  await assert.rejects(broken.hook.submitAuthToken("private raw"));
  assert.equal(broken.calls.length, 0);
  missing.unmount(); broken.unmount();
});

test("changed gate definition cannot create a replacement original", async () => {
  const app = controller({ ports: { raw: () => { throw new Error("unknown result"); } } });
  await assert.rejects(app.hook.submitAuthToken("raw"));
  app.open({ ...gate, accountLabel: "Changed account" });
  const before = app.calls.length;
  await assert.rejects(app.hook.submitAuthToken("raw"));
  assert.equal(app.calls.length, before);
  assert.equal(app.saved.values.size, 1);
  app.unmount();
});

test("a different setup expiry or completion correlation cannot close the gate", async () => {
  let setups = 0;
  const app = controller({ ports: {
    setup: request => setupAck(request, { expires_at: ++setups === 1 ? expiry : "2099-01-02T00:00:00Z" }),
    raw: request => completedAck(request, { identity: { ...request.identity, flow_id: randomUUID() } }),
  } });
  await assert.rejects(app.hook.submitAuthToken("raw"));
  assert.ok(app.currentGate);
  await assert.rejects(app.hook.submitAuthToken("raw"));
  assert.equal(app.calls.filter(c => c.name === "raw").length, 1);
  assert.ok(app.currentGate);
  app.unmount();
});

for (const change of ["cancel", "gate-cancel", "thread", "definition", "unmount"]) {
  test(`late manual completion is fenced after ${change}`, async () => {
    const held = deferred(), entered = deferred();
    const app = controller({ ports: { raw: request => { entered.resolve(request); return held.promise; } } });
    const operation = app.hook.submitAuthToken("raw");
    const request = await entered.promise;
    if (change === "cancel") await app.hook.cancelRun("operator canceled");
    else if (change === "gate-cancel") await app.hook.resolveGate("cancelled");
    else if (change === "thread") app.render("another-thread");
    else if (change === "definition") app.open({ ...gate, provider: "different-provider" });
    else app.unmount();
    const before = app.updates.length;
    held.resolve(completedAck(request));
    await assert.rejects(operation, /gate changed/);
    assert.equal(app.updates.length, before, "late response cannot update current UI state");
    app.unmount();
  });
}

test("known terminal setup receipts never submit raw and retain the closed original", async () => {
  for (const status of ["canceled", "expired", "failed"]) {
    const app = controller({ ports: { setup: request => setupAck(request, { status }) } });
    await assert.rejects(app.hook.submitAuthToken("private raw"), failure => {
      assert.equal(failure.safeAuthGateCode, `manual_setup_${status}`);
      return true;
    });
    assert.deepEqual(app.calls.map(call => call.name), ["setup"]);
    const record = JSON.parse([...app.saved.values.values()][0]);
    assert.equal(record.closed, true);
    assert.equal(record.setup.status, status);
    app.unmount();
  }
});

test("canonical HTTP terminal errors are distinct from an unknown publication failure", async () => {
  const cases = [
    [404, "unknown_or_expired_flow", "manual_setup_unavailable"],
    [400, "canceled", "manual_setup_canceled"],
    [409, "flow_already_terminal", "manual_setup_terminal"],
    [503, "backend_unavailable", undefined],
  ];
  for (const [status, code, expected] of cases) {
    const app = controller({ ports: { setup: () => {
      const failure = new Error("deliberate canonical HTTP failure");
      failure.status = status; failure.payload = { code }; throw failure;
    } } });
    await assert.rejects(app.hook.submitAuthToken("raw"), failure => {
      assert.equal(failure.safeAuthGateCode, expected);
      return true;
    });
    assert.equal(app.calls.length, 1);
    assert.equal(JSON.parse([...app.saved.values.values()][0]).closed, false,
      "an error alone cannot claim confirmed cancellation/publication or erase the original");
    if (status === 503) {
      await assert.rejects(app.hook.submitAuthToken("raw"));
      assert.deepEqual(app.calls[1].request.identity, app.calls[0].request.identity);
      assert.equal(app.calls[1].request.invocationId, app.calls[0].request.invocationId);
    }
    app.unmount();
  }
});
