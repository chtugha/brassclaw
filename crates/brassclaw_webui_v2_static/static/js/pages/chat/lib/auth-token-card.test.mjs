import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function card({ token, onSubmit, onContinue }) {
  let index = 0;
  const states = [token];
  const React = {
    useCallback: fn => fn,
    useState(initial) {
      const position = index++;
      if (!(position in states)) states[position] = initial;
      return [states[position], value => { states[position] = value; }];
    },
  };
  const context = vm.createContext({
    React, useT: () => key => key,
    html: (strings, ...values) => ({ strings, values }),
    Button: "Button", Input: "Input", AuthGateShell: "AuthGateShell",
  });
  const original = readFileSync(new URL("../components/auth-token-card.js", import.meta.url), "utf8");
  const source = original.split("\n").filter(line => !line.startsWith("import "))
    .join("\n").replace("export function AuthTokenCard", "function AuthTokenCard");
  vm.runInContext(`${source}\nglobalThis.card = AuthTokenCard;`, context);
  const props = { gate: {}, onSubmit, onContinue };
  let tree = context.card(props);
  function attribute(node, name) {
    for (let i = 0; i < node.strings.length; i++) {
      if (node.strings[i].endsWith(`${name}=`)) return node.values[i];
    }
  }
  function find(node, predicate) {
    if (!node?.strings) return null;
    if (predicate(node)) return node;
    for (const child of node.values) {
      const found = find(child, predicate);
      if (found) return found;
    }
    return null;
  }
  const submit = attribute(tree, "onSubmit");
  const saved = find(tree, node => node.values.includes("authGate.continueSaved"));
  function attributes(node, name) {
    if (!node?.strings) return [];
    const values = [];
    for (let i = 0; i < node.strings.length; i++) {
      if (node.strings[i].endsWith(`${name}=`)) values.push(node.values[i]);
    }
    for (const child of node.values) values.push(...attributes(child, name));
    return values;
  }
  return { states, submit: () => submit({ preventDefault() {} }),
    continueSaved: saved && attribute(saved, "onClick"),
    disabledAfterRender() { index = 0; tree = context.card(props); return attributes(tree, "disabled"); },
  };
}

test("manual token card preserves exact raw text and never displays a raw error", async () => {
  const raw = "  original ü  ";
  let submitted;
  const view = card({ token: raw, onSubmit: async value => { submitted = value; } });
  await view.submit();
  assert.equal(submitted, raw);
  assert.equal(view.states[0], "");
  const failed = card({ token: raw, onSubmit: async () => { throw new Error(raw); } });
  await failed.submit();
  assert.equal(failed.states[0], raw);
  assert.equal(failed.states[1], "authGate.operationUnconfirmed");
  assert.equal(failed.states[1].includes(raw), false);
});

test("manual token card has a distinct reference action that never submits entered raw text", async () => {
  let rawCalls = 0, referenceArguments;
  const view = card({ token: "entered but unused raw", onSubmit: async () => { rawCalls += 1; },
    onContinue: async (...args) => { referenceArguments = args; } });
  await view.continueSaved();
  assert.equal(rawCalls, 0);
  assert.deepEqual(referenceArguments, []);
  assert.equal(view.states[0], "");
});

test("terminal setup disables raw/reference while cancellation remains available and unknown codes stay generic", async () => {
  const expired = card({ token: "private", onContinue: async () => {}, onSubmit: async () => {
    const failure = new Error("private provider detail");
    failure.safeAuthGateCode = "manual_setup_expired"; throw failure;
  } });
  await expired.submit();
  assert.equal(expired.states[1], "authGate.setupExpired");
  assert.equal(expired.states[3], true);
  assert.equal(expired.states[0], "");
  const disabled = expired.disabledAfterRender();
  assert.equal(disabled.filter(Boolean).length, 3);
  assert.equal(disabled.filter(value => value === false).length, 1);
  const unknown = card({ token: "private", onSubmit: async () => {
    const failure = new Error("private provider detail");
    failure.safeAuthGateCode = "__proto__"; throw failure;
  } });
  await unknown.submit();
  assert.equal(unknown.states[1], "authGate.operationUnconfirmed");
  assert.equal(unknown.states[3], false);
  assert.equal(unknown.states[0], "private");
});
