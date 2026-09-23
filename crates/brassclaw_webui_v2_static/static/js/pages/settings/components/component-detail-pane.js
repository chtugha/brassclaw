import { React, html } from "../../../lib/html.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery } from "@tanstack/react-query";
import {
  fetchSettingsComponentDetail,
  fetchSettingsComponentGraph,
} from "../lib/settings-api.js";
import {
  classLabel,
  componentRefs,
  EMPTY_GRAPH_INDEX,
  indexComponentGraph,
  recipeChannelAudit,
  recipeOutcomeStats,
} from "../lib/component-graph.js";
import {
  ChannelBadge,
  ChipList,
  CodeBlock,
  ConsumerTags,
  Field,
  FieldGrid,
  IntentExamples,
  Section,
  TextBlock,
  TierBadge,
  isEmptyValue,
} from "./component-detail-primitives.js";

/**
 * The `/api/settings/{type}/{id}` path segment a class is addressed by —
 * the frontend mirror of `SettingsComponentType::from_path_segment`. Classes
 * without a detail endpoint (12, 14, 15, 17, 18, 19, 20) return `null`.
 */
export function componentTypeForClass(classCode) {
  const code = Number(classCode);
  if (code === 0) return "tools";
  if ([1, 2, 3].includes(code)) return "skills";
  if (code === 10) return "orchestrators";
  if (code === 13) return "tool-skills";
  if (code === 16) return "actions";
  if (code === 21) return "recipes";
  if (code === 22) return "python-code";
  if (code === 23) return "extension-catalogues";
  if (code === 50) return "scaffolds";
  return null;
}

/**
 * The whole catalog's wiring, fetched once and shared by every open pane.
 *
 * The query key carries no component id on purpose: react-query then serves
 * every expanded row from one cached payload instead of re-fetching the graph
 * per row.
 */
export function useComponentGraph() {
  const query = useQuery({
    queryKey: ["settings", "component-graph"],
    queryFn: fetchSettingsComponentGraph,
    staleTime: 60_000,
  });
  const index = React.useMemo(
    () => (query.data ? indexComponentGraph(query.data) : EMPTY_GRAPH_INDEX),
    [query.data]
  );
  return { index, isLoading: query.isLoading, isError: query.isError, isReady: Boolean(query.data) };
}

/**
 * Class-aware read-only detail pane for one Component Catalog row.
 *
 * `componentType` is the `/api/settings/{type}/{id}` path segment; the
 * response `component` is the whole DB row as opaque JSON, so every renderer
 * reads defensively — a column the engine has not grown yet is simply absent.
 */
export function ComponentDetailPane({ componentType, id, classCode }) {
  const t = useT();
  const resolvedType = componentType ?? componentTypeForClass(classCode);
  const graph = useComponentGraph();
  const query = useQuery({
    queryKey: ["settings", "component", resolvedType, id],
    queryFn: () => fetchSettingsComponentDetail(resolvedType, id),
    enabled: Boolean(resolvedType && id),
  });

  if (!resolvedType) {
    return html`
      <p className="text-xs text-[var(--v2-text-faint)]">
        ${t("componentDetail.unsupportedClass", { class: String(classCode) })}
      </p>
    `;
  }

  if (query.isLoading) {
    return html`
      <div className="space-y-2">
        <div className="h-3 w-40 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
        <div className="h-24 w-full animate-pulse rounded bg-[var(--v2-surface-muted)]" />
      </div>
    `;
  }

  if (query.isError) {
    return html`
      <p className="text-xs text-[var(--v2-danger-text)]">
        ${t("componentDetail.failedLoad", {
          message: query.error?.message ?? String(query.error),
        })}
      </p>
    `;
  }

  const row = query.data?.component ?? {};
  const resolvedClass = Number(query.data?.class_code ?? classCode);
  const resolvedId = query.data?.id ?? id;

  return html`
    <div className="space-y-5">
      ${classBody(resolvedClass, row, graph)}
      <${CommonFooter}
        row=${row}
        classCode=${resolvedClass}
        id=${resolvedId}
        graph=${graph}
      />
    </div>
  `;
}

function classBody(classCode, row, graph) {
  if (classCode === 21) return html`<${RecipeDetail} row=${row} graph=${graph} />`;
  if (classCode === 13) return html`<${ToolSkillDetail} row=${row} />`;
  if (classCode === 22) return html`<${PythonCodeDetail} row=${row} />`;
  if (classCode === 23) return html`<${ExtensionCatalogueDetail} row=${row} />`;
  if (classCode === 16) return html`<${ActionDetail} row=${row} />`;
  if (classCode === 0) return html`<${ToolDetail} row=${row} />`;
  if ([1, 2, 3, 10, 50].includes(classCode)) return html`<${PromptBodyDetail} row=${row} />`;
  return html`<${GenericDetail} row=${row} />`;
}

/* ── Class 21 — Recipe ───────────────────────────────────────────────── */

/**
 * `step_descriptions` is stored as `StepDescriptionEntry[]`, each holding a
 * nested `steps[]`; the canonical authoring shape in the v3 docs is a flat
 * array of step objects. Both are flattened to one ordered list here.
 */
function normalizeRecipeSteps(stepDescriptions) {
  if (!Array.isArray(stepDescriptions)) return [];
  const out = [];
  stepDescriptions.forEach((entry, entryIdx) => {
    if (!entry || typeof entry !== "object") return;
    if (Array.isArray(entry.steps) && entry.steps.length > 0) {
      entry.steps.forEach((step, stepIdx) =>
        out.push(toRecipeStep(step, entry.label, `${entryIdx}-${stepIdx}`))
      );
      return;
    }
    out.push(toRecipeStep(entry, entry.label, String(entryIdx)));
  });
  return out;
}

function toRecipeStep(step, groupLabel, key) {
  const source = step && typeof step === "object" ? step : {};
  return {
    key,
    number: source.step_id ?? source.stepnumber ?? source.desc_idx ?? key,
    channel: source.channel ?? source.knowledge ?? null,
    type: source.type ?? null,
    label: source.label ?? source.goal ?? groupLabel ?? null,
    include: Array.isArray(source.include) ? source.include : [],
    toolBindings: Array.isArray(source.tool_bindings) ? source.tool_bindings : [],
    content: source.content ?? null,
    info: source.info ?? null,
  };
}

function RecipeDetail({ row, graph }) {
  const t = useT();
  const llmCallRequired =
    typeof row.steps === "object" && row.steps !== null
      ? row.steps.llm_call_required
      : undefined;
  const steps = normalizeRecipeSteps(row.step_descriptions);
  const variants = Array.isArray(row.variants) ? row.variants : [];
  const audit = graph?.isReady ? recipeChannelAudit(steps, graph.index) : null;
  const outcome = recipeOutcomeStats(row);

  return html`
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2">
        <${TierBadge} llmCallRequired=${llmCallRequired} />
        ${row.trigger &&
        html`<span className="font-mono text-[11px] text-[var(--v2-text-muted)]">
          ${t("componentDetail.recipe.trigger")}: ${row.trigger}
        </span>`}
      </div>

      ${audit?.unmatchedRustSteps > 0 &&
      html`<${ChannelViolation} audit=${audit} />`}

      ${outcome && html`<${RecipeOutcome} outcome=${outcome} />`}

      ${variants.length > 0 &&
      html`<${Section} title=${t("componentDetail.recipe.variants")}>
        <div className="space-y-3">
          ${variants.map(
            (variant, i) => html`<${RecipeVariant} key=${i} variant=${variant} />`
          )}
        </div>
      <//>`}

      <${Section} title=${t("componentDetail.recipe.steps")}>
        ${steps.length === 0
          ? html`<p className="text-xs text-[var(--v2-text-faint)]">
              ${t("componentDetail.recipe.noSteps")}
            </p>`
          : html`<ol className="space-y-2">
              ${steps.map((step) => html`<${RecipeStepRow} key=${step.key} step=${step} />`)}
            </ol>`}
      <//>

      ${!isEmptyValue(row.intent_examples) &&
      html`<${Section} title=${t("componentDetail.intentExamples")}>
        <${IntentExamples} examples=${row.intent_examples} />
      <//>`}

      ${!isEmptyValue(row.prior_knowledge_content) &&
      html`<${Section} title=${t("componentDetail.priorKnowledge")}>
        <${TextBlock} value=${row.prior_knowledge_content} />
      <//>`}
    </div>
  `;
}

/**
 * Q1 Rule 2: a `rust` step only pre-loads a ToolSkill binding — without a
 * following orchestrator PythonCode step to dispatch it, the tool is never
 * invoked. That is a hard authoring error, so it is stated as one.
 */
function ChannelViolation({ audit }) {
  const t = useT();
  return html`
    <div
      className="rounded border border-[color-mix(in_srgb,var(--v2-danger-text)_34%,var(--v2-panel-border))] bg-[var(--v2-danger-soft)] px-3 py-2"
    >
      <div className="flex flex-wrap items-center gap-2">
        <${Badge} tone="danger" label=${t("componentDetail.recipe.q1Rule2")} size="sm" />
        <span className="text-xs text-[var(--v2-danger-text)]">
          ${t("componentDetail.recipe.q1Rule2Detail", {
            rust: String(audit.rustSteps),
            orchestrator: String(audit.orchestratorExecutors),
          })}
        </span>
      </div>
      <p className="mt-1 text-[11px] text-[var(--v2-text-muted)]">
        ${t("componentDetail.recipe.q1Rule2Hint")}
      </p>
    </div>
  `;
}

/**
 * The outcome counters `record_recipe_outcome` maintains on `reborn_recipes`.
 *
 * There is no last-used value on purpose: the table has no `last_used_at`
 * column and `updated_at` moves on edits too, so it would not mean "last
 * used".
 */
function RecipeOutcome({ outcome }) {
  const t = useT();
  const percent =
    typeof outcome.wilsonLower === "number"
      ? `${Math.round(outcome.wilsonLower * 100)}%`
      : null;
  return html`
    <${Section} title=${t("componentDetail.recipe.outcomes")}>
      <${FieldGrid}>
        <${Field} label=${t("componentDetail.recipe.usageCount")} value=${outcome.usage} mono=${true} />
        <${Field} label=${t("componentDetail.recipe.successCount")} value=${outcome.success} mono=${true} />
        <${Field} label=${t("componentDetail.recipe.failureCount")} value=${outcome.failure} mono=${true} />
        <${Field} label=${t("componentDetail.recipe.wilsonLower")} value=${percent} mono=${true} />
        <${Field} label=${t("componentDetail.recipe.rewardTier")} value=${outcome.rewardTier} mono=${true} />
      <//>
      <p className="mt-1 text-[11px] text-[var(--v2-text-faint)]">
        ${t("componentDetail.recipe.noLastUsed")}
      </p>
    <//>
  `;
}

function RecipeVariant({ variant }) {
  const t = useT();
  const patterns = Array.isArray(variant?.variable_patterns) ? variant.variable_patterns : [];
  return html`
    <div className="rounded border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-2">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-mono text-xs font-semibold text-[var(--v2-text-strong)]">
          ${variant?.variant_key ?? t("componentDetail.recipe.unnamedVariant")}
        </span>
        ${variant?.step_link &&
        html`<span className="font-mono text-[10px] text-[var(--v2-text-faint)]">
          ${t("componentDetail.recipe.stepLink")}: ${variant.step_link}
        </span>`}
      </div>
      ${variant?.description &&
      html`<p className="mt-1 text-xs text-[var(--v2-text-muted)]">${variant.description}</p>`}
      ${!isEmptyValue(variant?.intent_examples) &&
      html`<div className="mt-2">
        <${IntentExamples} examples=${variant.intent_examples} />
      </div>`}
      ${patterns.length > 0 &&
      html`<div className="mt-2 space-y-1">
        <span className="font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--v2-text-faint)]">
          ${t("componentDetail.recipe.variablePatterns")}
        </span>
        ${patterns.map(
          (pattern, i) => html`
            <div key=${i} className="flex flex-wrap items-baseline gap-2 text-xs">
              <span className="font-mono text-[var(--v2-accent-text)]">${pattern?.name}</span>
              ${pattern?.pattern &&
              html`<span className="font-mono text-[10px] text-[var(--v2-text-muted)]">
                ${pattern.pattern}
              </span>`}
              ${pattern?.description &&
              html`<span className="text-[var(--v2-text-muted)]">${pattern.description}</span>`}
            </div>
          `
        )}
      </div>`}
    </div>
  `;
}

function RecipeStepRow({ step }) {
  const t = useT();
  return html`
    <li className="rounded border border-[var(--v2-panel-border)] px-3 py-2">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-mono text-[10px] text-[var(--v2-text-faint)]">${step.number}</span>
        ${step.type &&
        html`<${Badge} tone="accent" label=${String(step.type)} size="sm" dot=${false} />`}
        <${ChannelBadge} channel=${step.channel} />
        ${step.label &&
        html`<span className="text-xs text-[var(--v2-text-strong)]">${step.label}</span>`}
      </div>
      ${step.include.length > 0 &&
      html`<div className="mt-1.5 space-y-1">
        <span className="font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--v2-text-faint)]">
          ${t("componentDetail.recipe.include")}
        </span>
        <${ChipList} values=${step.include} tone="accent" />
      </div>`}
      ${step.toolBindings.length > 0 &&
      html`<div className="mt-1.5 space-y-1">
        <span className="font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--v2-text-faint)]">
          ${t("componentDetail.recipe.toolBindings")}
        </span>
        <${ChipList}
          values=${step.toolBindings.map((binding) =>
            typeof binding === "string" ? binding : binding?.tool_name ?? binding?.name ?? binding
          )}
        />
      </div>`}
      ${step.info &&
      html`<p className="mt-1 text-[11px] text-[var(--v2-text-muted)]">${step.info}</p>`}
      ${step.content && html`<div className="mt-1.5"><${CodeBlock} value=${step.content} maxHeight="12rem" /></div>`}
    </li>
  `;
}

/* ── Class 13 — ToolSkill ────────────────────────────────────────────── */

function ToolSkillDetail({ row }) {
  const t = useT();
  return html`
    <div className="space-y-5">
      <${FieldGrid}>
        <${Field} label=${t("componentDetail.toolSkill.toolName")} value=${row.tool_name} mono=${true} />
        <${Field} label=${t("componentDetail.toolSkill.capabilityId")} value=${row.capability_id} mono=${true} />
      <//>

      ${!isEmptyValue(row.param_schema) &&
      html`<${Section} title=${t("componentDetail.toolSkill.paramSchema")}>
        <${CodeBlock} value=${row.param_schema} />
      <//>`}

      ${!isEmptyValue(row.param_template) &&
      html`<${Section} title=${t("componentDetail.toolSkill.paramTemplate")}>
        <${CodeBlock} value=${row.param_template} />
      <//>`}

      ${!isEmptyValue(row.preconditions) &&
      html`<${Section} title=${t("componentDetail.preconditions")}>
        <${TextBlock} value=${row.preconditions} />
      <//>`}

      ${!isEmptyValue(row.error_handling) &&
      html`<${Section} title=${t("componentDetail.errorHandling")}>
        <${TextBlock} value=${row.error_handling} />
      <//>`}

      ${!isEmptyValue(row.content) &&
      html`<${Section} title=${t("componentDetail.toolSkill.body")}>
        <${TextBlock} value=${row.content} />
      <//>`}

      ${!isEmptyValue(row.includes) &&
      html`<${Section} title=${t("componentDetail.includes")}>
        <${ChipList} values=${row.includes} tone="accent" />
      <//>`}

      ${!isEmptyValue(row.intent_examples) &&
      html`<${Section} title=${t("componentDetail.intentExamples")}>
        <${IntentExamples} examples=${row.intent_examples} />
      <//>`}
    </div>
  `;
}

/* ── Class 22 — PythonCode ───────────────────────────────────────────── */

/**
 * Class 22 has no `channel` column: the channel is fixed by the architecture
 * (a PythonCode executor only ever runs in the orchestrator channel) and the
 * dispatched tool is whatever the body calls, so it is read off the body.
 */
function extractDispatches(body) {
  const source = typeof body === "string" ? body : "";
  const found = new Set();
  for (const match of source.matchAll(/host\.([A-Za-z_][A-Za-z0-9_]*)\s*\(/g)) {
    found.add(`host.${match[1]}()`);
  }
  for (const match of source.matchAll(/__execute_action__\(\s*["']([^"']+)["']/g)) {
    found.add(`${match[1]}()`);
  }
  return [...found];
}

function PythonCodeDetail({ row }) {
  const t = useT();
  const dispatches = extractDispatches(row.content);
  return html`
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2">
        <${ChannelBadge} channel="orchestrator" />
        <span className="text-[11px] text-[var(--v2-text-faint)]">
          ${t("componentDetail.pythonCode.channelNote")}
        </span>
      </div>

      ${dispatches.length > 0 &&
      html`<${Section} title=${t("componentDetail.pythonCode.dispatches")}>
        <${ChipList} values=${dispatches} tone="accent" />
      <//>`}

      <${Section} title=${t("componentDetail.pythonCode.body")}>
        ${isEmptyValue(row.content)
          ? html`<p className="text-xs text-[var(--v2-text-faint)]">
              ${t("componentDetail.emptyBody")}
            </p>`
          : html`<${CodeBlock} value=${row.content} maxHeight="32rem" />`}
      <//>

      ${!isEmptyValue(row.includes) &&
      html`<${Section} title=${t("componentDetail.includes")}>
        <${ChipList} values=${row.includes} tone="accent" />
      <//>`}
    </div>
  `;
}

/* ── Class 23 — ExtensionCatalogue ───────────────────────────────────── */

function taskGroupRecipes(group) {
  if (!group || typeof group !== "object") return [];
  const candidates = group.recipes ?? group.recipe_names ?? group.tasks ?? [];
  if (!Array.isArray(candidates)) return [];
  return candidates.map((entry) =>
    typeof entry === "string" ? entry : entry?.recipe ?? entry?.name ?? entry
  );
}

function ExtensionCatalogueDetail({ row }) {
  const t = useT();
  const groups = Array.isArray(row.task_groups) ? row.task_groups : [];
  return html`
    <div className="space-y-5">
      ${!isEmptyValue(row.overview_doc) &&
      html`<${Section} title=${t("componentDetail.catalogue.overviewDoc")}>
        <${TextBlock} value=${row.overview_doc} />
      <//>`}

      <${Section} title=${t("componentDetail.catalogue.taskGroups")}>
        ${groups.length === 0
          ? html`<p className="text-xs text-[var(--v2-text-faint)]">
              ${t("componentDetail.catalogue.noTaskGroups")}
            </p>`
          : html`<div className="space-y-2">
              ${groups.map((group, i) => {
                const recipes = taskGroupRecipes(group);
                return html`
                  <div
                    key=${i}
                    className="rounded border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-2"
                  >
                    <span className="font-mono text-xs font-semibold text-[var(--v2-text-strong)]">
                      ${group?.name ?? group?.title ?? group?.group ?? `#${i}`}
                    </span>
                    ${group?.description &&
                    html`<p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">
                      ${group.description}
                    </p>`}
                    ${recipes.length > 0 &&
                    html`<div className="mt-1.5 space-y-1">
                      <span className="font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--v2-text-faint)]">
                        ${t("componentDetail.catalogue.recipes")}
                      </span>
                      <${ChipList} values=${recipes} tone="accent" />
                    </div>`}
                  </div>
                `;
              })}
            </div>`}
      <//>

      ${!isEmptyValue(row.child_component_ids) &&
      html`<${Section} title=${t("componentDetail.catalogue.childComponents")}>
        <${ChipList} values=${row.child_component_ids} />
      <//>`}

      ${!isEmptyValue(row.intent_index) &&
      html`<${Section} title=${t("componentDetail.catalogue.intentIndex")}>
        <${CodeBlock} value=${row.intent_index} maxHeight="16rem" />
      <//>`}
    </div>
  `;
}

/* ── Classes 1/2/3 (Skill), 10 (Orchestrator), 50 (Scaffold) ─────────── */

function PromptBodyDetail({ row }) {
  const t = useT();
  return html`
    <div className="space-y-5">
      <${Section} title=${t("componentDetail.skill.body")}>
        ${isEmptyValue(row.body)
          ? html`<p className="text-xs text-[var(--v2-text-faint)]">
              ${t("componentDetail.emptyBody")}
            </p>`
          : html`<${TextBlock} value=${row.body} />`}
      <//>

      ${!isEmptyValue(row.prior_knowledge_content) &&
      html`<${Section} title=${t("componentDetail.priorKnowledge")}>
        <${TextBlock} value=${row.prior_knowledge_content} />
      <//>`}

      ${!isEmptyValue(row.allowed_tools) &&
      html`<${Section} title=${t("componentDetail.allowedTools")}>
        <${ChipList} values=${row.allowed_tools} tone="accent" />
      <//>`}

      ${!isEmptyValue(row.keywords) &&
      html`<${Section} title=${t("componentDetail.skill.keywords")}>
        <${ChipList} values=${row.keywords} />
      <//>`}

      ${!isEmptyValue(row.tags) &&
      html`<${Section} title=${t("componentDetail.skill.tags")}>
        <${ChipList} values=${row.tags} />
      <//>`}

      ${!isEmptyValue(row.intent_examples) &&
      html`<${Section} title=${t("componentDetail.intentExamples")}>
        <${IntentExamples} examples=${row.intent_examples} />
      <//>`}

      <${FieldGrid}>
        <${Field}
          label=${t("componentDetail.skill.maxContextTokens")}
          value=${row.max_context_tokens}
          mono=${true}
        />
        <${Field} label=${t("componentDetail.skill.compatibility")} value=${row.compatibility} mono=${true} />
      <//>
    </div>
  `;
}

/* ── Class 16 — Action ───────────────────────────────────────────────── */

function ActionDetail({ row }) {
  const t = useT();
  return html`
    <div className="space-y-5">
      ${!isEmptyValue(row.steps) &&
      html`<${Section} title=${t("componentDetail.action.steps")}>
        <${CodeBlock} value=${row.steps} />
      <//>`}

      ${!isEmptyValue(row.preconditions) &&
      html`<${Section} title=${t("componentDetail.preconditions")}>
        <${CodeBlock} value=${row.preconditions} maxHeight="16rem" />
      <//>`}

      ${!isEmptyValue(row.error_handling) &&
      html`<${Section} title=${t("componentDetail.errorHandling")}>
        <${CodeBlock} value=${row.error_handling} maxHeight="16rem" />
      <//>`}

      ${!isEmptyValue(row.param_schema) &&
      html`<${Section} title=${t("componentDetail.toolSkill.paramSchema")}>
        <${CodeBlock} value=${row.param_schema} maxHeight="16rem" />
      <//>`}

      ${!isEmptyValue(row.allowed_tools) &&
      html`<${Section} title=${t("componentDetail.allowedTools")}>
        <${ChipList} values=${row.allowed_tools} tone="accent" />
      <//>`}

      <${FieldGrid}>
        <${Field} label=${t("componentDetail.action.timeout")} value=${row.timeout_secs} mono=${true} />
      <//>
    </div>
  `;
}

/* ── Class 0 — Tool ──────────────────────────────────────────────────── */

function ToolDetail({ row }) {
  const t = useT();
  return html`
    <div className="space-y-5">
      <${FieldGrid}>
        <${Field} label=${t("componentDetail.tool.capabilityId")} value=${row.capability_id} mono=${true} />
        <${Field} label=${t("componentDetail.tool.effectType")} value=${row.effect_type} mono=${true} />
      <//>

      ${!isEmptyValue(row.param_schema) &&
      html`<${Section} title=${t("componentDetail.toolSkill.paramSchema")}>
        <${CodeBlock} value=${row.param_schema} />
      <//>`}

      ${!isEmptyValue(row.param_template) &&
      html`<${Section} title=${t("componentDetail.toolSkill.paramTemplate")}>
        <${CodeBlock} value=${row.param_template} maxHeight="16rem" />
      <//>`}

      ${!isEmptyValue(row.preconditions) &&
      html`<${Section} title=${t("componentDetail.preconditions")}>
        <${TextBlock} value=${row.preconditions} />
      <//>`}

      ${!isEmptyValue(row.error_handling) &&
      html`<${Section} title=${t("componentDetail.errorHandling")}>
        <${TextBlock} value=${row.error_handling} />
      <//>`}

      ${!isEmptyValue(row.prior_knowledge_content) &&
      html`<${Section} title=${t("componentDetail.priorKnowledge")}>
        <${TextBlock} value=${row.prior_knowledge_content} />
      <//>`}
    </div>
  `;
}

function GenericDetail({ row }) {
  const t = useT();
  return html`
    <${Section} title=${t("componentDetail.rawRow")}>
      <${CodeBlock} value=${row} />
    <//>
  `;
}

/**
 * Both directions of a component's wiring.
 *
 * A Recipe's `include[]`, a ToolSkill's `tool_name` and any
 * `dependency_registry` entry are all edges of one graph, so "what this
 * includes" and "what includes this" are the same lookup read from either
 * end. An edge whose target is not in the catalog is shown as unresolved
 * rather than dropped — a dangling include is drift, not noise.
 */
function CrossReferences({ graph, id }) {
  const t = useT();
  if (graph?.isError) {
    return html`
      <${Section} title=${t("componentDetail.references")}>
        <p className="text-xs text-[var(--v2-text-faint)]">
          ${t("componentDetail.referencesFailed")}
        </p>
      <//>
    `;
  }
  if (!graph?.isReady) return null;

  const { references, referencedBy } = componentRefs(graph.index, id);
  if (references.length === 0 && referencedBy.length === 0) {
    return html`
      <${Section} title=${t("componentDetail.references")}>
        <p className="text-xs text-[var(--v2-text-faint)]">${t("componentDetail.noReferences")}</p>
      <//>
    `;
  }

  return html`
    <div className="space-y-3">
      ${references.length > 0 &&
      html`<${Section} title=${t("componentDetail.references")}>
        <${ReferenceList} refs=${references} />
      <//>`}
      ${referencedBy.length > 0 &&
      html`<${Section} title=${t("componentDetail.referencedBy")}>
        <${ReferenceList} refs=${referencedBy} />
      <//>`}
    </div>
  `;
}

function ReferenceList({ refs }) {
  const t = useT();
  return html`
    <ul className="space-y-1">
      ${refs.map(
        (ref, i) => html`
          <li
            key=${`${ref.id}-${ref.kind}-${ref.stepRef ?? i}`}
            className="flex flex-wrap items-center gap-2 rounded border border-[var(--v2-panel-border)] px-2 py-1.5"
          >
            <${Badge}
              tone=${ref.node ? "accent" : "danger"}
              label=${ref.node ? classLabel(ref.node.class_code) : t("componentDetail.unresolvedRef")}
              size="sm"
              dot=${false}
            />
            <span className="font-mono text-xs text-[var(--v2-text-strong)] break-all">
              ${ref.node?.name ?? ref.id}
            </span>
            <${ChannelBadge} channel=${ref.channel} />
            <span className="font-mono text-[10px] text-[var(--v2-text-faint)]">
              ${t(`componentDetail.refKind.${ref.kind}`)}
            </span>
            ${ref.stepRef &&
            html`<span className="font-mono text-[10px] text-[var(--v2-text-faint)]">
              ${ref.stepRef}
            </span>`}
            ${ref.stepLabel &&
            html`<span className="text-[11px] text-[var(--v2-text-muted)]">${ref.stepLabel}</span>`}
          </li>
        `
      )}
    </ul>
  `;
}

function CommonFooter({ row, classCode, id, graph }) {
  const t = useT();
  return html`
    <div className="space-y-2 border-t border-[var(--v2-panel-border)] pt-3">
      <${CrossReferences} graph=${graph} id=${id} />
      ${!isEmptyValue(row.consumer_tags) &&
      html`<div className="space-y-1">
        <span className="font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--v2-text-faint)]">
          ${t("componentDetail.consumerTags")}
        </span>
        <${ConsumerTags} tags=${row.consumer_tags} />
      </div>`}
      ${!isEmptyValue(row.dependency_registry) &&
      html`<${Section} title=${t("componentDetail.dependencyRegistry")}>
        <${CodeBlock} value=${row.dependency_registry} maxHeight="12rem" />
      <//>`}
      <${FieldGrid}>
        <${Field} label=${t("componentDetail.id")} value=${id} mono=${true} />
        <${Field} label=${t("componentDetail.classCode")} value=${String(classCode)} mono=${true} />
        <${Field} label=${t("componentDetail.source")} value=${row.source} mono=${true} />
        <${Field} label=${t("componentDetail.promptUid")} value=${row.prompt_uid} mono=${true} />
        <${Field} label=${t("componentDetail.version")} value=${row.version} mono=${true} />
      <//>
    </div>
  `;
}
