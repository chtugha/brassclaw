import assert from "node:assert/strict";
import test from "node:test";

import { deleteThread, listAutomations, requestManualTokenSetup, submitManualTokenSecret, continueManualToken, submitManualToken } from "./api.js";

test("listAutomations reads through the v2 automations route", async () => {
  const calls = [];
  globalThis.sessionStorage = {
    getItem: () => "token-1",
    setItem: () => {},
    removeItem: () => {},
  };
  globalThis.fetch = async (path, options) => {
    calls.push({ path, options });
    return new Response(JSON.stringify({ automations: [] }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };

  const response = await listAutomations({ limit: 50 });

  assert.deepEqual(response, { automations: [] });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].path, "/api/webchat/v2/automations?limit=50");
  assert.equal(calls[0].options.credentials, "same-origin");
  assert.equal(calls[0].options.headers.get("Authorization"), "Bearer token-1");
});

test("listAutomations propagates api errors from the automations route", async () => {
  globalThis.sessionStorage = {
    getItem: () => "",
    setItem: () => {},
    removeItem: () => {},
  };
  globalThis.fetch = async () =>
    new Response("temporarily unavailable", {
      status: 503,
      statusText: "Service Unavailable",
      headers: { "content-type": "text/plain" },
    });

  await assert.rejects(listAutomations({ limit: 50 }), (error) => {
    assert.equal(error.name, "ApiError");
    assert.equal(error.status, 503);
    assert.equal(error.statusText, "Service Unavailable");
    assert.equal(error.body, "temporarily unavailable");
    return true;
  });
});

test("deleteThread sends DELETE to the encoded thread route", async () => {
  const calls = [];
  globalThis.sessionStorage = {
    getItem: () => "token-1",
    setItem: () => {},
    removeItem: () => {},
  };
  globalThis.fetch = async (path, options) => {
    calls.push({ path, options });
    return new Response(
      JSON.stringify({ thread_id: "thread/needs encoding", deleted: true }),
      {
        status: 200,
        headers: { "content-type": "application/json" },
      }
    );
  };

  const response = await deleteThread({ threadId: "thread/needs encoding" });

  assert.deepEqual(response, {
    thread_id: "thread/needs encoding",
    deleted: true,
  });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].path, "/api/webchat/v2/threads/thread%2Fneeds%20encoding");
  assert.equal(calls[0].options.method, "DELETE");
  assert.equal(calls[0].options.credentials, "same-origin");
  assert.equal(calls[0].options.headers.get("Authorization"), "Bearer token-1");
});

test("deleteThread rejects before fetch when thread id is missing", async () => {
  let fetchCalled = false;
  globalThis.sessionStorage = {
    getItem: () => "token-1",
    setItem: () => {},
    removeItem: () => {},
  };
  globalThis.fetch = async () => {
    fetchCalled = true;
    throw new Error("fetch should not be called");
  };

  await assert.rejects(deleteThread(), /threadId is required/);

  assert.equal(fetchCalled, false);
});

test("manual setup/raw/reference use the same explicit original and keep continuation token-free", async () => {
  const calls = [];
  globalThis.sessionStorage = { getItem: () => "operator-token" };
  globalThis.fetch = async (path, options) => {
    calls.push({ path, options, body: JSON.parse(options.body) });
    return new Response(JSON.stringify({ acknowledged: true }), {
      status: 200, headers: { "content-type": "application/json" },
    });
  };
  const original = {
    identity: { interaction_id: "10000000-0000-4000-8000-000000000001", flow_id: "20000000-0000-4000-8000-000000000002" },
    invocationId: "30000000-0000-4000-8000-000000000003", threadId: "original-thread",
  };
  await requestManualTokenSetup({ ...original, provider: "test", accountLabel: "Test account", runId: "run", gateRef: "gate" });
  await submitManualTokenSecret({ ...original, token: "  private raw ü  " });
  await continueManualToken({ ...original, token: "must never be transmitted by reference" });
  assert.deepEqual(calls.map(call => call.path), [
    "/api/reborn/product-auth/manual-token/setup",
    "/api/reborn/product-auth/manual-token/secret-submit",
    "/api/reborn/product-auth/manual-token/continue",
  ]);
  for (const call of calls) {
    assert.deepEqual(call.body.identity, original.identity);
    assert.equal(call.body.invocation_id, original.invocationId);
    assert.equal(call.body.thread_id, original.threadId);
    assert.equal(call.options.headers.get("Authorization"), "Bearer operator-token");
    assert.equal(call.options.credentials, "same-origin");
  }
  assert.equal(calls[1].body.token, "  private raw ü  ");
  assert.equal(Object.hasOwn(calls[0].body, "token"), false);
  assert.equal(Object.hasOwn(calls[2].body, "token"), false);
});

test("every manual transport rejects a missing original before fetch", async () => {
  let calls = 0;
  globalThis.fetch = async () => { calls += 1; throw new Error("unexpected fetch"); };
  for (const port of [requestManualTokenSetup, submitManualTokenSecret, continueManualToken, submitManualToken]) {
    await assert.rejects(async () => port({ threadId: "thread", token: "private" }), /identity is required/);
  }
  assert.equal(calls, 0);
});

test("original setup-only transport preserves optional thread and absent continuation", async () => {
  let body;
  globalThis.sessionStorage = { getItem: () => "operator-token" };
  globalThis.fetch = async (_path, options) => {
    body = JSON.parse(options.body);
    return new Response(JSON.stringify({ acknowledged: true }), {
      status: 200, headers: { "content-type": "application/json" },
    });
  };
  await requestManualTokenSetup({
    identity: { interaction_id: "10000000-0000-4000-8000-000000000001", flow_id: "20000000-0000-4000-8000-000000000002" },
    invocationId: "30000000-0000-4000-8000-000000000003", provider: "test", accountLabel: "Settings setup",
  });
  assert.equal(Object.hasOwn(body, "thread_id"), false);
  assert.equal(Object.hasOwn(body, "run_id"), false);
  assert.equal(Object.hasOwn(body, "gate_ref"), false);
  assert.equal(Object.hasOwn(body, "token"), false);
});
