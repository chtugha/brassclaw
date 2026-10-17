import assert from "node:assert/strict";
import test from "node:test";

import {
  API_KEY_UNCHANGED,
  groupProvidersByStatus,
  providerAcceptsApiKey,
  providerStatus,
  providerPayload,
  providerModelDiscoveryPolicy,
  providerRequestPolicy,
  providerEmbeddingModels,
} from "./llm-providers.js";

test("embedding contracts retain exact names, reject ambiguity and preserve zero/off", () => {
  assert.equal(providerEmbeddingModels({}), undefined);
  assert.deepEqual(providerEmbeddingModels({ embeddingModels: [] }), {});
  const row = { model: "custom-model", dimension: "3", cacheMaxEntries: "0" };
  assert.deepEqual(providerEmbeddingModels({ embeddingModels: [row] }), { "custom-model": { dimension: 3, cache_max_entries: 0 } });
  for (const patch of [{ model: "" }, { model: " custom-model" }, { dimension: "0" }, { dimension: "1.5" }, { cacheMaxEntries: "" }, { cacheMaxEntries: "4294967296" }, { cacheMaxEntries: true }]) {
    assert.throws(() => providerEmbeddingModels({ embeddingModels: [{ ...row, ...patch }] }), RangeError);
  }
  assert.throws(() => providerEmbeddingModels({ embeddingModels: [row, row] }), RangeError);
  assert.deepEqual(providerEmbeddingModels({ embeddingModels: [{ ...row, vectorRevision: "2" }] }), { "custom-model": { dimension: 3, cache_max_entries: 0, vector_revision: 2 } });
  for (const vectorRevision of ["0", "", "1.5", "4294967296", null, false]) {
    assert.throws(() => providerEmbeddingModels({ embeddingModels: [{ ...row, vectorRevision }] }), RangeError);
  }
  const prototypeName = providerEmbeddingModels({ embeddingModels: [{ ...row, model: "__proto__" }] });
  assert.equal(Object.hasOwn(prototypeName, "__proto__"), true);
  assert.equal(Object.getPrototypeOf(prototypeName), Object.prototype);
});

// Helpers — minimal provider shapes that satisfy the configured/missing
// predicates without coupling the test to the full provider schema.
const builtinReady = (id, opts = {}) => ({
  id,
  name: id,
  builtin: true,
  adapter: "open_ai_completions",
  api_key_required: true,
  base_url_required: false,
  has_api_key: true,
  default_model: "model-x",
  ...opts,
});

const builtinNeedsKey = (id) => ({
  id,
  name: id,
  builtin: true,
  adapter: "anthropic",
  api_key_required: true,
  base_url_required: false,
  has_api_key: false,
  default_model: "claude",
});

const builtinNeedsBaseUrl = (id) => ({
  id,
  name: id,
  builtin: true,
  adapter: "open_ai_completions",
  api_key_required: false,
  base_url_required: true,
  has_api_key: true,
  default_model: "model-x",
});

const customReady = (id) => ({
  id,
  name: id,
  builtin: false,
  adapter: "open_ai_completions",
  base_url: "https://example.com/v1",
  api_key: API_KEY_UNCHANGED,
  default_model: "m",
});

test("providerStatus returns 'active' for the active id regardless of configuration", () => {
  const provider = builtinNeedsKey("anthropic");
  assert.equal(providerStatus(provider, {}, "anthropic"), "active");
});

test("providerStatus returns 'ready' for configured non-active providers", () => {
  assert.equal(providerStatus(builtinReady("nearai"), {}, "other"), "ready");
  assert.equal(providerStatus(customReady("vllm"), {}, "other"), "ready");
});

test("providerStatus returns 'setup' when an API key is missing", () => {
  assert.equal(providerStatus(builtinNeedsKey("anthropic"), {}, "nearai"), "setup");
});

test("providerStatus returns 'setup' when a required base URL is missing", () => {
  assert.equal(providerStatus(builtinNeedsBaseUrl("openai"), {}, "nearai"), "setup");
});

test("providerAcceptsApiKey supports dual-auth NEAR providers", () => {
  const nearai = builtinReady("nearai", {
    adapter: "nearai",
    api_key_required: false,
    accepts_api_key: true,
    has_api_key: false,
  });

  assert.equal(providerAcceptsApiKey(nearai), true);
});

test("providerAcceptsApiKey rejects missing provider input", () => {
  assert.equal(providerAcceptsApiKey(null), false);
  assert.equal(providerAcceptsApiKey(undefined), false);
});

test("providerAcceptsApiKey honors explicit false", () => {
  const loginOnly = builtinReady("openai_codex", {
    adapter: "openai_codex",
    api_key_required: false,
    accepts_api_key: false,
  });

  assert.equal(providerAcceptsApiKey(loginOnly), false);
});

test("groupProvidersByStatus buckets providers into active/ready/setup", () => {
  const providers = [
    builtinReady("nearai"),
    builtinNeedsKey("anthropic"),
    builtinReady("bedrock"),
    builtinNeedsKey("cerebras"),
    customReady("vllm"),
  ];

  const groups = groupProvidersByStatus(providers, {}, "nearai");

  assert.deepEqual(
    groups.active.map((p) => p.id),
    ["nearai"]
  );
  assert.deepEqual(
    groups.ready.map((p) => p.id),
    ["bedrock", "vllm"]
  );
  assert.deepEqual(
    groups.setup.map((p) => p.id),
    ["anthropic", "cerebras"]
  );
});

test("groupProvidersByStatus preserves upstream order inside each bucket", () => {
  const providers = [
    builtinReady("z-provider"),
    builtinReady("a-provider"),
    builtinNeedsKey("y-needs-key"),
    builtinNeedsKey("b-needs-key"),
  ];

  const groups = groupProvidersByStatus(providers, {}, "none");

  assert.deepEqual(
    groups.ready.map((p) => p.id),
    ["z-provider", "a-provider"],
    "ready bucket preserves input ordering (no implicit sort)"
  );
  assert.deepEqual(
    groups.setup.map((p) => p.id),
    ["y-needs-key", "b-needs-key"],
    "setup bucket preserves input ordering"
  );
});

test("groupProvidersByStatus honours builtin overrides when classifying", () => {
  // Provider declares it needs an API key but the override supplies a stored
  // sentinel value — should be considered ready, not in setup.
  const provider = builtinNeedsKey("anthropic");
  const overrides = { anthropic: { api_key: API_KEY_UNCHANGED } };

  const groups = groupProvidersByStatus([provider], overrides, "nearai");

  assert.deepEqual(groups.ready.map((p) => p.id), ["anthropic"]);
  assert.deepEqual(groups.setup.map((p) => p.id), []);
});

test("groupProvidersByStatus returns empty arrays for missing buckets, not undefined", () => {
  const groups = groupProvidersByStatus([], {}, null);
  assert.deepEqual(groups, { active: [], ready: [], setup: [] });
});

test("groupProvidersByStatus treats non-array input as empty", () => {
  const groups = groupProvidersByStatus(null, {}, null);
  assert.deepEqual(groups, { active: [], ready: [], setup: [] });
});


test("provider discovery payload preserves explicit endpoint and typed limits without fabricating a model or credential", () => {
  const form = { id: "local", adapter: "open_ai_completions", baseUrl: " http://127.0.0.1:8000/v1 ", model: "", discoveryTimeout: "9", discoveryBytes: "65536" };
  const payload = providerPayload(null, form, "", {});
  assert.deepEqual(JSON.parse(JSON.stringify(payload)), {
    provider_id: "local", provider_type: "custom", adapter: "open_ai_completions", base_url: "http://127.0.0.1:8000/v1",
    model_discovery: { timeout_secs: 9, max_response_bytes: 65536 },
  });
  for (const invalid of ["", "0", "-1", "1.5", "Infinity", Number.MAX_SAFE_INTEGER + 1, null, true]) {
    assert.throws(() => providerModelDiscoveryPolicy({ ...form, discoveryTimeout: invalid }), RangeError);
  }
  assert.throws(() => providerModelDiscoveryPolicy({ discoveryTimeout: "9" }), RangeError);
  assert.equal(providerModelDiscoveryPolicy({}), undefined);
});


test("request-policy form encodes disabled circuits and rejects invalid/partial limits", () => {
  const form = { requestTimeout: "3", requestRetries: "0", circuitThreshold: "0", circuitRecovery: "5" };
  assert.deepEqual(providerRequestPolicy(form), { timeout_secs: 3, max_retries: 0, circuit_breaker_threshold: null, circuit_breaker_recovery_secs: 5 });
  assert.equal(providerRequestPolicy({}), undefined);
  for (const patch of [{requestTimeout: ""}, {requestTimeout: "0"}, {requestRetries: "-1"}, {requestRetries: "4294967296"}, {circuitThreshold: "4294967296"}, {circuitRecovery: "0"}, {circuitRecovery: "1.5"}, {requestTimeout: null}]) {
    assert.throws(() => providerRequestPolicy({...form, ...patch}), RangeError);
  }
  assert.throws(() => providerRequestPolicy({requestTimeout: "3"}), RangeError);
});


test("retry timing carries explicit ceilings and rejects incomplete or inconsistent form data", () => {
  const form = { requestTimeout: "3", requestRetries: "2", circuitThreshold: "0", circuitRecovery: "5", retryBase: "5", retryMax: "10", retryJitter: "0", retryAfterMax: "7200" };
  assert.deepEqual(providerRequestPolicy(form).retry_timing, { base_delay_ms: 5, max_backoff_ms: 10, jitter_percent: 0, max_retry_after_secs: 7200 });
  for (const patch of [{ retryBase: "0" }, { retryMax: "4" }, { retryJitter: "101" }, { retryJitter: "-1" }, { retryAfterMax: "" }, { retryAfterMax: "0" }, { retryMax: undefined }]) {
    assert.throws(() => providerRequestPolicy({ ...form, ...patch }), RangeError);
  }
});


test("response capacity remains optional, validates finite byte values and rejects unsupported adapters", () => {
  const form = { adapter: "open_ai_completions", requestTimeout: "3", requestRetries: "2", circuitThreshold: "0", circuitRecovery: "5" };
  assert.equal(providerRequestPolicy(form).max_response_bytes, undefined);
  assert.equal(providerRequestPolicy({ ...form, responseBytes: "0" }).max_response_bytes, null);
  assert.equal(providerRequestPolicy({ ...form, responseBytes: "65536" }).max_response_bytes, 65536);
  for (const value of ["", "-1", "1.5", "Infinity", null, true, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => providerRequestPolicy({ ...form, responseBytes: value }), RangeError);
  }
  assert.throws(() => providerRequestPolicy({ ...form, adapter: "bedrock", responseBytes: "1" }), RangeError);
  assert.equal(providerRequestPolicy({ ...form, adapter: "bedrock", responseBytes: "0" }).max_response_bytes, null);
});
