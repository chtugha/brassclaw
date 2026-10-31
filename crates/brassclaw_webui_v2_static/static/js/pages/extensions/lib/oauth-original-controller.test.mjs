import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function source(path) {
  let importing = false;
  return readFileSync(new URL(path, import.meta.url), "utf8").split("\n").flatMap(line => {
    if (!importing && line.startsWith("import ")) { importing = !line.trimEnd().endsWith(";"); return []; }
    if (importing) { importing = !line.trimEnd().endsWith(";"); return []; }
    return [line.replace(/^export /, "")];
  }).join("\n");
}
function store() {
  const values = new Map();
  return { values, getItem: key => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
}
const defaultContext = { packageId: "notion", requirement: "oauth-account", provider: "notion",
  accountLabel: "notion account", scopes: ["read"], invocationId: randomUUID() };
function reply(body, overrides = {}) {
  return { flow_id: body.flow_id, provider: body.provider, account_label: body.account_label,
    scopes: body.scopes, callback_scope: { invocation_id: body.invocation_id },
    expires_at: body.expires_at, continuation: { type: "setup_only" }, status: "awaiting_user",
    terminal: false, expired: false, error: null, cleanup: null,
    authorization_url: "https://issuer.example/authorize?state=PRIVATE_OAUTH_STATE", ...overrides };
}
function harness({ saved = store(), respond = body => reply(body), popupBlocked = false, context = defaultContext,
  policy = async () => ({ execution_limits: { effective_revision: 1, limits: { oauth_setup_default_ttl_secs: 600, oauth_setup_max_ttl_secs: 600 } } }) } = {}) {
  const slots = [], pendingEffects = [], calls = [], popups = [], events = [], watched = [];
  let cursor = 0, dirty = false, refreshes = 0, handlers = new Map(), tree;
  const React = {
    useState(initial) {
      const index = cursor++;
      slots[index] ??= { value: initial };
      return [slots[index].value, value => { slots[index].value = value; dirty = true; }];
    },
    useRef(initial) { const index = cursor++; slots[index] ??= { current: initial }; return slots[index]; },
    useEffect(callback, dependencies) {
      const index = cursor++, old = slots[index];
      if (!old || dependencies.some((value, i) => old.dependencies[i] !== value)) {
        pendingEffects.push(() => { old?.cleanup?.(); slots[index] = { dependencies, cleanup: callback() }; });
      }
    },
  };
  const Button = () => {};
  const html = (strings, ...values) => ({ strings: [...strings], values });
  const sandbox = vm.createContext({ React, html, Button, URL, clientActionId: randomUUID, sessionStorage: saved,
    fetchMontyVmStatus: async () => { events.push("policy"); return policy(); },
    useT: () => key => key,
    useOauthSetupProgress: () => ({ refreshSetupState: () => { refreshes++; },
      watchOauthProgress: (popup, expiry) => { events.push("watch"); watched.push({ popup, expiry }); }, clearWatcher: () => events.push("clear") }),
    window: { open(url) {
      events.push("open");
      if (popupBlocked) return null;
      const popup = { closed: false, location: { href: url }, opener: {}, close() { this.closed = true; } };
      popups.push(popup); return popup;
    } },
    apiFetch: async (path, options) => {
      const body = JSON.parse(options.body);
      const retained = [...(saved.values?.values() || [])].map(value => JSON.parse(value));
      assert.ok(retained.some(value => value.flowId === body.flow_id && value.expiresAt === body.expires_at),
        "the exact original must be durably readable before every Auth submission");
      events.push("http"); calls.push({ path, body });
      return respond(body);
    },
  });
  vm.runInContext(source("../../../lib/original-custody.js"), sandbox);
  vm.runInContext(source("./oauth-original.js"), sandbox);
  vm.runInContext(source("./extensions-api.js"), sandbox);
  vm.runInContext(source("../components/oauth-original-setup.js"), sandbox);
  vm.runInContext("globalThis.renderComponent=OAuthOriginalSetup; globalThis.sendOriginal=sendExtensionOauthOriginal;", sandbox);
  const props = { packageRef: { id: context.packageId }, secret: { name: context.requirement, provider: context.provider,
    setup: { account_label: context.accountLabel, scopes: context.scopes, invocation_id: context.invocationId } } };
  function collect(node) {
    if (!node || typeof node !== "object") return;
    node.values?.forEach((value, index) => {
      if (typeof value === "function" && node.strings[index]?.endsWith("onClick=")) {
        // Each button's label follows its own callback. A preceding help text
        // in the same template is not the label of the inline Check button.
        const label = node.values.slice(index + 1).find(item => typeof item === "string" && item.startsWith("extensions.oauth."));
        if (label) handlers.set(label, value);
      }
      collect(value);
    });
  }
  function render() {
    let attempts = 0;
    do {
      assert.ok(attempts++ < 20, "test render must settle");
      dirty = false; cursor = 0; tree = sandbox.renderComponent(props);
      while (pendingEffects.length) pendingEffects.shift()();
    } while (dirty);
    handlers = new Map(); collect(tree); return tree;
  }
  render();
  return { calls, saved, popups, events, watched, render, props,
    click: label => { render(); const action = handlers.get(label); assert.ok(action, `missing ${label}`); return action(); },
    handler: label => handlers.get(label),
    unmount: () => slots.forEach(slot => slot?.cleanup?.()),
    refreshes: () => refreshes,
    text: () => JSON.stringify(tree, (_key, value) => typeof value === "function" ? "function" : value),
    send: sandbox.sendOriginal,
  };
}

test("controller writes the original before transport and rejects duplicate clicks", async () => {
  let complete;
  const ui = harness({ respond: body => new Promise(resolve => { complete = () => resolve(reply(body)); }) });
  const action = ui.handler("extensions.oauth.start");
  const first = action(); await action();
  await new Promise(setImmediate);
  assert.equal(ui.calls.length, 1);
  assert.deepEqual(ui.events.slice(0, 3), ["open", "policy", "http"]);
  complete(); await first; ui.render();
  assert.equal(ui.popups[0].location.href, "https://issuer.example/authorize?state=PRIVATE_OAUTH_STATE");
  assert.ok(ui.text().includes("extensions.oauth.open"));
  assert.equal(JSON.stringify([...ui.saved.values]).includes("PRIVATE_OAUTH_STATE"), false);
});

test("terminal cleanup is visible and explicit reconciliation keeps the exact original without a popup", async () => {
  let reconciled=false;
  const ui=harness({respond:body=>body.action==="start" ? reply(body) : reply(body,{
    status:"canceled",terminal:true,authorization_url:null,
    cleanup:reconciled ? {remote:"confirmed",local:"confirmed",issue:null,http_status:204,error:null,can_reconcile:false} :
      {remote:"confirmed",local:"unresolved",issue:null,http_status:204,error:null,can_reconcile:true},
  })});
  await ui.click("extensions.oauth.start");
  const original=ui.calls[0].body;
  await ui.click("extensions.oauth.cancel");ui.render();
  assert.ok(ui.text().includes("extensions.oauth.local.unresolved"));
  const windows=ui.popups.length;
  reconciled=true;
  await ui.click("extensions.oauth.reconcile");ui.render();
  assert.deepEqual(ui.calls.at(-1).body,{...original,action:"cancel"});
  assert.equal(ui.popups.length,windows);
  assert.ok(ui.text().includes("extensions.oauth.local.confirmed"));
  assert.equal(ui.handler("extensions.oauth.reconcile"),undefined);
  assert.equal(JSON.stringify([...ui.saved.values]).includes("PRIVATE_OAUTH_STATE"),false);
});

test("lost acknowledgement and reload repeat the same original without renewing expiry", async () => {
  const saved = store();
  const first = harness({ saved, respond: () => Promise.reject(new Error("lost acknowledgement")) });
  await first.click("extensions.oauth.start");
  const original = first.calls[0].body;
  first.unmount();
  const second = harness({ saved });
  await second.click("extensions.oauth.start");
  assert.deepEqual(second.calls[0].body, original);
  assert.notEqual(second.popups[0].location.href, "about:blank");
});

test("unknown receipt offers no new original; cancel and explicit renewal first recover the old one", async () => {
  let mode = "unknown";
  const ui = harness({ respond: body => mode === "unknown"
    ? reply(body, { status: "pending", authorization_url: null,
      error: { code: "backend_unavailable", retryable: true } })
    : body.action === "cancel" || body.action === "recover"
      ? reply(body, { status: "canceled", terminal: true, authorization_url: null }) : reply(body) });
  await ui.click("extensions.oauth.start"); ui.render();
  assert.equal(ui.handler("extensions.oauth.new"), undefined);
  assert.ok(ui.text().includes("extensions.oauth.unknown"));
  const original = ui.calls[0].body;
  mode = "known";
  await ui.click("extensions.oauth.cancel");
  assert.equal(ui.calls[1].body.flow_id, original.flow_id);
  await ui.click("extensions.oauth.new");
  assert.deepEqual(ui.calls.slice(2).map(call => call.body.action), ["recover", "start"]);
  assert.equal(ui.calls[2].body.flow_id, original.flow_id);
  assert.notEqual(ui.calls[3].body.flow_id, original.flow_id);
  assert.equal(ui.calls[3].body.invocation_id, original.invocation_id);
  assert.equal(ui.popups.at(-1).location.href, "https://issuer.example/authorize?state=PRIVATE_OAUTH_STATE");
});

test("unmounted and changed-context replies never navigate or refresh another card", async () => {
  for (const changeContext of [false,true]) {
    let complete;
    const ui=harness({respond:body=>new Promise(resolve=>{complete=()=>resolve(reply(body));})});
    const pending=ui.click("extensions.oauth.start");
    await new Promise(setImmediate);
    if (changeContext) {ui.props.secret.setup.invocation_id=randomUUID();ui.render();}
    else ui.unmount();
    complete();await pending;
    assert.equal(ui.popups[0].closed,true);
    assert.equal(ui.popups[0].location.href,"about:blank");
    assert.equal(ui.refreshes(),0);
    assert.equal(JSON.stringify([...ui.saved.values]).includes("PRIVATE_OAUTH_STATE"),false);
  }
});

test("a blocked popup still leaves an explicit validated provider link without persisting it", async () => {
  const ui=harness({popupBlocked:true});
  await ui.click("extensions.oauth.start");ui.render();
  assert.equal(ui.calls.length,1);
  assert.ok(ui.text().includes("extensions.oauth.open"));
  assert.ok(ui.text().includes("https://issuer.example/authorize"));
  assert.equal(JSON.stringify([...ui.saved.values]).includes("PRIVATE_OAUTH_STATE"),false);
});

test("failed browser persistence blocks Auth submission and closes the blank placeholder", async () => {
  const ui = harness({ saved: { getItem: () => null, setItem() {} } });
  await ui.click("extensions.oauth.start"); ui.render();
  assert.equal(ui.calls.length, 0);
  assert.equal(ui.popups.length, 1);
  assert.equal(ui.popups[0].closed, true);
  assert.equal(ui.popups[0].location.href, "about:blank");
  assert.ok(ui.text().includes("extensions.oauth.unknown"));
});

test("the actual API adapter rejects a different package before transport", () => {
  const ui = harness();
  assert.throws(() => ui.send({ id: "different" }, { definition: { packageId: "notion" } }, "start"));
  assert.equal(ui.calls.length, 0);
});


test("Google uses the retained original controller for start, reload, recover and cancel", async () => {
  const context = { ...defaultContext, packageId: "google-drive", provider: "google",
    scopes: ["https://www.googleapis.com/auth/drive"], invocationId: randomUUID() };
  const respond = body => reply(body, body.action === "cancel"
    ? { status: "canceled", terminal: true, authorization_url: null } : {});
  const first = harness({ context, respond });
  await first.click("extensions.oauth.start");
  const original = first.calls[0].body;
  assert.match(first.calls[0].path, /extensions\/google-drive\/setup\/oauth\/start$/);
  assert.equal(original.provider, "google");
  first.unmount();
  const resumed = harness({ context, respond, saved: first.saved });
  await resumed.click("extensions.oauth.check");
  await resumed.click("extensions.oauth.cancel");
  assert.deepEqual(resumed.calls.map(call => call.body.action), ["recover", "cancel"]);
  for (const { body } of resumed.calls) {
    assert.equal(body.flow_id, original.flow_id);
    assert.equal(body.invocation_id, original.invocation_id);
    assert.equal(body.expires_at, original.expires_at);
  }
  assert.equal(JSON.stringify([...resumed.saved.values]).includes("PRIVATE_OAUTH_STATE"), false);
});


test("controller keeps the original explicit account after displayed readiness changes", async () => {
  const ui = harness();
  const selected = randomUUID(); ui.props.secret.credential_ref = selected;
  await ui.click("extensions.oauth.start");
  ui.props.secret.credential_ref = randomUUID();
  await ui.click("extensions.oauth.check");
  assert.deepEqual(ui.calls.map(call => call.body.target), [
    { kind: "existing", account_id: selected }, { kind: "existing", account_id: selected },
  ]);
  assert.equal(ui.calls[0].body.requirement, ui.props.secret.name);
});

test("a blocked popup observes the exact confirmed original receipt expiry", async () => {
  const ui = harness({ popupBlocked: true });
  await ui.click("extensions.oauth.start");
  assert.equal(ui.watched.length, 1);
  assert.equal(ui.watched[0].popup, null);
  assert.equal(ui.watched[0].expiry, ui.calls[0].body.expires_at);
  assert.equal(ui.popups.length, 0);
});

test("new originals use the acknowledged default while retained recovery needs no policy read", async () => {
  let ttl = 47, policyReads = 0;
  const policy = async () => { policyReads++; return { execution_limits: { effective_revision: 5,
    limits: { oauth_setup_default_ttl_secs: ttl, oauth_setup_max_ttl_secs: 900 } } }; };
  const first = harness({ policy });
  const before = Date.now(); await first.click("extensions.oauth.start");
  const original = first.calls[0].body;
  assert(Date.parse(original.expires_at) >= before + 47000);
  assert(Date.parse(original.expires_at) <= Date.now() + 47000);
  ttl = 1;
  const resumed = harness({ saved: first.saved, policy: async () => { throw Error("no policy needed for original"); } });
  await resumed.click("extensions.oauth.start");
  assert.deepEqual(resumed.calls[0].body, original);
  assert.equal(resumed.events.includes("policy"), false);
  assert.equal(policyReads, 1);
});

test("missing or incoherent policy fails before a new original and Auth submission", async () => {
  for (const value of [null, {}, { execution_limits: { effective_revision: 0, limits: { oauth_setup_default_ttl_secs: 1, oauth_setup_max_ttl_secs: 1 } } },
    ...[0, -1, 1.5, "600", 4294967296, null].map(ttl => ({ execution_limits: { effective_revision: 1, limits: { oauth_setup_default_ttl_secs: ttl, oauth_setup_max_ttl_secs: 600 } } })),
    { execution_limits: { effective_revision: 1, limits: { oauth_setup_default_ttl_secs: 601, oauth_setup_max_ttl_secs: 600 } } }]) {
    const ui = harness({ policy: async () => value });
    await ui.click("extensions.oauth.start"); ui.render();
    assert.equal(ui.calls.length, 0);
    assert.equal(ui.saved.values.size, 0);
    assert.equal(ui.popups[0].closed, true);
    assert(ui.text().includes("extensions.oauth.unknown"));
  }
});

test("policy read failure and context changes leave no replacement original", async () => {
  const failed = harness({ policy: async () => { throw Error("owner unavailable"); } });
  await failed.click("extensions.oauth.start");
  assert.equal(failed.calls.length, 0); assert.equal(failed.saved.values.size, 0);
  assert.equal(failed.popups[0].closed, true);
  let finish;
  const ui = harness({ policy: () => new Promise(resolve => { finish = resolve; }) });
  const pending = ui.click("extensions.oauth.start");
  assert.equal(ui.popups.length, 1, "placeholder precedes policy await");
  ui.props.secret.setup.invocation_id = randomUUID(); ui.render();
  finish({ execution_limits: { effective_revision: 1, limits: { oauth_setup_default_ttl_secs: 47, oauth_setup_max_ttl_secs: 600 } } });
  await pending;
  assert.equal(ui.calls.length, 0); assert.equal(ui.saved.values.size, 0);
  assert.equal(ui.popups[0].closed, true);
});

test("explicit renewal reads a new default only after confirmed terminal recovery", async () => {
  let ttl = 600;
  const ui = harness({ policy: async () => ({ execution_limits: { effective_revision: 3,
    limits: { oauth_setup_default_ttl_secs: ttl, oauth_setup_max_ttl_secs: 600 } } }),
    respond: body => body.action === "start" ? reply(body) : reply(body, { status: "canceled", terminal: true, authorization_url: null }) });
  await ui.click("extensions.oauth.start");
  const original = ui.calls[0].body;
  await ui.click("extensions.oauth.cancel");
  ttl = 37; const before = Date.now(), start = ui.events.length;
  await ui.click("extensions.oauth.new");
  const successor = ui.calls.at(-1).body;
  assert.notEqual(successor.flow_id, original.flow_id);
  assert(Date.parse(successor.expires_at) >= before + 37000);
  assert(Date.parse(successor.expires_at) <= Date.now() + 37000);
  assert.deepEqual(ui.events.slice(start, start + 4), ["open", "http", "policy", "http"]);
  assert.equal(ui.calls.at(-2).body.expires_at, original.expires_at);
});

test("a rejected first admission retries exact expiry without policy rebase", async () => {
  let reads = 0;
  const ui = harness({ policy: async () => { reads++; return { execution_limits: { effective_revision: 7,
    limits: { oauth_setup_default_ttl_secs: 47, oauth_setup_max_ttl_secs: 600 } } }; },
    respond: () => { throw Error("maximum changed before admission"); } });
  await ui.click("extensions.oauth.start"); const first = ui.calls[0].body;
  await ui.click("extensions.oauth.start");
  assert.deepEqual(ui.calls[1].body, first);
  assert.equal(reads, 1);
});

test("first setup retains the account chosen before an asynchronous policy read", async () => {
  let finish;
  const ui = harness({ policy: () => new Promise(resolve => { finish = resolve; }) });
  const account = randomUUID();
  ui.props.secret.credential_ref = account; ui.render();
  const pending = ui.click("extensions.oauth.start");
  ui.props.secret.credential_ref = randomUUID(); ui.render();
  finish({ execution_limits: { effective_revision: 1,
    limits: { oauth_setup_default_ttl_secs: 47, oauth_setup_max_ttl_secs: 600 } } });
  await pending;
  assert.deepEqual(ui.calls[0].body.target, { kind: "existing", account_id: account });
});

test("explicit renewal retains the account chosen before recovery and policy awaits", async () => {
  let hold = false, finish;
  const ui = harness({
    policy: () => hold ? new Promise(resolve => { finish = resolve; }) : Promise.resolve({
      execution_limits: { effective_revision: 1,
        limits: { oauth_setup_default_ttl_secs: 600, oauth_setup_max_ttl_secs: 600 } } }),
    respond: body => body.action === "start" ? reply(body) : reply(body, {
      status: "canceled", terminal: true, authorization_url: null,
    }),
  });
  await ui.click("extensions.oauth.start");
  await ui.click("extensions.oauth.cancel");
  const account = randomUUID();
  ui.props.secret.credential_ref = account; ui.render(); hold = true;
  const pending = ui.click("extensions.oauth.new");
  await new Promise(setImmediate);
  ui.props.secret.credential_ref = randomUUID(); ui.render();
  finish({ execution_limits: { effective_revision: 2,
    limits: { oauth_setup_default_ttl_secs: 47, oauth_setup_max_ttl_secs: 600 } } });
  await pending;
  assert.deepEqual(ui.calls.at(-1).body.target, { kind: "existing", account_id: account });
});
