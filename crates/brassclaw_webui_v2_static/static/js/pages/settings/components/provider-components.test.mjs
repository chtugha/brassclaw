import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

import * as providerLibrary from "../lib/llm-providers.js";
import * as bootstrapLibrary from "../lib/provider-bootstrap.js";
import * as mutationLibrary from "../lib/provider-mutations.js";
import * as settingsApi from "../lib/settings-api.js";
import * as setupLibrary from "../lib/model-setup-draft.js";

const PROVIDER_GROUP_LABELS = [
  "llm.groupActive",
  "llm.groupReady",
  "llm.groupSetup",
];

// Keep the existing shallow template/hook harness, but bind every static
// import to an explicit module namespace. A new path or missing export fails
// at load time, including dependencies on render branches not visited below.
function evaluateSource(path, exportNames, modules, globals = {}) {
  const url = new URL(path, import.meta.url);
  const imports = [];
  let source = readFileSync(url, "utf8").replace(
    /^import\s+\{([^}]+)\}\s+from\s+["']([^"']+)["'];/gm,
    (_, names, specifier) => {
      const identity = specifier.startsWith(".") ? new URL(specifier, url).href : specifier;
      assert.ok(modules.has(identity), `${url.pathname}: unbound module ${specifier}`);
      const namespace = modules.get(identity);
      const bindings = names.split(",").map((name) => name.trim()).filter(Boolean).map((name) => {
        const match = name.match(/^(\w+)(?:\s+as\s+(\w+))?$/);
        assert.ok(match, `unsupported test import ${name} in ${url.pathname}`);
        const [, imported, local = imported] = match;
        assert.ok(Object.hasOwn(namespace, imported) && namespace[imported] !== undefined,
          `${url.pathname}: missing export ${imported} from ${specifier}`);
        return `${imported}: ${local}`;
      });
      imports.push(namespace);
      return `const { ${bindings.join(", ")} } = __testImports[${imports.length - 1}];`;
    }
  );
  assert.doesNotMatch(source, /^\s*import\b/m, `unsupported test import in ${url.pathname}`);
  source = source.replace(/^export function /gm, "function ");
  return vm.runInNewContext(
    `${source}\n;({ ${exportNames.join(", ")} });`,
    { ...globals, __testImports: imports },
    { filename: url.pathname }
  );
}

function loadForTest(path, exportNames, context = {}) {
  const modules = new Map();
  const bind = (path, namespace) => modules.set(new URL(path, import.meta.url).href, namespace);
  const unmounted = () => { throw Error("hook outside the selected component fixture"); };
  bind("../../../lib/html.js", {
    html,
    React: context.React || Object.fromEntries(
      ["useState", "useRef", "useEffect", "useReducer", "useCallback"].map(name => [name, unmounted])
    ),
  });
  bind("../../../lib/i18n.js", { useT: context.useT || (() => (key) => key) });
  for (const [file, names] of [
    ["button", ["Button"]], ["input", ["Select"]], ["card", ["Card"]],
    ["icons", ["Icon"]], ["badge", ["Badge"]],
  ]) {
    bind(`../../../design-system/${file}.js`, Object.fromEntries(names.map(name => [name, name])));
  }
  // These remain shallow child boundaries; their behavior has separate suites.
  for (const [file, name] of [
    ["settings-search-empty", "SettingsSearchEmpty"], ["provider-card", "ProviderCard"],
    ["provider-dialog", "ProviderDialog"], ["provider-login-status", "ProviderLoginStatus"],
    ["provider-mutation-status", "ProviderMutationStatus"], ["provider-runtime-status", "ProviderRuntimeStatus"],
    ["model-token-limits", "ModelTokenLimits"],
  ]) bind(`./${file}.js`, { [name]: name });
  bind("../hooks/useProviderManagementActions.js", { useProviderManagementActions: context.useProviderManagementActions || unmounted });
  bind("../hooks/useProviderLogin.js", { useProviderLogin: context.useProviderLogin || unmounted });
  bind("../lib/llm-providers.js", providerLibrary);
  bind("../lib/provider-bootstrap.js", bootstrapLibrary);
  bind("../lib/provider-mutations.js", mutationLibrary);
  bind("../lib/settings-api.js", settingsApi);
  bind("../lib/model-setup-draft.js", setupLibrary);
  bind("../../onboarding/provider-logos.js", { ProviderLogo: "ProviderLogo" });
  modules.set("react-router", { useNavigate: unmounted, useOutletContext: unmounted });
  for (const [file, name] of [
    ["provider-auth-status", "ProviderAuthStatus"],
    ["prefix-model-setup-draft", "PrefixModelSetupDraft"],
    ["provider-bootstrap-status", "ProviderBootstrapStatus"],
  ]) bind(`./${file}.js`, evaluateSource(`./${file}.js`, [name], modules));
  return evaluateSource(path, exportNames, modules, { document: context.document });
}

function html(strings, ...values) {
  return { strings: Array.from(strings), values };
}

function visit(node, fn) {
  if (Array.isArray(node)) {
    for (const item of node) visit(item, fn);
    return;
  }
  if (!node || typeof node !== "object") return;
  fn(node);
  if (Array.isArray(node.values)) {
    for (const value of node.values) visit(value, fn);
  }
}

function findComponentNodes(root, component) {
  const nodes = [];
  visit(root, (node) => {
    if (Array.isArray(node.values) && node.values.includes(component)) nodes.push(node);
  });
  return nodes;
}

function componentProps(node, component) {
  const props = {};
  const start = node.values.indexOf(component);
  for (let index = start + 1; index < node.values.length; index += 1) {
    const name = node.strings[index]?.match(/([A-Za-z][A-Za-z0-9]*)=\s*$/)?.[1];
    if (name) props[name] = node.values[index];
  }
  return props;
}

function collectScalars(root) {
  const scalars = [];
  visit(root, (node) => {
    if (!Array.isArray(node.values)) return;
    for (const value of node.values) {
      if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
        scalars.push(value);
      }
    }
  });
  return scalars;
}

function collectTemplateText(root) {
  const text = [];
  visit(root, (node) => {
    if (!Array.isArray(node.strings)) return;
    text.push(...node.strings);
  });
  return text.join("");
}

function valueAfter(rendered, fragment) {
  const index = rendered.strings.findIndex((part) => part.includes(fragment));
  assert.notEqual(index, -1, `expected template fragment ${fragment}`);
  return rendered.values[index];
}

function valuesAfter(rendered, fragment) {
  return rendered.strings.reduce((values, part, index) => {
    if (part.includes(fragment)) values.push(rendered.values[index]);
    return values;
  }, []);
}

function deepValuesAfter(root, fragment) {
  const values = [];
  visit(root, (node) => {
    if (!Array.isArray(node.strings) || !Array.isArray(node.values)) return;
    node.strings.forEach((part, index) => {
      if (part.includes(fragment)) values.push(node.values[index]);
    });
  });
  return values;
}

function builtinProvider(id, overrides = {}) {
  return {
    id,
    name: id,
    builtin: true,
    adapter: "open_ai_completions",
    api_key_required: true,
    base_url_required: false,
    has_api_key: true,
    default_model: "model",
    ...overrides,
  };
}

function customProvider(id, overrides = {}) {
  return {
    id,
    name: id,
    builtin: false,
    adapter: "ollama",
    api_key_required: false,
    base_url: "http://127.0.0.1:11434",
    default_model: "llama",
    ...overrides,
  };
}

function useProviderManagementActionsStub({ providers, activeProviderId, inactiveProviders, opened }) {
  return () => ({
    allProviderIds: providers.map((provider) => provider.id),
    closeDialog: () => {},
    dialogProvider: null,
    filteredProviders: providers,
    handleDelete: () => {},
    handleSave: () => {},
    handleUse: () => {},
    isDialogOpen: false,
    message: null,
    openDialog: (provider) => opened.push(provider),
    providerState: {
      inactiveProviders,
      activeProviderId,
      builtinOverrides: {},
      error: null,
      isBusy: false,
      isLoading: false,
      selectedModel: "llama",
    },
  });
}

function renderProviderManagement({ providers, activeProviderId = "nearai", searchQuery = "", inactiveProviders = [], React }) {
  const opened = [];
  const ProviderCard = "ProviderCard";
  const context = {
    React,
    useProviderManagementActions: useProviderManagementActionsStub({
      providers,
      activeProviderId,
      inactiveProviders,
      opened,
    }),
    useProviderLogin: () => ({
      codexBusy: false,
      nearaiBusy: false,
      startCodex: () => {},
      startNearai: () => {},
      startNearaiWallet: () => {},
    }),
    useT: () => (key) => key,
  };

  const { ProviderManagement } = loadForTest("./provider-management.js", ["ProviderManagement"], context);
  const rendered = ProviderManagement({
    settings: {},
    gatewayStatus: {},
    searchQuery,
  });
  const cardProps = findComponentNodes(rendered, ProviderCard).map((node) =>
    componentProps(node, ProviderCard)
  );
  return { rendered, cardProps, opened };
}

function groupLabels(rendered) {
  return collectScalars(rendered).filter((value) => PROVIDER_GROUP_LABELS.includes(value));
}

function depsChanged(previous, next) {
  if (!previous || !next || previous.length !== next.length) return true;
  return next.some((value, index) => value !== previous[index]);
}

function createReactStateStub(state) {
  return {
    useCallback: (fn) => fn,
    useEffect: (fn, deps) => {
      if (depsChanged(state.effectDeps, deps)) {
        state.effectDeps = deps ? Array.from(deps) : deps;
        fn();
      }
    },
    useState: (initial) => {
      if (!Object.hasOwn(state, "expanded")) {
        state.expanded = typeof initial === "function" ? initial() : initial;
      }
      return [
        state.expanded,
        (next) => {
          state.expanded = typeof next === "function" ? next(state.expanded) : next;
        },
      ];
    },
  };
}

function createReactMenuStateStub(state) {
  return {
    useEffect: (fn, deps) => {
      if (depsChanged(state.effectDeps, deps)) {
        state.effectDeps = deps ? Array.from(deps) : deps;
        fn();
      }
    },
    useRef: () => ({ current: null }),
    useState: (initial) => {
      if (!Object.hasOwn(state, "open")) {
        state.open = typeof initial === "function" ? initial() : initial;
      }
      return [
        state.open,
        (next) => {
          state.open = typeof next === "function" ? next(state.open) : next;
        },
      ];
    },
  };
}

function createProviderCardHarness({ translate = key => key } = {}) {
  const state = {};
  const context = {
    React: createReactStateStub(state),
    useT: () => translate,
  };

  const { ProviderCard } = loadForTest("./provider-card.js", ["ProviderCard"], context);

  return {
    state,
    render: (props) =>
      ProviderCard({
        activeProviderId: "nearai",
        selectedModel: "active-model",
        builtinOverrides: {},
        isBusy: false,
        onUse: () => {},
        onConfigure: () => {},
        onDelete: () => {},
        onNearaiLogin: () => {},
        onNearaiWallet: () => {},
        onCodexLogin: () => {},
        loginBusy: false,
        ...props,
      }),
  };
}

function createNearAiSetupMenuHarness() {
  const state = {};
  const calls = [];
  const context = {
    React: createReactMenuStateStub(state),
    document: {
      addEventListener: (type, handler) => {
        state.listeners ??= {};
        state.listeners[type] = handler;
      },
      removeEventListener: (type, handler) => {
        if (state.listeners?.[type] === handler) delete state.listeners[type];
      },
    },
  };

  const { NearAiSetupMenu } = loadForTest("../../onboarding/onboarding-page.js", ["NearAiSetupMenu"], context);

  return {
    calls,
    state,
    render: (props = {}) =>
      NearAiSetupMenu({
        provider: builtinProvider("nearai", { adapter: "nearai" }),
        isBusy: false,
        login: {
          nearaiBusy: false,
          startNearai: (provider) => calls.push(["sso", provider]),
          startNearaiWallet: () => calls.push(["wallet"]),
        },
        t: (key) => key,
        onSetUp: (provider) => calls.push(["configure", provider.id]),
        ...props,
      }),
  };
}

function firstButtonProps(rendered) {
  return componentProps(findComponentNodes(rendered, "Button")[0], "Button");
}

test("ProviderManagement groups filtered providers through the render caller", () => {
  const { rendered, cardProps } = renderProviderManagement({
    providers: [
      builtinProvider("nearai", { adapter: "nearai" }),
      builtinProvider("openai"),
      builtinProvider("anthropic", {
        adapter: "anthropic",
        has_api_key: false,
      }),
    ],
  });

  assert.deepEqual(groupLabels(rendered), PROVIDER_GROUP_LABELS);
  assert.deepEqual(deepValuesAfter(rendered, "data-provider-status="), [
    "active",
    "ready",
    "setup",
  ]);
  assert.deepEqual(
    cardProps.map((props) => props.provider.id),
    ["nearai", "openai", "anthropic"]
  );
  assert.deepEqual(
    cardProps.map((props) => props.activeProviderId),
    ["nearai", "nearai", "nearai"]
  );
});

test("ProviderManagement hides empty buckets after search filtering", () => {
  const { rendered, cardProps } = renderProviderManagement({
    providers: [builtinProvider("openai")],
    searchQuery: "open",
  });

  assert.deepEqual(groupLabels(rendered), ["llm.groupReady"]);
  assert.deepEqual(
    cardProps.map((props) => props.provider.id),
    ["openai"]
  );
});

test("ProviderCard disclosure responds to row, keyboard, and chevron controls", () => {
  const harness = createProviderCardHarness();
  const renderOpenAi = () =>
    harness.render({
      provider: builtinProvider("openai", { default_model: "gpt" }),
    });

  let rendered = renderOpenAi();
  assert.equal(valueAfter(rendered, "aria-expanded="), "false");

  valueAfter(rendered, "onClick=")();
  assert.equal(harness.state.expanded, true);

  rendered = renderOpenAi();
  assert.equal(valueAfter(rendered, "aria-expanded="), "true");

  valueAfter(rendered, "onClick=")();
  assert.equal(harness.state.expanded, false);

  rendered = renderOpenAi();
  valuesAfter(rendered, "onClick=")[1]();
  assert.equal(harness.state.expanded, true);
});

test("ProviderCard syncs disclosure state when active provider changes", () => {
  const harness = createProviderCardHarness();
  const provider = builtinProvider("openai", { default_model: "gpt" });

  let rendered = harness.render({ provider, activeProviderId: "nearai" });
  assert.equal(valueAfter(rendered, "aria-expanded="), "false");

  rendered = harness.render({ provider, activeProviderId: "openai" });
  rendered = harness.render({ provider, activeProviderId: "openai" });
  assert.equal(valueAfter(rendered, "aria-expanded="), "true");
  assert.equal(harness.state.expanded, true);

  rendered = harness.render({ provider, activeProviderId: "nearai" });
  rendered = harness.render({ provider, activeProviderId: "nearai" });
  assert.equal(valueAfter(rendered, "aria-expanded="), "false");
  assert.equal(harness.state.expanded, false);
});

test("ProviderCard actions keep existing callbacks without toggling disclosure", () => {
  const calls = [];
  const harness = createProviderCardHarness();

  let rendered = harness.render({
    onUse: (provider) => calls.push(["use", provider.id]),
    provider: builtinProvider("openai", { default_model: "gpt" }),
  });

  firstButtonProps(rendered).onClick();
  assert.deepEqual(calls, [["use", "openai"]]);
  assert.equal(harness.state.expanded, false);

  rendered = harness.render({
    onConfigure: (provider) => calls.push(["configure", provider.id]),
    provider: builtinProvider("anthropic", {
      adapter: "anthropic",
      has_api_key: false,
      default_model: "claude",
    }),
  });
  firstButtonProps(rendered).onClick();
  assert.deepEqual(calls.at(-1), ["configure", "anthropic"]);
  assert.equal(harness.state.expanded, false);

  harness.state.expanded = true;
  rendered = harness.render({
    onDelete: (provider) => calls.push(["delete", provider.id]),
    provider: customProvider("local"),
  });
  const deleteButton = findComponentNodes(rendered, "Button").find((node) =>
    collectScalars(node).includes("common.delete")
  );
  assert.ok(deleteButton, "expected delete button for expanded custom provider");
  componentProps(deleteButton, "Button").onClick();
  assert.deepEqual(calls.at(-1), ["delete", "local"]);
  assert.equal(harness.state.expanded, true);
});

test("ProviderCard renders login actions instead of generic use for login providers", () => {
  const calls = [];
  const harness = createProviderCardHarness();

  let rendered = harness.render({
    activeProviderId: "openai",
    onConfigure: (provider) => calls.push(["configure", provider.id]),
    provider: builtinProvider("nearai", { adapter: "nearai", has_api_key: false }),
  });
  let labels = collectScalars(rendered);
  let templateText = collectTemplateText(rendered);
  assert.ok(labels.includes("onboarding.nearWallet"));
  assert.ok(labels.includes("llm.addApiKey"));
  assert.ok(templateText.includes("GitHub"));
  assert.ok(templateText.includes("Google"));
  assert.ok(!labels.includes("llm.use"));
  const addKeyButton = findComponentNodes(rendered, "Button").find((node) => {
    const scalars = collectScalars(node);
    return scalars.includes("llm.addApiKey") && !scalars.includes("onboarding.nearWallet");
  });
  assert.ok(addKeyButton, "expected NEAR API key action");
  componentProps(addKeyButton, "Button").onClick();
  assert.deepEqual(calls, [["configure", "nearai"]]);

  rendered = harness.render({
    activeProviderId: "openai",
    provider: builtinProvider("openai_codex", { adapter: "openai_codex" }),
  });
  labels = collectScalars(rendered);
  templateText = collectTemplateText(rendered);
  assert.ok(labels.includes("onboarding.codexSignIn"));
  assert.ok(!labels.includes("llm.use"));
});

test("ProviderCard offers wallet action only with a bound handler", () => {
  const calls = [];
  const harness = createProviderCardHarness();
  const props = {
    activeProviderId: "openai",
    provider: builtinProvider("nearai", { adapter: "nearai", has_api_key: false }),
  };
  let rendered = harness.render({ ...props, onNearaiWallet: undefined });
  assert.ok(!collectScalars(rendered).includes("onboarding.nearWallet"));
  const walletHandler = () => calls.push("wallet");
  rendered = harness.render({ ...props, onNearaiWallet: walletHandler });
  const wallet = findComponentNodes(rendered, "Button").find((node) => componentProps(node, "Button").onClick === walletHandler);
  componentProps(wallet, "Button").onClick();
  assert.deepEqual(calls, ["wallet"]);
});

test("ProviderCard renders generic use action for NEAR when an API key is configured", () => {
  const calls = [];
  const harness = createProviderCardHarness();
  harness.state.expanded = true;

  const rendered = harness.render({
    activeProviderId: "openai",
    onUse: (provider) => calls.push(["use", provider.id]),
    provider: builtinProvider("nearai", {
      adapter: "nearai",
      has_api_key: true,
    }),
  });
  const labels = collectScalars(rendered);
  const templateText = collectTemplateText(rendered);

  assert.ok(labels.includes("llm.use"));
  assert.ok(labels.includes("llm.configure"));
  assert.ok(!labels.includes("llm.addApiKey"));
  assert.ok(!labels.includes("onboarding.nearWallet"));
  assert.ok(!templateText.includes("GitHub"));

  firstButtonProps(rendered).onClick();
  assert.deepEqual(calls, [["use", "nearai"]]);
});

test("NearAiSetupMenu keeps NEAR onboarding SSO choices behind setup dropdown", () => {
  const harness = createNearAiSetupMenuHarness();

  let rendered = harness.render();
  assert.equal(valueAfter(rendered, "aria-expanded="), "false");
  assert.equal(firstButtonProps(rendered).disabled, false);
  let labels = collectScalars(rendered);
  assert.ok(labels.includes("onboarding.setUp"));
  assert.ok(!labels.includes("llm.addApiKey"));
  assert.ok(!labels.includes("onboarding.nearWallet"));
  assert.ok(!labels.includes("GitHub"));

  firstButtonProps(rendered).onClick();
  assert.equal(harness.state.open, true);

  rendered = harness.render();
  assert.equal(valueAfter(rendered, "aria-expanded="), "true");
  assert.equal(typeof harness.state.listeners.keydown, "function");
  labels = collectScalars(rendered);
  assert.ok(labels.includes("llm.addApiKey"));
  assert.ok(labels.includes("onboarding.nearWallet"));
  assert.ok(labels.includes("GitHub"));
  assert.ok(labels.includes("Google"));

  deepValuesAfter(rendered, "onClick=")[1]();
  assert.deepEqual(harness.calls, [["configure", "nearai"]]);
  assert.equal(harness.state.open, false);

  firstButtonProps(harness.render()).onClick();
  rendered = harness.render();
  deepValuesAfter(rendered, "onClick=")[3]();
  assert.deepEqual(harness.calls.at(-1), ["sso", "github"]);
});

test("NearAiSetupMenu does not advertise an unbound wallet action", () => {
  const harness = createNearAiSetupMenuHarness();
  const login = { nearaiBusy: false, startNearai: () => {} };
  firstButtonProps(harness.render({ login })).onClick();
  assert.ok(!collectScalars(harness.render({ login })).includes("onboarding.nearWallet"));
});

test("NearAiSetupMenu disables setup trigger while setup or login is busy", () => {
  const harness = createNearAiSetupMenuHarness();

  assert.equal(firstButtonProps(harness.render({ isBusy: true })).disabled, true);
  assert.equal(
    firstButtonProps(
      harness.render({
        login: {
          nearaiBusy: true,
          startNearai: () => {},
          startNearaiWallet: () => {},
        },
      })
    ).disabled,
    true
  );
});

test("NearAiSetupMenu closes the setup dropdown on Escape", () => {
  const harness = createNearAiSetupMenuHarness();

  firstButtonProps(harness.render()).onClick();
  harness.render();

  harness.state.listeners.keydown({ key: "Enter" });
  assert.equal(harness.state.open, true);

  harness.state.listeners.keydown({ key: "Escape" });
  assert.equal(harness.state.open, false);
});

test("ProviderManagement restores only the selected retained provider through its configure dialog", () => {
  const retained = builtinProvider("removed-provider", { revision: 7 });
  const { rendered, cardProps, opened } = renderProviderManagement({ providers: [], inactiveProviders: [retained] });
  assert.equal(cardProps.length, 0, "removed definitions are not active cards");
  const select = findComponentNodes(rendered, "Select")[0];
  assert.ok(select);
  const props = componentProps(select, "Select");
  props.onChange({ target: { value: "removed-provider" } });
  assert.equal(opened.length, 1);
  assert.equal(opened[0], retained);
  assert.equal(opened[0].revision, 7);
  props.onChange({ target: { value: "missing-provider" } });
  assert.equal(opened.length, 1, "a stale choice cannot invent a definition");
});

test("Codex card dispatches the clicked custom provider, model and separate Kohai/Sempai roles", () => {
  const calls = [], harness = createProviderCardHarness();
  const provider = customProvider("custom-codex", { adapter:"openai_codex",default_model:"clicked-model",revision:7 });
  const rendered = harness.render({ provider, activeProviderId:"other",onCodexLogin:(...args)=>calls.push(args) });
  const group = findComponentNodes(rendered,"Button").find(node=>collectScalars(node).includes("onboarding.codexSignInSempai"));
  assert.ok(group);
  const callbacks = group.values.filter((value,index)=>typeof value === "function" && /onClick=\s*$/.test(group.strings[index]));
  assert.equal(callbacks.length,2);
  callbacks.forEach(callback=>callback());
  assert.deepEqual(calls,[[provider,"kohai","clicked-model"],[provider,"sempai","clicked-model"]]);
});

test("Codex onboarding passes its actual displayed provider and model", () => {
  const calls = [];
  const { FeaturedProviderRow } = loadForTest("../../onboarding/onboarding-page.js", ["FeaturedProviderRow"]);
  const provider = builtinProvider("openai_codex",{adapter:"openai_codex",default_model:"onboarding-model",revision:9});
  const rendered = FeaturedProviderRow({entry:{id:"openai_codex",auth:"codex",nameKey:"name",descKey:"desc"},
    provider,configured:false,isBusy:false,login:{codexBusy:false,startCodex:(...args)=>calls.push(args)},t:key=>key});
  const button = findComponentNodes(rendered,"Button")[0];
  componentProps(button,"Button").onClick();
  assert.deepEqual(calls,[[provider,"kohai","onboarding-model"]]);
});

function renderStatusChildren(root, name) {
  const rendered = [];
  visit(root, (node) => {
    const component = node.values?.find(value => typeof value === "function" && value.name === name);
    if (component) {
      const props = componentProps(node, component);
      rendered.push({ props, rendered: component(props) });
    }
  });
  return rendered;
}

test("ProviderManagement mounts the real bootstrap status without inventing an original target", () => {
  const cleanups = [];
  const React = {
    useState: initial => [initial, () => { throw Error("unexpected state update without a bootstrap event"); }],
    useRef: initial => ({ current: initial }),
    useEffect: effect => cleanups.push(effect()),
  };
  try {
    const { rendered } = renderProviderManagement({ providers: [], React });
    const statuses = renderStatusChildren(rendered, "ProviderBootstrapStatus");
    assert.equal(statuses.length, 1);
    assert.equal(statuses[0].rendered, null, "no receipt means no setup/ready claim");
    assert.equal(cleanups.length, 1, "the real subscription effect was mounted");
  } finally {
    for (const cleanup of cleanups) cleanup();
  }
});

test("ProviderCard passes the actual provider to auth status and preserves missing status", () => {
  for (const native_auth of [undefined, null]) {
    const harness = createProviderCardHarness();
    const provider = builtinProvider("openai", { native_auth });
    const { rendered, props } = renderStatusChildren(harness.render({ provider }), "ProviderAuthStatus")[0];
    assert.equal(props.provider, provider);
    assert.equal(props.compact, true);
    assert.equal(rendered, null, "missing auth observation is not authenticated/ready");
  }
});

// Projection fixtures only: none of these observations authenticates a user,
// renews credentials, publishes a provider or establishes serving readiness.
for (const observation of [
  { name: "locally current with unknown lifetime", capability: "available", registration: "bound",
    account: { status: "configured" }, access: "current", lifetime: "unknown", refresh: "present",
    recovery: "configured", login_supported: true },
  { name: "unfinished original setup", capability: "available", registration: "bound",
    account: { status: "pending_setup" }, access: "absent", lifetime: "unknown", refresh: "absent",
    recovery: "pending_setup", login_supported: true },
  { name: "failed refresh requiring original reconciliation", capability: "available", registration: "bound",
    account: { status: "refresh_failed" }, access: "expired", lifetime: "finite_deadline", refresh: "blocked_original",
    account_expires_at: "2026-11-12T10:00:00Z", secret_expires_at: "2026-11-12T09:00:00Z",
    blocking_refresh: { id: "original-refresh", outcome: "unconfirmed", publication: "pending", error: "refresh_rejected" },
    recovery: "refresh_failed", login_supported: true },
  { name: "missing registered account during custody recovery", capability: "custody_recovery_pending", registration: "account_missing",
    account: null, access: "unavailable", lifetime: "unknown", refresh: "unavailable",
    recovery: "account_missing", login_supported: false },
  { name: "unsupported native login", capability: "unsupported", registration: "absent",
    account: null, access: "ineligible", lifetime: "unknown", refresh: "absent",
    recovery: "no_account", login_supported: false },
]) {
  test(`ProviderCard renders real compact/detail auth projections: ${observation.name}`, () => {
    const translations = [];
    const harness = createProviderCardHarness({ translate: (key, values) => {
      translations.push({ key, values });
      return key;
    } });
    const provider = builtinProvider("openai", { native_auth: { ...observation, observed_at: "2026-11-13T10:00:00Z" } });
    const statuses = renderStatusChildren(harness.render({ provider, activeProviderId: "openai" }), "ProviderAuthStatus");
    assert.equal(statuses.length, 2);
    const [compact, detail] = statuses;
    assert.equal(compact.props.provider, provider);
    assert.equal(compact.props.compact, true);
    assert.equal(detail.props.provider, provider);
    assert.equal(detail.props.compact, undefined);
    for (const status of statuses) {
      const labels = collectScalars(status.rendered);
      assert.ok(labels.includes(`llm.nativeAuth.access.${observation.access}`));
      assert.ok(labels.includes(`llm.nativeAuth.refresh.${observation.refresh}`));
      const registration = observation.account ? `account.${observation.account.status}` : `registration.${observation.registration}`;
      // The compact registration label combines translated scalar values.
      assert.ok(labels.some(label => String(label).includes(`llm.nativeAuth.${registration}`)));
      assert.equal(labels.includes("llm.nativeAuth.loginUnsupported"), !observation.login_supported);
      if (observation.capability !== "available") {
        assert.ok(labels.includes(`llm.nativeAuth.capability.${observation.capability}`));
      }
    }
    const labels = collectScalars(detail.rendered);
    for (const label of [`llm.nativeAuth.registration.${observation.registration}`,
      `llm.nativeAuth.lifetime.${observation.lifetime}`, `llm.nativeAuth.recovery.${observation.recovery}`,
      provider.native_auth.observed_at, "llm.nativeAuth.help", "llm.nativeAuth.serving"]) assert.ok(labels.includes(label), label);
    assert.equal(labels.includes("llm.nativeAuth.accountExpiry"), Boolean(observation.account_expires_at));
    assert.equal(labels.includes("llm.nativeAuth.secretExpiry"), Boolean(observation.secret_expires_at));
    if (observation.blocking_refresh) {
      assert.deepEqual(translations.filter(call => call.key === "onboarding.codexStatusError").map(call => call.values.code),
        ["refresh_rejected", "refresh_rejected"], "both views preserve the actual safe error code");
      for (const value of ["original-refresh", "llm.nativeAuth.outcome.unconfirmed", "llm.nativeAuth.publication.pending",
        observation.account_expires_at, observation.secret_expires_at]) assert.ok(labels.includes(value), value);
    } else {
      assert.ok(!labels.includes("llm.nativeAuth.original"));
    }
  });
}

test("Codex onboarding renders the real missing/unsupported auth observation", () => {
  const { FeaturedProviderRow } = loadForTest("../../onboarding/onboarding-page.js", ["FeaturedProviderRow"]);
  for (const native_auth of [undefined, { capability: "unsupported", registration: "absent", access: "ineligible",
    lifetime: "unknown", refresh: "absent", login_supported: false }]) {
    const provider = builtinProvider("openai_codex", { native_auth });
    const row = FeaturedProviderRow({ entry: { id: "openai_codex", auth: "codex", nameKey: "name", descKey: "desc" },
      provider, configured: false, isBusy: false, login: { codexBusy: false }, t: key => key });
    const [status] = renderStatusChildren(row, "ProviderAuthStatus");
    assert.equal(status.props.provider, provider);
    if (!native_auth) assert.equal(status.rendered, null);
    else {
      const labels = collectScalars(status.rendered);
      assert.ok(labels.includes("llm.nativeAuth.capability.unsupported"));
      assert.ok(labels.includes("llm.nativeAuth.loginUnsupported"));
      assert.ok(!labels.includes("llm.nativeAuth.access.current"));
    }
  }
});
