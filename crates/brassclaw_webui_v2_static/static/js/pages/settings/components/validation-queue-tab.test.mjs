// Rendered UI gates and actual message packs only. No mocked backend approval.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const withoutImports = source => source.replace(/^import[\s\S]*?from\s+["'][^"']+["'];[ \t]*\n?/gm, "");
function messages(language) {
  let pack;
  const source = readFileSync(new URL(`../../../i18n/${language}.js`, import.meta.url), "utf8");
  vm.runInNewContext(withoutImports(source), { registerPack(name, entries) { if (name === language) pack = entries; } });
  assert.ok(pack, `missing actual ${language} message pack`);
  return pack;
}
function translations(language) {
  let providerContext;
  const context = {
    navigator: { language }, localStorage: { getItem: () => null },
    document: { documentElement: {} },
    React: {
      createContext(initial) { providerContext = { current: initial, Provider: "I18nProvider" }; return providerContext; },
      useState: initial => [typeof initial === "function" ? initial() : initial, () => {}],
      useCallback: callback => callback, useMemo: callback => callback(),
      useEffect: callback => callback(), useContext: context => context.current,
    },
    html(strings, provider, value) {
      assert.equal(provider, providerContext.Provider);
      providerContext.current = value;
    },
  };
  vm.createContext(context);
  const source = readFileSync(new URL("../../../lib/i18n.js", import.meta.url), "utf8");
  vm.runInContext(withoutImports(source).replaceAll("export ", ""), context);
  for (const packLanguage of new Set(["en", language])) {
    const packSource = readFileSync(new URL(`../../../i18n/${packLanguage}.js`, import.meta.url), "utf8");
    vm.runInContext(withoutImports(packSource), context);
  }
  // Execute the real provider/detection/translate/useT owners with controlled
  // React context storage; fallback and interpolation are not reimplemented.
  context.I18nProvider({ children: null });
  assert.equal(providerContext.current.lang, language);
  assert.equal(context.document.documentElement.lang, language);
  return { useT: context.useT, t: context.useT() };
}
function fixture({ language = "en", state = "legacy_unresolved", classCode = 21 } = {}) {
  const pack = messages(language), states = [], validationCalls = [], rejectionCalls = [];
  const { useT, t } = translations(language);
  let cursor = 0;
  const context = {
    React: { useState(initial) { const i = cursor++; if (!(i in states)) states[i] = initial; return [states[i], value => { states[i] = typeof value === "function" ? value(states[i]) : value; }]; } },
    html: (strings, ...values) => ({ strings: [...strings], values }), Badge: "Badge", useT,
    useQueryClient: () => ({ invalidateQueries() {} }),
    // No synthetic success/evidence callbacks: only the affordance is tested.
    useMutation: options => ({ isPending: false, mutate: value => options.mutationFn(value) }),
    validateComponent: (...args) => { validationCalls.push(args); },
    rejectComponent: (...args) => { rejectionCalls.push(args); },
    componentTypeForClass: () => null,
  };
  const source = withoutImports(readFileSync(new URL("./validation-queue-tab.js", import.meta.url), "utf8")).replaceAll("export ", "");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.renderRow = QueueRow;", context);
  const item = {
    id: "component", name: "Original retained candidate", description: "Review fixture", class_code: classCode,
    queue_code: "q2_manual", validation_status: "auto_passed", llm_audit_status: "not_applicable", q2_actor: null,
    original_review: { state, queue_id: "original-queue-id", submission_id: ["joint_unsupported", "bound"].includes(state) ? "retained-original-subject" : null },
  };
  return { render() { cursor = 0; return context.renderRow({ item }); }, pack, t, validationCalls, rejectionCalls };
}
function nodes(root) {
  const out = [];
  const visit = value => { if (Array.isArray(value)) return value.forEach(visit); if (!value?.strings) return; out.push(value); value.values.forEach(visit); };
  visit(root); return out;
}
function button(tree, label) {
  for (const node of nodes(tree)) {
    const end = node.values.indexOf(label); if (end < 0) continue;
    let start = end; while (start >= 0 && !node.strings[start].includes("<button")) start--;
    if (start >= 0) return { node, start, end };
  }
}
function prop({ node, start, end }, name) {
  const index = node.strings.findIndex((text, i) => i >= start && i <= end && text.includes(`${name}=`));
  assert.ok(index >= 0, `missing ${name}`); return node.values[index];
}
function visible(tree, text) {
  assert.equal(typeof text, "string", "expected an actual message/text value");
  return nodes(tree).some(node => node.values.includes(text));
}

for (const language of ["en", "de"]) {
  for (const state of ["legacy_unresolved", "joint_unsupported"]) {
    test(`${language} ${state} is visible with its original queue ID and disables Q2 approval`, () => {
      const f = fixture({ language, state }); const tree = f.render();
      const message = f.pack[`validationQueue.original.${state}`];
      assert.ok(visible(tree, message), "the diagnosis must be visible, not just an unused translation");
      assert.ok(visible(tree, "original-queue-id"));
      const approveLabel = f.t("validationQueue.validate");
      assert.notEqual(approveLabel, "validationQueue.validate", "approval must have an actual translated/fallback label");
      const approve = button(tree, approveLabel);
      assert.ok(approve); assert.equal(prop(approve, "disabled"), true);
      assert.equal(f.validationCalls.length, 0); assert.equal(f.rejectionCalls.length, 0);
      if (state === "legacy_unresolved") {
        assert.match(message, language === "en" ? /custody is unresolved/i : /Kustodie ist ungeklärt/);
        assert.match(message, language === "en" ? /not connected yet/ : /noch nicht verbunden/);
        assert.match(message, language === "en" ? /do not replay or resubmit/ : /nicht erneut ausführen oder einreichen/);
      } else {
        assert.match(message, language === "en" ? /partial approval is blocked/ : /Teilfreigabe ist gesperrt/);
      }
    });
  }
}
for (const classCode of [1, 22]) {
  test(`joint class ${classCode} shows the missing action and offers no partial Q2 button`, () => {
    const f = fixture({ state: "joint_unsupported", classCode }); const tree = f.render();
    assert.ok(visible(tree, f.pack["validationQueue.original.joint_unsupported"]));
    assert.ok(visible(tree, f.pack["validationQueue.original.actionUnavailable"]));
    assert.equal(button(tree, f.t("validationQueue.validate")), undefined);
    assert.equal(button(tree, f.t("validationQueue.reject")), undefined);
    assert.equal(f.validationCalls.length, 0); assert.equal(f.rejectionCalls.length, 0);
  });
}
test("a bound Recipe Q2 row keeps its existing approval affordance without claiming backend success", () => {
  const f = fixture({ state: "bound" }); const tree = f.render();
  const approve = button(tree, f.t("validationQueue.validate"));
  assert.ok(approve); assert.equal(prop(approve, "disabled"), false);
  assert.equal(visible(tree, f.pack["validationQueue.original.legacy_unresolved"]), false);
  assert.equal(visible(tree, f.pack["validationQueue.original.joint_unsupported"]), false);
  assert.equal(f.validationCalls.length, 0);
});
test("a new unbound draft explains explicit binding while approval stays disabled", () => {
  const f = fixture({ state: "unresolved" }); const tree = f.render();
  assert.ok(visible(tree, f.pack["validationQueue.original.unresolved"]));
  assert.equal(visible(tree, f.pack["validationQueue.original.legacy_unresolved"]), false);
  assert.equal(prop(button(tree, f.t("validationQueue.validate")), "disabled"), true);
  assert.equal(f.validationCalls.length, 0);
});
