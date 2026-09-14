/**
 * InstalledTab — shows the recipe-system ExtensionCatalogue entries (class 23)
 * that are seeded into the orchestrator at boot.
 *
 * Data source: GET /api/settings/extensions  → { items: SettingsComponentSummary[] }
 *
 * The catalogue IDs map to three logical sections:
 *   • Core capabilities  — builtin-filesystem / -network / -memory / -process / -management
 *   • Workflow domains   — ext-coding, ext-commit, ext-github, ext-code-review,
 *                          ext-qa-review, ext-security-review, ext-plan-mode
 *   • Integrations       — ext-zencoder, ext-doc-sync
 *
 * Individual per-tool sub-catalogues (ext-read-file, ext-http, …) are filtered
 * out — they are visible in the Settings › Extensions detail view.
 */
import { React, html } from "../../../lib/html.js";
import { useQuery } from "@tanstack/react-query";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { fetchSettingsExtensions } from "../../settings/lib/settings-api.js";

// ── Section membership ────────────────────────────────────────────────────────

const CORE_IDS = new Set([
  "builtin-filesystem",
  "builtin-network",
  "builtin-memory",
  "builtin-process",
  "builtin-management",
]);

const WORKFLOW_IDS = new Set([
  "ext-coding",
  "ext-commit",
  "ext-github",
  "ext-code-review",
  "ext-qa-review",
  "ext-security-review",
  "ext-plan-mode",
]);

const INTEGRATION_IDS = new Set([
  "ext-zencoder",
  "ext-doc-sync",
]);

// Human-readable fallback labels for well-known IDs (backend descriptions are
// used when present; these are the last-resort fallbacks).
const KNOWN_DESCRIPTIONS = {
  "builtin-filesystem":    "File read, write, list, glob, grep, and patch operations.",
  "builtin-network":       "HTTP requests, web search, and file-saving from URLs.",
  "builtin-memory":        "Workspace memory: search, read, write, and tree operations.",
  "builtin-process":       "Shell execution, child-agent delegation, and trigger management.",
  "builtin-management":    "Time, JSON processing, and skill lifecycle utilities.",
  "ext-coding":            "Coding best practices — disciplined edits, search-before-write, minimal diffs.",
  "ext-commit":            "Guided git commit workflow: inspect staged changes, draft message, confirm, commit.",
  "ext-github":            "GitHub REST API — issues, pull requests, search, and authenticated user.",
  "ext-code-review":       "Paranoid architect code review for local diffs or GitHub pull requests.",
  "ext-qa-review":         "QA and test coverage analysis: gaps, edge cases, regression risks.",
  "ext-security-review":   "OWASP-based security audit for code changes and pull requests.",
  "ext-plan-mode":         "Structured task planning stored as memory documents at plans/<slug>.md.",
  "ext-zencoder":          "Delegate coding tasks to the Zencoder/Zenflow cloud AI agent pipeline.",
  "ext-doc-sync":          "Doc-conversion pipeline: sync markdown docs into the agent knowledge base.",
};

const KNOWN_LABELS = {
  "builtin-filesystem":    "Filesystem",
  "builtin-network":       "Network",
  "builtin-memory":        "Memory",
  "builtin-process":       "Process",
  "builtin-management":    "Management",
  "ext-coding":            "Coding",
  "ext-commit":            "Git Commit",
  "ext-github":            "GitHub",
  "ext-code-review":       "Code Review",
  "ext-qa-review":         "QA Review",
  "ext-security-review":   "Security Review",
  "ext-plan-mode":         "Plan Mode",
  "ext-zencoder":          "Zencoder / Zenflow",
  "ext-doc-sync":          "Doc Sync",
};

// ── Section order ─────────────────────────────────────────────────────────────

const SECTION_ORDER = ["core", "workflow", "integration"];

function sectionFor(id) {
  if (CORE_IDS.has(id)) return "core";
  if (WORKFLOW_IDS.has(id)) return "workflow";
  if (INTEGRATION_IDS.has(id)) return "integration";
  return null;
}

// ── Card ──────────────────────────────────────────────────────────────────────

function ExtCatalogueCard({ item, t }) {
  const label = KNOWN_LABELS[item.id] || item.name;
  const description = item.description || KNOWN_DESCRIPTIONS[item.id] || "";
  const isValidated = item.validation_status === "validated";

  return html`
    <div
      className="flex flex-col rounded-[14px] border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] p-4"
    >
      <div className="flex items-start gap-2">
        <${Badge}
          tone=${isValidated ? "positive" : "neutral"}
          label=${isValidated ? t("ext.installed.active") : item.validation_status}
          size="sm"
        />
        <span className="min-w-0 flex-1 truncate text-sm font-semibold text-[var(--v2-text-strong)]">
          ${label}
        </span>
      </div>

      ${item.id && html`
        <div className="mt-1 font-mono text-[10px] text-[var(--v2-text-faint)]">
          ${item.id}
        </div>
      `}

      ${description && html`
        <p className="mt-2 line-clamp-3 text-xs leading-5 text-[var(--v2-text-muted)]">
          ${description}
        </p>
      `}
    </div>
  `;
}

// ── Section ───────────────────────────────────────────────────────────────────

function ExtSection({ sectionKey, items, t }) {
  const titleKey = `ext.installed.section.${sectionKey}`;
  return html`
    <div className="v2-panel rounded-[18px] p-5 sm:p-6">
      <h3 className="mb-4 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t(titleKey)}
      </h3>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-3">
        ${items.map(
          (item) => html`
            <${ExtCatalogueCard} key=${item.id} item=${item} t=${t} />
          `
        )}
      </div>
    </div>
  `;
}

// ── Skeleton ──────────────────────────────────────────────────────────────────

function InstalledSkeleton() {
  return html`
    <div className="space-y-5 animate-pulse">
      <div className="h-48 rounded-[18px] bg-[var(--v2-surface-soft)]" />
      <div className="h-64 rounded-[18px] bg-[var(--v2-surface-soft)]" />
      <div className="h-32 rounded-[18px] bg-[var(--v2-surface-soft)]" />
    </div>
  `;
}

// ── Main component ────────────────────────────────────────────────────────────

export function InstalledTab() {
  const t = useT();

  const query = useQuery({
    queryKey: ["settings-extensions-catalogue"],
    queryFn: fetchSettingsExtensions,
    staleTime: 60_000,
  });

  if (query.isLoading) {
    return html`<${InstalledSkeleton} />`;
  }

  if (query.error) {
    return html`
      <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
        ${t("ext.installed.failedLoad", { message: query.error.message || String(query.error) })}
      </div>
    `;
  }

  const allItems = query.data?.items || [];

  // Only show domain-level catalogues (filter out per-tool sub-catalogues).
  const known = allItems.filter((item) => sectionFor(item.id) !== null);

  if (known.length === 0) {
    return html`
      <div className="v2-panel rounded-[18px] p-6 sm:p-8">
        <h3 className="text-lg font-semibold text-[var(--v2-text-strong)]">
          ${t("ext.installed.emptyTitle")}
        </h3>
        <p className="mt-2 max-w-md text-sm leading-6 text-[var(--v2-text-muted)]">
          ${t("ext.installed.emptyDesc")}
        </p>
      </div>
    `;
  }

  // Group by section preserving SECTION_ORDER.
  const bySection = {};
  for (const item of known) {
    const s = sectionFor(item.id);
    if (!bySection[s]) bySection[s] = [];
    bySection[s].push(item);
  }

  return html`
    <div className="space-y-5">
      ${SECTION_ORDER.filter((s) => bySection[s]?.length > 0).map(
        (s) => html`
          <${ExtSection} key=${s} sectionKey=${s} items=${bySection[s]} t=${t} />
        `
      )}
    </div>
  `;
}
