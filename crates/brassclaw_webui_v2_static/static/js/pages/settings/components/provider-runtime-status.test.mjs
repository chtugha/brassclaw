import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
import { providerPublicationRows } from "../lib/llm-providers.js";
const source = readFileSync(new URL("./provider-runtime-status.js", import.meta.url), "utf8")
  .split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
const context = { html: (strings, ...values) => ({ strings: [...strings], values }) };
vm.createContext(context);
vm.runInContext(source + "\nglobalThis.render=ProviderRuntimeStatus;", context);
function values(value, result = []) {
  if (Array.isArray(value)) value.forEach(item => values(item, result));
  else if (value?.values) value.values.forEach(item => values(item, result));
  else if (value !== undefined) result.push(value);
  return result;
}
test("runtime panel renders desired and actual identities separately without claiming prefix uptake", () => {
  const old = {state: "configured", selection: {provider_id: "original", model: "old", definition_revision: 2, configuration_ref: "original-authenticated-ref"}};
  const snapshot = {desired_publications: {kohai: {...old, selection: {...old.selection, model: "new", configuration_ref: "new-authenticated-ref"}},
    sempai: {state: "unconfigured"}, embedding: {state: "untracked"}},
    runtime_publications: {kohai: old, sempai: {state: "unconfigured"}, embedding: {state: "untracked"}},
    embedding_active: {provider_id: "broken-external-selection"}};
  const view = values(context.render({rows: providerPublicationRows(snapshot), t: key => key}));
  for (const item of ["llm.publicationHelp", "llm.publicationState.pending", "llm.publicationState.disabled", "llm.publicationState.invalid",
    "original · old · r2 · original-authenticated-ref", "original · new · r2 · new-authenticated-ref", "broken-external-selection"]) assert.ok(view.some(value => String(value).includes(item)), item);
  assert.equal(context.render({t: key => key}), null, "absent status is not invented acknowledgement");
});

test("failed target is displayed separately from a later pending edit", () => {
  const selected = ref => ({state: "configured", selection: {provider_id: "original", model: "model", definition_revision: 2, configuration_ref: ref}});
  const snapshot = {desired_publications: {kohai: selected("newer-target")}, runtime_publications: {kohai: selected("effective-target")}, runtime_publication_failures: {kohai: selected("failed-target")}};
  const view = values(context.render({rows: providerPublicationRows(snapshot), t: key => key}));
  for (const item of ["llm.publicationFailureHelp", "llm.publicationFailedTarget", "llm.publicationState.pending", "original · model · r2 · failed-target"]) assert.ok(view.some(value => String(value).includes(item)), item);
  assert.ok(!view.includes("llm.publicationState.failed"));
});
