/**
 * ValidatorReviewPanel — B3-09 product integration.
 *
 * Shows the original immutable subject and evidence for one class-21 (Recipe)
 * or class-22 (PythonCode) validator queue item. The operator sees the same
 * subject the backend holds — identified by the original subject_checksum —
 * before deciding to approve or reject.
 *
 * Rules:
 * - Loads from B1's retained immutable subject via fetchComponentReviewSubmission;
 *   never from a latest component-row lookup.
 * - Approve/reject route through the existing typed validate/rejectComponent API
 *   (B1 class/profile dispatcher); never to a Skill-only association endpoint.
 * - A stale-view/base conflict (409) shows the new backend state and blocks
 *   re-approval until the operator explicitly re-prepares.
 * - Retry after lost ACK reuses the original item class/id; a new submission
 *   creates a new ID and uses ComponentSubmissionPanel.
 * - Admin catalogue inspection does NOT publish matching or MCP commands.
 * - Class 10 (Orchestrator) and 50 (Scaffold) are never shown here — they go
 *   through the LLM-audit-gated path in the existing QueueRow.
 * - Private class 05:validator route exclusion: this panel is for class 21/22
 *   only; it does not list validators or publish their intents.
 */
import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  fetchComponentReviewSubmission,
  validateComponent,
  rejectComponent,
} from "../lib/settings-api.js";
import { Section, Field, FieldGrid, CodeBlock } from "./component-detail-primitives.js";

/** Classes for which this panel supplies approve/reject actions (B3-09 req 4). */
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
 * draft / pending → warning, approved/active → positive, rejected/failed → danger.
 */
function reviewStatusTone(status) {
  if (!status) return "muted";
  if (status === "approved" || status === "active" || status === "validated") return "positive";
  if (status === "rejected" || status === "failed" || status === "garbage") return "danger";
  if (status === "pending" || status === "auto_passed" || status === "awaiting") return "warning";
  return "muted";
}

/**
 * Classes for which the backend Q2 approval writer is currently connected.
 * Class 22 (PythonCode) is tracked under B1-06 and is NOT connected yet;
 * it shows the subject for inspection but no approve/reject buttons.
 */
const CONNECTED_Q2_CLASSES = new Set([21]);

/**
 * Returns true when the item is eligible for the Q2 review section —
 * q2_manual queue with a linked submission AND a connected backend writer.
 * The subject may still be unbound (prerequisitesMissing = true) in which
 * case the action buttons are disabled but the prerequisite warning is shown.
 */
function isEligibleForReview(item) {
  return (
    item.queue_code === "q2_manual" &&
    CONNECTED_Q2_CLASSES.has(item.class_code) &&
    typeof item.original_review?.submission_id === "string" &&
    item.original_review.submission_id.length > 0
  );
}

/**
 * Returns true only when the original_review state is "bound" — meaning all
 * prerequisites are met and approve/reject buttons are enabled.
 */
function isReviewable(item) {
  return isEligibleForReview(item) && item.original_review?.state === "bound";
}

/**
 * Render one component revision reference inline.
 * Used directly in ValidatorReviewPanel — no sub-component boundary.
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
 * ValidatorReviewPanel — public export.
 *
 * Renders nothing for classes other than 21 and 22, or when the queue item
 * has no retained submission to display. Silently returns null for private
 * validators (05:validator) since those are excluded by the queue filter.
 *
 * All content is rendered directly in this function (no sub-component calls)
 * so static-vm test traversal can reach all displayed values.
 *
 * @param {object} item - Queue row item from fetchValidationQueue.
 * @param {function?} onDone - Called after a successful approve/reject.
 */
/**
 * Inner implementation — only rendered when class_code is 21 or 22.
 * All hooks are at the top level, before any conditional returns.
 */
function ValidatorReviewPanelInner({ item, onDone }) {
  const t = useT();
  const queryClient = useQueryClient();
  const [rejectOpen, setRejectOpen] = React.useState(false);
  const [feedback, setFeedback] = React.useState("");
  const [actionError, setActionError] = React.useState("");

  const submissionId = item?.original_review?.submission_id;
  const hasSubmission = typeof submissionId === "string" && submissionId.length > 0;

  const query = useQuery({
    queryKey: ["settings", "validator-review-submission", submissionId],
    queryFn: () => fetchComponentReviewSubmission(submissionId),
    enabled: hasSubmission,
    staleTime: 30_000,
  });

  const invalidateQueue = () => {
    queryClient.invalidateQueries({ queryKey: ["settings", "validation-queue"] });
    queryClient.invalidateQueries({ queryKey: ["settings", "validation-queue", "count"] });
    if (onDone) onDone();
  };

  const approveMutation = useMutation({
    mutationFn: () => validateComponent(item.class_code, item.id),
    onSuccess: invalidateQueue,
    onError: (err) => {
      setActionError(
        err.status === 409
          ? t("validatorReview.staleViewConflict")
          : t("validationQueue.validateError", { message: err.message })
      );
    },
  });

  const rejectMutation = useMutation({
    mutationFn: (reason) => rejectComponent(item.class_code, item.id, reason),
    onSuccess: () => {
      setRejectOpen(false);
      setFeedback("");
      invalidateQueue();
    },
    onError: (err) => {
      setActionError(
        err.status === 409
          ? t("validatorReview.staleViewConflict")
          : t("validationQueue.rejectError", { message: err.message })
      );
    },
  });

  const isBusy = approveMutation.isPending || rejectMutation.isPending;
  const trimmedFeedback = feedback.trim();
  // canShowActions: show approve/reject only when the backend writer is
  // connected (CONNECTED_Q2_CLASSES, currently class 21 only). Class 22 shows
  // the subject for inspection but no action buttons until B1-06 connects.
  const canShowActions = isEligibleForReview(item);
  const prerequisitesMissing = item.original_review?.state !== "bound";
  // notYetConnected: show the "writer not connected" notice for classes that
  // are in the subject panel (REVIEW_PANEL_CLASSES) but not wired for Q2.
  const notYetConnected =
    item.queue_code === "q2_manual" &&
    REVIEW_PANEL_CLASSES.has(item.class_code) &&
    !CONNECTED_Q2_CLASSES.has(item.class_code);

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

      ${/* Title + scope */null}
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

      ${/* Historical subject warning */
        isHistorical
          ? html`<div className="rounded border border-[color-mix(in_srgb,var(--v2-warning-text)_34%,var(--v2-panel-border))] bg-[var(--v2-surface-soft)] px-3 py-2">
              <span className="text-xs text-[var(--v2-warning-text)]">
                ${t("validatorReview.historicalSubject")}: ${receipt.review_status}
              </span>
            </div>`
          : null}

      ${/* Subject identity fields */null}
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

      ${/* Candidate, base, dependencies */
        revisionRefRow(t, t("validatorReview.candidate"), receipt.candidate)}

      ${receipt.base
        ? revisionRefRow(t, t("validatorReview.base"), receipt.base)
        : null}

      ${Array.isArray(receipt.dependencies) && receipt.dependencies.length > 0
        ? html`<${Section} title=${t("validatorReview.dependencies")}>
            ${receipt.dependencies.map((dep, i) =>
              revisionRefRow(t, String(i + 1), dep)
            )}
          <//>` : null}

      ${/* Function partner */
        receipt.function_partner
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

      ${/* Q1 work status */
        view.work
          ? html`<${Section} title=${t("validatorReview.workStatus")}>
              <span className="text-xs text-[var(--v2-text-muted)]">
                ${t(workStatusKey(view.work.status))}
              </span>
            <//>` : null}

      ${/* Subject document */null}
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

      ${notYetConnected
          ? html`<${Section} title=${t("validatorReview.reviewActions")}>
              <p className="text-xs text-[var(--v2-text-muted)]">
                ${t("validatorReview.actionNotConnected")}
              </p>
            <//>` : null}

      ${canShowActions
          ? html`<${Section} title=${t("validatorReview.reviewActions")}>
              <div className="space-y-3">
                ${prerequisitesMissing
                  ? html`<p className="text-xs text-[var(--v2-warning-text)]">
                      ${t("validatorReview.prerequisitesMissing")}
                    </p>` : null}

                ${actionError
                  ? html`<p role="alert" className="text-xs text-[var(--v2-danger-text)]">
                      ${actionError}
                    </p>` : null}

                <div className="flex items-center gap-2">
                  <button
                    onClick=${() => {
                      setActionError("");
                      setRejectOpen((open) => !open);
                    }}
                    disabled=${isBusy || prerequisitesMissing}
                    className="rounded px-2 py-1 text-xs font-medium text-[var(--v2-danger-text)] hover:bg-[var(--v2-danger-bg)] disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    ${t("validationQueue.reject")}
                  </button>
                  <button
                    onClick=${() => {
                      setActionError("");
                      approveMutation.mutate();
                    }}
                    disabled=${isBusy || prerequisitesMissing}
                    className="rounded px-2 py-1 text-xs font-medium text-[var(--v2-accent-text)] hover:bg-[var(--v2-accent-bg)] disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    ${t("validationQueue.validate")}
                  </button>
                </div>

                ${rejectOpen
                  ? html`<div className="rounded border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-2">
                      <label
                        htmlFor="validator-reject-feedback"
                        className="font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--v2-text-faint)]"
                      >
                        ${t("validationQueue.feedbackLabel")}
                      </label>
                      <textarea
                        id="validator-reject-feedback"
                        rows=${3}
                        value=${feedback}
                        onChange=${(e) => setFeedback(e.target.value)}
                        placeholder=${t("validationQueue.feedbackPlaceholder")}
                        className="mt-1 w-full rounded border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-2 py-1 text-xs text-[var(--v2-text-strong)]"
                      />
                      <p className="mt-1 text-[11px] text-[var(--v2-text-muted)]">
                        ${t("validationQueue.feedbackHint")}
                      </p>
                      <div className="mt-2 flex items-center justify-end gap-2">
                        <button
                          onClick=${() => { setRejectOpen(false); setFeedback(""); }}
                          disabled=${isBusy}
                          className="rounded px-2 py-1 text-xs font-medium text-[var(--v2-text-muted)] hover:bg-[var(--v2-surface-muted)] disabled:opacity-40 disabled:cursor-not-allowed"
                        >
                          ${t("common.cancel")}
                        </button>
                        <button
                          onClick=${() => {
                            setActionError("");
                            rejectMutation.mutate(trimmedFeedback);
                          }}
                          disabled=${isBusy || trimmedFeedback.length === 0}
                          title=${trimmedFeedback.length === 0
                            ? t("validationQueue.feedbackRequired")
                            : ""}
                          className="rounded px-2 py-1 text-xs font-medium text-[var(--v2-danger-text)] hover:bg-[var(--v2-danger-bg)] disabled:opacity-40 disabled:cursor-not-allowed"
                        >
                          ${t("validationQueue.confirmReject")}
                        </button>
                      </div>
                    </div>` : null}

                <p className="text-[11px] text-[var(--v2-text-faint)]">
                  ${t("validatorReview.newDraftHint")}
                </p>
              </div>
            <//>` : null}

    </div>
  `;
}

/**
 * Public guard wrapper. Returns null for excluded classes without entering
 * any hook scope, so the inner component's hooks are always called in a
 * stable order inside their own React component boundary.
 *
 * React element creation (`html\`<${ValidatorReviewPanelInner}/>\``) establishes
 * a separate fiber/component instance for the inner function. This guarantees
 * that toggling between an included and excluded class never disrupts the hook
 * call sequence of ValidatorReviewPanelInner.
 *
 * @param {object} item - Queue row item from fetchValidationQueue.
 * @param {function?} onDone - Called after a successful approve/reject.
 */
export function ValidatorReviewPanel({ item, onDone }) {
  if (!REVIEW_PANEL_CLASSES.has(item?.class_code)) return null;
  return html`<${ValidatorReviewPanelInner} item=${item} onDone=${onDone} />`;
}
