import assert from "node:assert/strict";
import test from "node:test";

import {
  classOf,
  componentRefs,
  indexComponentGraph,
  recipeChannelAudit,
  recipeOutcomeStats,
  tierCoverage,
} from "./component-graph.js";

const node = (id, class_code, name) => ({
  id,
  name,
  class_code,
  validation_status: "validated",
});

// A canonical Tier-0 recipe: rust step pre-loads the ToolSkill, orchestrator
// step runs the PythonCode that dispatches it, ToolSkill binds the Tool.
const GRAPH = {
  nodes: [
    node("recipe-1", 21, "read file"),
    node("ts-1", 13, "ts-read-file"),
    node("pc-1", 22, "pc-read-file"),
    node("tool-1", 0, "read_file"),
  ],
  edges: [
    {
      from: "recipe-1",
      to: "ts-1",
      kind: "step_include",
      channel: "rust",
      step_ref: "step-0",
      step_label: "Pre-load binding",
    },
    {
      from: "recipe-1",
      to: "pc-1",
      kind: "step_include",
      channel: "orchestrator",
      step_ref: "step-1",
      step_label: "Execute",
    },
    {
      from: "ts-1",
      to: "tool-1",
      kind: "tool_binding",
      channel: null,
      step_ref: null,
      step_label: "read_file",
    },
  ],
};

test("indexComponentGraph indexes both directions", () => {
  const index = indexComponentGraph(GRAPH);
  assert.equal(index.nodes.size, 4);
  assert.equal(index.outbound.get("recipe-1").length, 2);
  assert.equal(index.inbound.get("pc-1").length, 1);
  assert.equal(index.inbound.get("tool-1")[0].from, "ts-1");
});

test("indexComponentGraph tolerates an absent or malformed payload", () => {
  for (const payload of [null, undefined, {}, { nodes: null, edges: null }]) {
    const index = indexComponentGraph(payload);
    assert.equal(index.nodes.size, 0);
    assert.equal(index.outbound.size, 0);
  }
  const partial = indexComponentGraph({
    nodes: [null, { name: "no id" }],
    edges: [{ from: "a" }, { to: "b" }],
  });
  assert.equal(partial.nodes.size, 0);
  assert.equal(partial.outbound.size, 0);
});

test("componentRefs resolves the reverse reference a list view cannot", () => {
  const index = indexComponentGraph(GRAPH);
  const pythonCode = componentRefs(index, "pc-1");
  assert.equal(pythonCode.references.length, 0);
  assert.equal(pythonCode.referencedBy.length, 1);
  assert.equal(pythonCode.referencedBy[0].node.name, "read file");
  assert.equal(pythonCode.referencedBy[0].channel, "orchestrator");

  const recipe = componentRefs(index, "recipe-1");
  assert.deepEqual(
    recipe.references.map((r) => r.id),
    ["ts-1", "pc-1"]
  );
});

test("componentRefs keeps an edge whose target was never seeded", () => {
  const index = indexComponentGraph({
    nodes: [node("recipe-1", 21, "r")],
    edges: [{ from: "recipe-1", to: "ghost", kind: "step_include" }],
  });
  const refs = componentRefs(index, "recipe-1");
  assert.equal(refs.references.length, 1);
  assert.equal(refs.references[0].node, null);
  assert.equal(refs.references[0].id, "ghost");
});

test("componentRefs returns empty for an unknown id", () => {
  const index = indexComponentGraph(GRAPH);
  assert.deepEqual(componentRefs(index, "nope"), { references: [], referencedBy: [] });
  assert.deepEqual(componentRefs(null, "recipe-1"), { references: [], referencedBy: [] });
});

test("classOf reads the node class, null when unknown", () => {
  const index = indexComponentGraph(GRAPH);
  assert.equal(classOf(index, "ts-1"), 13);
  assert.equal(classOf(index, "ghost"), null);
});

test("recipeChannelAudit accepts a matched rust/orchestrator pair", () => {
  const index = indexComponentGraph(GRAPH);
  const steps = [
    { channel: "rust", include: ["ts-1"] },
    { channel: "orchestrator", include: ["pc-1"] },
  ];
  assert.deepEqual(recipeChannelAudit(steps, index), {
    rustSteps: 1,
    orchestratorExecutors: 1,
    unmatchedRustSteps: 0,
  });
});

test("recipeChannelAudit flags a rust step with no PythonCode executor", () => {
  const index = indexComponentGraph(GRAPH);
  const steps = [{ channel: "rust", include: ["ts-1"] }];
  assert.equal(recipeChannelAudit(steps, index).unmatchedRustSteps, 1);
});

test("recipeChannelAudit ignores an orchestrator step that loads a Skill, not PythonCode", () => {
  const index = indexComponentGraph({
    nodes: [...GRAPH.nodes, node("skill-1", 1, "leaf skill")],
    edges: GRAPH.edges,
  });
  const steps = [
    { channel: "rust", include: ["ts-1"] },
    { channel: "orchestrator", include: ["skill-1"] },
  ];
  assert.equal(recipeChannelAudit(steps, index).unmatchedRustSteps, 1);
});

test("recipeChannelAudit does not manufacture a violation without a graph", () => {
  const steps = [
    { channel: "rust", include: ["ts-1"] },
    { channel: "orchestrator", include: ["pc-1"] },
  ];
  assert.equal(recipeChannelAudit(steps, null).unmatchedRustSteps, 0);
  assert.equal(recipeChannelAudit([], null).rustSteps, 0);
  assert.equal(recipeChannelAudit(null, null).rustSteps, 0);
});

test("recipeChannelAudit skips a rust step that pre-loads nothing", () => {
  const index = indexComponentGraph(GRAPH);
  assert.equal(recipeChannelAudit([{ channel: "rust", include: [] }], index).rustSteps, 0);
});

test("tierCoverage counts an unknown tier separately from Tier 1", () => {
  const coverage = tierCoverage([
    { tier: "0" },
    { tier: "0" },
    { tier: "1" },
    { tier: null },
  ]);
  assert.deepEqual(coverage, {
    tier0: 2,
    tier1: 1,
    unknown: 1,
    total: 4,
    percent: 67,
  });
});

test("tierCoverage reports no percentage when no tier is known", () => {
  assert.equal(tierCoverage([{ tier: null }]).percent, null);
  assert.equal(tierCoverage([]).percent, null);
  assert.equal(tierCoverage(null).total, 0);
});

test("recipeOutcomeStats surfaces the counters the table actually has", () => {
  assert.deepEqual(
    recipeOutcomeStats({
      usage_count: 12,
      success_count: 11,
      failure_count: 1,
      wilson_lower: 0.64,
      tier: "growing",
    }),
    { usage: 12, success: 11, failure: 1, wilsonLower: 0.64, rewardTier: "growing" }
  );
});

test("recipeOutcomeStats is null when the row carries no counters", () => {
  assert.equal(recipeOutcomeStats({}), null);
  assert.equal(recipeOutcomeStats(null), null);
  assert.equal(recipeOutcomeStats({ usage_count: "12" }), null);
});
