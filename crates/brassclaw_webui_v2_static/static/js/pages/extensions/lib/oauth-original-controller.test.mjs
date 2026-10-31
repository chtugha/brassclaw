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
function harness({ saved = store(), respond = body => reply(body), popupBlocked = false, context = defaultContext } = {}) {
  const slots = [], pendingEffects = [], calls = [], popups = [], events = [];
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
    useT: () => key => key,
    useOauthSetupProgress: () => ({ refreshSetupState: () => { refreshes++; },
      watchOauthProgress: () => events.push("watch"), clearWatcher: () => events.push("clear") }),
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
        "the exact original must be durably readable before every network call");
      events.push("http"); calls.push({ path, body });
      return respond(body);
    },
  });
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
  return { calls, saved, popups, events, render, props,
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
  assert.equal(ui.calls.length, 1);
  assert.deepEqual(ui.events.slice(0, 2), ["open", "http"]);
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

test("failed browser persistence blocks both transport and popup creation", async () => {
  const ui = harness({ saved: { getItem: () => null, setItem() {} } });
  await ui.click("extensions.oauth.start"); ui.render();
  assert.equal(ui.calls.length, 0);
  assert.equal(ui.popups.length, 0);
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
