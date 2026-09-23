import { html } from "../../../lib/html.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";

export function statusTone(validationStatus) {
  if (validationStatus === "validated") return "positive";
  if (validationStatus === "rejected") return "danger";
  if (validationStatus === "pending") return "warning";
  return "muted";
}

export function isEmptyValue(value) {
  if (value === null || value === undefined) return true;
  if (typeof value === "string") return value.trim() === "";
  if (Array.isArray(value)) return value.length === 0;
  if (typeof value === "object") return Object.keys(value).length === 0;
  return false;
}

export function stringifyValue(value) {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch (_) {
    return String(value);
  }
}

export function Section({ title, children }) {
  return html`
    <section className="space-y-2">
      <h4 className="font-mono text-[10px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${title}
      </h4>
      ${children}
    </section>
  `;
}

export function Field({ label, value, mono = false }) {
  if (isEmptyValue(value)) return null;
  const valueClass = mono
    ? "font-mono text-xs text-[var(--v2-text-strong)] break-all"
    : "text-xs text-[var(--v2-text-strong)] whitespace-pre-wrap";
  return html`
    <div className="flex flex-col gap-0.5 sm:flex-row sm:gap-3">
      <span className="shrink-0 font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--v2-text-faint)] sm:w-40 sm:pt-0.5">
        ${label}
      </span>
      <span className=${valueClass}>${value}</span>
    </div>
  `;
}

export function FieldGrid({ children }) {
  return html`<div className="space-y-1.5">${children}</div>`;
}

export function CodeBlock({ value, maxHeight = "24rem" }) {
  if (isEmptyValue(value)) return null;
  return html`
    <div
      className="overflow-auto whitespace-pre rounded border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-2 font-mono text-[11px] leading-5 text-[var(--v2-text-strong)]"
      style=${{ maxHeight }}
    >
      ${stringifyValue(value)}
    </div>
  `;
}

export function TextBlock({ value }) {
  if (isEmptyValue(value)) return null;
  return html`
    <div className="max-h-96 overflow-auto whitespace-pre-wrap rounded border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-2 text-xs leading-5 text-[var(--v2-text-strong)]">
      ${String(value)}
    </div>
  `;
}

export function ChipList({ values, tone = "muted" }) {
  const list = (values ?? []).filter((v) => !isEmptyValue(v));
  if (list.length === 0) return null;
  const base =
    "inline-flex items-center rounded px-1.5 py-0.5 font-mono text-[10px] border";
  const toneClass =
    tone === "accent"
      ? "border-[var(--v2-accent-border)] bg-[var(--v2-accent-soft)] text-[var(--v2-accent-text)]"
      : "border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] text-[var(--v2-text-muted)]";
  return html`
    <div className="flex flex-wrap gap-1">
      ${list.map(
        (value, i) => html`
          <span key=${`${value}-${i}`} className=${`${base} ${toneClass}`}>
            ${typeof value === "string" ? value : stringifyValue(value)}
          </span>
        `
      )}
    </div>
  `;
}

/**
 * Consumer-tag chip with greyed-out rendering per spec §3.9: while a
 * component carries `05:validator` (in queue), non-validator chips render
 * greyed but stay visible so operators can see the target audience.
 */
export function ConsumerTags({ tags }) {
  const list = tags ?? [];
  if (list.length === 0) return null;
  const isInQueue = list.includes("05:validator");
  const base =
    "inline-flex items-center rounded px-1.5 py-0.5 font-mono text-[10px] border";
  return html`
    <div className="flex flex-wrap gap-1">
      ${list.map((tag) => {
        const greyed = isInQueue && tag !== "05:validator";
        const style = greyed
          ? `${base} border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] text-[var(--v2-text-faint)] opacity-50`
          : tag === "05:validator"
          ? `${base} border-amber-400/40 bg-amber-500/10 text-amber-300`
          : `${base} border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] text-[var(--v2-text-muted)]`;
        return html`<span key=${tag} className=${style}>${tag}</span>`;
      })}
    </div>
  `;
}

export function IntentExamples({ examples }) {
  const t = useT();
  const list = examples ?? [];
  if (list.length === 0) return null;
  return html`
    <ul className="space-y-1">
      ${list.map((example, i) => {
        const text =
          typeof example === "string"
            ? example
            : example?.input ?? stringifyValue(example);
        const cls = typeof example === "object" ? example?.class : null;
        return html`
          <li
            key=${`${text}-${i}`}
            className="flex items-baseline gap-2 text-xs text-[var(--v2-text-strong)]"
          >
            <span className="text-[var(--v2-text-faint)]">›</span>
            <span className="min-w-0 break-words">${text}</span>
            ${cls &&
            html`<span className="shrink-0 font-mono text-[10px] text-[var(--v2-text-faint)]">
              ${t("componentDetail.intentClass", { class: cls })}
            </span>`}
          </li>
        `;
      })}
    </ul>
  `;
}

export function TierBadge({ llmCallRequired }) {
  const t = useT();
  if (llmCallRequired === true) {
    return html`<${Badge} tone="warning" label=${t("componentDetail.tier1")} size="sm" />`;
  }
  if (llmCallRequired === false) {
    return html`<${Badge} tone="positive" label=${t("componentDetail.tier0")} size="sm" />`;
  }
  return html`<${Badge} tone="muted" label=${t("componentDetail.tierUnknown")} size="sm" />`;
}

export function ChannelBadge({ channel }) {
  if (isEmptyValue(channel)) return null;
  const value = String(channel).toLowerCase();
  const tone = value === "rust" ? "copper" : value === "orchestrator" ? "info" : "muted";
  return html`<${Badge} tone=${tone} label=${value} size="sm" dot=${false} />`;
}
