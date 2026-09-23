// Cross-reference index over `GET /api/settings/component-graph`.
//
// The endpoint returns the whole catalog in one payload — every component as
// a node, every `step_descriptions` include / ToolSkill tool binding /
// `dependency_registry` entry as a directed edge. Indexing it here lets a
// detail pane answer both directions of a reference ("what does this Recipe
// include", "which Recipes include this PythonCode") without a second
// request per component.

/** Class code → catalog label, for rendering a reference's target type. */
export const CLASS_LABELS = {
  0: "Tool",
  1: "Skill",
  2: "Skill",
  3: "Skill",
  10: "Orchestrator",
  13: "ToolSkill",
  16: "Action",
  17: "Docu",
  21: "Recipe",
  22: "PythonCode",
  23: "ExtensionCatalogue",
  50: "Scaffold",
};

export function classLabel(classCode) {
  const code = Number(classCode);
  return CLASS_LABELS[code] ?? `class ${classCode}`;
}

export const EMPTY_GRAPH_INDEX = {
  nodes: new Map(),
  outbound: new Map(),
  inbound: new Map(),
};

function push(map, key, value) {
  const existing = map.get(key);
  if (existing) existing.push(value);
  else map.set(key, [value]);
}

/**
 * Build the lookup maps for one graph payload.
 *
 * An edge whose `to` matches no node is kept: an `include[]` pointing at a
 * component that was never seeded is drift an operator needs to see, so it
 * renders as an unresolved reference rather than disappearing.
 */
export function indexComponentGraph(graph) {
  const nodes = new Map();
  const outbound = new Map();
  const inbound = new Map();

  for (const node of graph?.nodes ?? []) {
    if (node?.id) nodes.set(node.id, node);
  }
  for (const edge of graph?.edges ?? []) {
    if (!edge?.from || !edge?.to) continue;
    push(outbound, edge.from, edge);
    push(inbound, edge.to, edge);
  }

  return { nodes, outbound, inbound };
}

function resolve(index, edges, endpointKey) {
  const seen = new Set();
  const out = [];
  for (const edge of edges ?? []) {
    const targetId = edge[endpointKey];
    const dedupeKey = `${targetId}|${edge.kind}|${edge.channel ?? ""}|${edge.step_ref ?? ""}`;
    if (seen.has(dedupeKey)) continue;
    seen.add(dedupeKey);
    out.push({
      id: targetId,
      node: index.nodes.get(targetId) ?? null,
      kind: edge.kind,
      channel: edge.channel ?? null,
      stepRef: edge.step_ref ?? null,
      stepLabel: edge.step_label ?? null,
    });
  }
  return out;
}

/** References this component makes (`references`) and receives (`referencedBy`). */
export function componentRefs(index, id) {
  if (!index || !id) return { references: [], referencedBy: [] };
  return {
    references: resolve(index, index.outbound.get(id), "to"),
    referencedBy: resolve(index, index.inbound.get(id), "from"),
  };
}

/** Class code of a component id, or `null` when the graph does not know it. */
export function classOf(index, id) {
  const node = index?.nodes?.get(id);
  return node ? Number(node.class_code) : null;
}

const PYTHON_CODE_CLASS = 22;

/**
 * Q1 Rule 2 audit of one Recipe's normalized steps.
 *
 * `channel: "rust"` only pre-loads a ToolSkill binding — the tool is invoked
 * solely by a following `channel: "orchestrator"` PythonCode step. A rust
 * step with no orchestrator PythonCode step to match it is a hard error, not
 * a style issue, so it is counted rather than glossed over.
 *
 * `index` resolves an include UUID to its class. Without it (graph still
 * loading, or an unseeded id) an orchestrator step carrying any include is
 * counted as an executor — a missing graph must not manufacture a violation.
 */
export function recipeChannelAudit(steps, index) {
  let rustSteps = 0;
  let orchestratorExecutors = 0;

  for (const step of steps ?? []) {
    const channel = String(step?.channel ?? "").toLowerCase();
    const includes = step?.include ?? [];
    if (channel === "rust" && includes.length > 0) {
      rustSteps += 1;
      continue;
    }
    if (channel !== "orchestrator") continue;
    const runsPythonCode = includes.some((id) => {
      const code = classOf(index, id);
      return code === null || code === PYTHON_CODE_CLASS;
    });
    if (runsPythonCode) orchestratorExecutors += 1;
  }

  return {
    rustSteps,
    orchestratorExecutors,
    unmatchedRustSteps: Math.max(0, rustSteps - orchestratorExecutors),
  };
}

/**
 * Tier-0 coverage across a Recipe list.
 *
 * `tier` is the list field derived server-side from `llm_call_required`;
 * a recipe whose `steps` JSONB predates the canonical shape has no tier and
 * is counted as unknown rather than assumed Tier 0.
 */
export function tierCoverage(items) {
  let tier0 = 0;
  let tier1 = 0;
  let unknown = 0;
  for (const item of items ?? []) {
    if (item?.tier === "0") tier0 += 1;
    else if (item?.tier === "1") tier1 += 1;
    else unknown += 1;
  }
  const total = tier0 + tier1 + unknown;
  const known = tier0 + tier1;
  return {
    tier0,
    tier1,
    unknown,
    total,
    percent: known === 0 ? null : Math.round((tier0 / known) * 100),
  };
}

/**
 * Per-recipe outcome counters, as `reborn_recipes` actually stores them
 * (V033: `usage_count`, `success_count`, `failure_count`, `wilson_lower`).
 *
 * There is deliberately no last-used value: the table has no
 * `last_used_at` column, and `updated_at` is bumped by edits as well as by
 * `record_outcome`, so showing it as "last used" would be a lie.
 */
export function recipeOutcomeStats(row) {
  const num = (value) => (typeof value === "number" && Number.isFinite(value) ? value : null);
  const usage = num(row?.usage_count);
  const success = num(row?.success_count);
  const failure = num(row?.failure_count);
  if (usage === null && success === null && failure === null) return null;
  return {
    usage,
    success,
    failure,
    wilsonLower: num(row?.wilson_lower),
    rewardTier: typeof row?.tier === "string" ? row.tier : null,
  };
}
