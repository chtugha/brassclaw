import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function useExtensionsSourceForTest() {
  const source = readFileSync(new URL("./useExtensions.js", import.meta.url), "utf8");
  const lines = [];
  let skippingImport = false;
  for (const line of source.split("\n")) {
    if (!skippingImport && line.startsWith("import ")) {
      skippingImport = !line.trimEnd().endsWith(";");
      continue;
    }
    if (skippingImport) {
      skippingImport = !line.trimEnd().endsWith(";");
      continue;
    }
    lines.push(line.replace(/^export function /, "function "));
  }
  return `${lines.join("\n")}\nglobalThis.__testExports = { usePairing, useOauthSetupProgress };`;
}

function contextFor(mutationState, queryCalls) {
  return {
    React: { useCallback: (fn) => fn, useEffect: () => {}, useRef: () => ({ current: null }), useState: () => [null, () => {}] },
    activateExtension: () => {},
    approvePairingCode: () => {},
    fetchExtensionRegistry: () => {},
    fetchExtensionSetup: () => {},
    fetchExtensions: () => {},
    fetchPairingRequests: () => {},
    gatewayStatus: () => {},
    globalThis: {},
    installExtension: () => {},
    removeExtension: () => {},
    startExtensionOauth: () => {},
    submitExtensionSetup: () => {},
    useMutation: () => mutationState,
    useQuery: (config) => {
      queryCalls.push(config);
      return { data: { requests: [] }, isLoading: false };
    },
    useQueryClient: () => ({ invalidateQueries: () => {} }),
  };
}

test("usePairing only exposes result on success and error on failure", () => {
  for (const [name, mutationState, expected] of [
    ["idle", { mutate: () => {}, isPending: false, isSuccess: false, isError: false }, { result: null, error: null }],
    ["success", { mutate: () => {}, isPending: false, isSuccess: true, isError: false, data: { success: true } }, { result: { success: true }, error: null }],
    ["error", { mutate: () => {}, isPending: false, isSuccess: false, isError: true, error: new Error("bad") }, { result: null, errorMessage: "bad" }],
  ]) {
    const queryCalls = [];
    const context = contextFor(mutationState, queryCalls);
    vm.runInNewContext(useExtensionsSourceForTest(), context);

    const pairing = context.globalThis.__testExports.usePairing("slack");

    assert.deepEqual(pairing.result, expected.result, name);
    assert.equal(pairing.error?.message || null, expected.errorMessage || null, name);
  }
});

test("usePairing can disable the legacy pairing request query for custom redeemers", () => {
  const queryCalls = [];
  const context = contextFor(
    { mutate: () => {}, isPending: false, isSuccess: false, isError: false },
    queryCalls
  );
  vm.runInNewContext(useExtensionsSourceForTest(), context);

  context.globalThis.__testExports.usePairing("slack", { enabled: false });

  assert.equal(queryCalls.length, 1);
  assert.equal(queryCalls[0].enabled, false);
});

// Pure observation control: no issuer result, Auth record or completed setup is
// invented. The native browser separately exercises the actual HTTP/PG owner.
function observationHarness() {
  let now = Date.parse("2026-10-31T12:00:00Z"), next = 1;
  const timers = new Map(), invalidations = [], cleanups = [];
  class ObservationClock extends Date { static now() { return now; } }
  const context = contextFor({}, []);
  context.Date = ObservationClock;
  context.window = {
    setInterval(fn, interval) { const id = next++; timers.set(id, { fn, interval }); return id; },
    clearInterval(id) { timers.delete(id); },
  };
  context.React.useEffect = callback => { cleanups.push(callback()); };
  context.useQueryClient = () => ({
    invalidateQueries({ queryKey }) { invalidations.push([...queryKey]); },
    getQueryData() { return undefined; },
  });
  vm.runInNewContext(useExtensionsSourceForTest(), context);
  const observer = context.globalThis.__testExports.useOauthSetupProgress({ id: "original-observation" });
  return { observer, timers, invalidations,
    advance: elapsed => { now += elapsed; },
    expiry: delay => new Date(now + delay).toISOString(),
    tick: () => { for (const { fn } of [...timers.values()]) fn(); },
    unmount: () => cleanups.forEach(cleanup => cleanup?.()),
  };
}

test("OAuth observation keeps the confirmed deadline across a reopened or blocked popup", () => {
  const view = observationHarness(), expiry = view.expiry(20 * 60 * 1000);
  view.observer.watchOauthProgress(null, expiry);
  assert.equal(view.timers.size, 1, "a blocked popup still observes its recovery link");
  view.advance(10 * 60 * 1000 + 1);
  view.tick();
  assert.equal(view.timers.size, 1, "ten minutes is not a separate lifetime");
  view.observer.watchOauthProgress({ closed: false }, expiry);
  assert.equal(view.timers.size, 1, "reopening replaces the observer without duplicating it");
  view.advance(10 * 60 * 1000);
  view.tick();
  assert.equal(view.timers.size, 0, "reopening cannot extend the original deadline");
  assert.deepEqual(view.invalidations.at(-1), ["extension-setup", "original-observation"]);
});

test("expired or malformed OAuth deadlines never create another observation window", () => {
  const view = observationHarness();
  view.observer.watchOauthProgress(null, view.expiry(-1));
  assert.equal(view.timers.size, 0);
  assert.equal(view.invalidations.length, 3, "one final read-only refresh");
  for (const invalid of [undefined, null, 0, {}, "invalid"]) {
    view.observer.watchOauthProgress(null, view.expiry(1000));
    assert.throws(() => view.observer.watchOauthProgress(null, invalid), /expiry is invalid/);
    assert.equal(view.timers.size, 0, "invalid replacement stops the prior observer");
  }
  assert.equal(view.invalidations.length, 3, "invalid metadata never becomes readiness");
});

test("popup close and unmount clean up OAuth observers", () => {
  const view = observationHarness(), popup = { closed: false };
  view.observer.watchOauthProgress(popup, view.expiry(10000));
  popup.closed = true;
  view.tick();
  assert.equal(view.timers.size, 0);
  view.observer.watchOauthProgress(null, view.expiry(10000));
  assert.equal(view.timers.size, 1);
  view.unmount();
  assert.equal(view.timers.size, 0);
});
