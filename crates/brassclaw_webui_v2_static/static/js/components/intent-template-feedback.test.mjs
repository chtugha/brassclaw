import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

// VM harness mirroring provider-components.test.mjs: the `html` tagged
// template is stubbed to return `{ strings, values }` nodes (no React
// needed), and the component + its template-feedback dependency are loaded
// into a shared sandbox with their imports stripped.

function sourceForTest(path, exportNames) {
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
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
  return `${lines.join("\n")}\nglobalThis.__testExports = { ${exportNames.join(", ")} };`;
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

function renderFeedback({ expr }) {
  const context = { html, globalThis: {} };

  vm.runInNewContext(
    sourceForTest("../lib/template-feedback.js", ["templateFeedback"]),
    context
  );
  // template-feedback.js declares `function templateFeedback` at top level,
  // so it lands on the sandbox global; expose it explicitly so the component's
  // bare identifier resolves even if the second script resets __testExports.
  context.templateFeedback = context.globalThis.__testExports.templateFeedback;

  vm.runInNewContext(
    sourceForTest("./intent-template-feedback.js", ["IntentTemplateFeedback"]),
    context
  );

  return context.globalThis.__testExports.IntentTemplateFeedback({ expr });
}

test("IntentTemplateFeedback returns null for an empty expression", () => {
  assert.equal(renderFeedback({ expr: "" }), null);
});

test("IntentTemplateFeedback renders an anchored single-slot expression", () => {
  const rendered = renderFeedback({ expr: "list files in the % dir" });
  const scalars = collectScalars(rendered);
  const text = collectTemplateText(rendered);

  assert.ok(scalars.includes("Anchored"), `classification label; got ${JSON.stringify(scalars)}`);
  assert.ok(scalars.includes(1), `slot count scalar; got ${JSON.stringify(scalars)}`);
  assert.ok(scalars.includes("list files in the "), `prefix display; got ${JSON.stringify(scalars)}`);
  assert.ok(scalars.includes(" dir"), `suffix display; got ${JSON.stringify(scalars)}`);
  assert.ok(text.includes("%"), `slot chip marker rendered; got ${JSON.stringify(text)}`);
  assert.ok(text.includes("v2-accent-bg"), `chip styling applied; got ${JSON.stringify(text)}`);
});

test("IntentTemplateFeedback renders a leading-% suffix-only expression with a warning", () => {
  const rendered = renderFeedback({ expr: "% files" });
  const scalars = collectScalars(rendered);
  const text = collectTemplateText(rendered);

  assert.ok(
    scalars.includes("Suffix anchor only"),
    `classification label; got ${JSON.stringify(scalars)}`
  );
  const warning = scalars.find((value) => String(value).includes("leading-`%` template"));
  assert.ok(warning, `leading-% warning text rendered; got ${JSON.stringify(scalars)}`);
  assert.ok(text.includes("%"), `slot chip marker rendered; got ${JSON.stringify(text)}`);
});

test("IntentTemplateFeedback flags middle-adjacent slots as a Q1 error", () => {
  const rendered = renderFeedback({ expr: "a %% b" });
  const scalars = collectScalars(rendered);

  assert.ok(scalars.includes("Anchored"), `still anchored (prefix a + suffix b); got ${JSON.stringify(scalars)}`);
  const error = scalars.find((value) => String(value).includes("adjacent `%` slot markers"));
  assert.ok(error, `adjacent-slots error rendered; got ${JSON.stringify(scalars)}`);
});

test("IntentTemplateFeedback renders a plain exact-match expression with no slot chip", () => {
  const rendered = renderFeedback({ expr: "no slots here" });
  const scalars = collectScalars(rendered);
  const text = collectTemplateText(rendered);

  assert.ok(
    scalars.includes("Exact-match intent"),
    `classification label; got ${JSON.stringify(scalars)}`
  );
  assert.ok(!text.includes("%"), `no slot chip for a plain intent; got ${JSON.stringify(text)}`);
});
