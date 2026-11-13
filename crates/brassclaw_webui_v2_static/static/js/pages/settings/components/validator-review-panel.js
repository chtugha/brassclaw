/**
 * ValidatorReviewPanel — B3-09 product integration.
 *
 * Shows the original immutable subject for one class-21 (Recipe) or class-22
 * (PythonCode) validator queue item. This is an inspection-only view: the
 * human Q2 decision path (approve/reject) is not wired here for either class.
 * The existing QueueRow approve/reject buttons continue to serve class 21
 * through the legacy queue graduation path; that path does not send the
 * displayed submission_id or subject_checksum and does not satisfy the
 * immutable-approval contract required by the connected writer.
 *
 * Rules (what this panel does):
 * - Loads the retained subject via fetchComponentReviewSubmission;
 *   never from a latest component-row lookup.
 * - Displays subject_checksum, candidate identity, component bytes and
 *   review selection so the operator can see what was submitted.
 * - Shows the Q1 work status when present.
 * - Shows an inspection-only notice for q2_manual items to make clear
 *   that approve/reject is not available here.
 * - Does not publish matching or MCP commands.
 * - Class 10 (Orchestrator) and 50 (Scaffold) are handled by the existing
 *   LLM-audit-gated QueueRow and are excluded from this panel.
 */
import { React, html } from "../../../lib/html.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery } from "@tanstack/react-query";
import { fetchComponentReviewSubmission } from "../lib/settings-api.js";
import { Section, Field, FieldGrid, CodeBlock } from "./component-detail-primitives.js";

/** Classes shown in this panel (subject inspection only). */
const REVIEW_PANEL_CLASSES = new Set([21, 22]);

/**
 * Map a ReviewWorkStatus snake_case string to an i18n key.
 * Falls back to the raw string so new statuses from the server are visible.
 */
function workStatusKey(status) {
  return `componentSubmission.work.${status}`;
}

/**
 * Tone for the review status badge.
 */
function reviewStatusTone(status) {
  if (!status) return "muted";
  if (status === "approved" || status === "active" || status === "validated") return "positive";
  if (status === "rejected" || status === "failed" || status === "garbage") return "danger";
  if (status === "pending" || status === "auto_passed" || status === "awaiting") return "warning";
  return "muted";
}

/**
 * Render one component revision reference inline.
 * Used directly in ValidatorReviewPanelInner — no sub-component boundary.
 */
function revisionRefRow(t, label, revRef) {
  if (!revRef) return null;
  return html`
    <div className="space-y-1">
      <span className="font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--v2-text-faint)]">
        ${label}
      </span>
      <div className="font-mono text-xs break-all text-[var(--v2-text-muted)]">
        <span>${revRef.uuid}</span>
        <span className="ml-2">v${revRef.version}</span>
        <span className="ml-2 text-[var(--v2-text-faint)]">${revRef.checksum}</span>
        ${revRef.class_code != null
          ? html`<span className="ml-2 text-[var(--v2-text-faint)]">
              ${t("validatorReview.classCode")}: ${revRef.class_code}
            </span>`
          : null}
      </div>
    </div>
  `;
}

/**
 * Inner implementation — only rendered when class_code is 21 or 22.
 * All hooks are at the top level, before any conditional returns, so the
 * outer guard wrapper can safely return null for excluded classes without
 * changing the hook call sequence here.
 */
function ValidatorReviewPanelInner({ item }) {
  const t = useT();

  const submissionId = item?.original_review?.submission_id;
  const hasSubmission = typeof submissionId === "string" && submissionId.length > 0;

  const query = useQuery({
    queryKey: ["settings", "validator-review-submission", submissionId],
    queryFn: () => fetchComponentReviewSubmission(submissionId),
    enabled: hasSubmission,
    staleTime: 30_000,
  });

  if (!hasSubmission) {
    return html`
      <${Section} title=${t("validatorReview.title")}>
        <p className="text-xs text-[var(--v2-text-faint)]">
          ${t("validatorReview.noSubmission")}
        </p>
      <//>
    `;
  }

  if (query.isLoading) {
    return html`
      <${Section} title=${t("validatorReview.title")}>
        <div className="space-y-2">
          <div className="h-3 w-48 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
          <div className="h-20 w-full animate-pulse rounded bg-[var(--v2-surface-muted)]" />
        </div>
      <//>
    `;
  }

  if (query.isError) {
    return html`
      <${Section} title=${t("validatorReview.title")}>
        <p className="text-xs text-[var(--v2-danger-text)]">
          ${t("validatorReview.loadFailed", {
            message: query.error?.message ?? String(query.error),
          })}
        </p>
      <//>
    `;
  }

  const view = query.data;
  const receipt = view.receipt;
  const isHistorical = receipt.review_status && receipt.review_status !== "pending";

  return html`
    <div className="space-y-5">

      <${Section} title=${t("validatorReview.title")}>
        <p className="text-[11px] text-[var(--v2-text-faint)]">
          ${t("validatorReview.scope")}
        </p>
        <div className="mt-1 flex items-center gap-2">
          <${Badge}
            tone=${reviewStatusTone(receipt.review_status)}
            label=${receipt.review_status ?? "unknown"}
            size="sm"
          />
          <${Badge}
            tone=${reviewStatusTone(item.validation_status)}
            label=${item.validation_status ?? "unknown"}
            size="sm"
          />
        </div>
      <//>

      ${isHistorical
        ? html`<div className="rounded border border-[color-mix(in_srgb,var(--v2-warning-text)_34%,var(--v2-panel-border))] bg-[var(--v2-surface-soft)] px-3 py-2">
            <span className="text-xs text-[var(--v2-warning-text)]">
              ${t("validatorReview.historicalSubject")}: ${receipt.review_status}
            </span>
          </div>`
        : null}

      <${FieldGrid}>
        <${Field}
          label=${t("validatorReview.subjectChecksum")}
          value=${receipt.subject_checksum}
          mono=${true}
        />
        <${Field}
          label=${t("validatorReview.reviewStatus")}
          value=${receipt.review_status}
          mono=${true}
        />
        <${Field}
          label=${t("validatorReview.submissionId")}
          value=${receipt.submission_id}
          mono=${true}
        />
        ${receipt.catalogue_activated
          ? html`<${Field}
              label=${t("validatorReview.catalogueActivated")}
              value=${t("validatorReview.yes")}
            />`
          : null}
      <//>

      ${revisionRefRow(t, t("validatorReview.candidate"), receipt.candidate)}

      ${receipt.base
        ? revisionRefRow(t, t("validatorReview.base"), receipt.base)
        : null}

      ${Array.isArray(receipt.dependencies) && receipt.dependencies.length > 0
        ? html`<${Section} title=${t("validatorReview.dependencies")}>
            ${receipt.dependencies.map((dep, i) =>
              revisionRefRow(t, String(i + 1), dep)
            )}
          <//>` : null}

      ${receipt.function_partner
        ? html`<${Section} title=${t("validatorReview.functionPartner")}>
            <span className="font-mono text-xs text-[var(--v2-text-faint)]">
              ${receipt.function_partner.changed
                ? t("validatorReview.partnerChanged")
                : t("validatorReview.partnerContext")}
            </span>
            ${revisionRefRow(
              t,
              t("validatorReview.partnerRef"),
              receipt.function_partner.reference
            )}
          <//>` : null}

      ${view.work
        ? html`<${Section} title=${t("validatorReview.workStatus")}>
            <span className="text-xs text-[var(--v2-text-muted)]">
              ${t(workStatusKey(view.work.status))}
            </span>
          <//>` : null}

      <${Section} title=${t("validatorReview.subjectBytes")}>
        <${CodeBlock} value=${view.subject_bytes} maxHeight="12rem" />
      <//>

      ${view.component_bytes.map((bytes, i) => html`
        <details key=${i} className="rounded border border-[var(--v2-panel-border)] px-3 py-2">
          <summary className="cursor-pointer text-xs text-[var(--v2-text-muted)]">
            ${t("validatorReview.componentBytes", { index: i + 1 })}
          </summary>
          <${CodeBlock} value=${bytes} maxHeight="20rem" />
        </details>
      `)}

      ${view.review_selection_bytes
        ? html`<details className="rounded border border-[var(--v2-panel-border)] px-3 py-2">
            <summary className="cursor-pointer text-xs text-[var(--v2-text-muted)]">
              ${t("validatorReview.reviewSelection")}
            </summary>
            <${CodeBlock} value=${view.review_selection_bytes} maxHeight="12rem" />
          </details>` : null}

      ${item.queue_code === "q2_manual"
        ? html`<${Section} title=${t("validatorReview.reviewActions")}>
            <p className="text-xs text-[var(--v2-text-muted)]">
              ${t("validatorReview.actionNotConnected")}
            </p>
          <//>` : null}

    </div>
  `;
}

/**
 * Public guard wrapper. Returns null for excluded classes without entering
 * any hook scope, so the inner component's hooks are always called in a
 * stable order inside their own React component boundary.
 *
 * @param {object} item - Queue row item from fetchValidationQueue.
 */
export function ValidatorReviewPanel({ item }) {
  if (!REVIEW_PANEL_CLASSES.has(item?.class_code)) return null;
  return html`<${ValidatorReviewPanelInner} item=${item} />`;
}
