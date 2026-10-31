import { validatePrefixRevision } from "./settings-api.js";
import { parseSetupDraft } from "./model-setup-draft.js";
const uuid = value => typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value) && value !== "00000000-0000-0000-0000-000000000000";
export function providerBootstrapTarget(receipt) {
  if (receipt?.bootstrap_job_id == null && receipt?.bootstrap_target == null) return null;
  const target = receipt?.bootstrap_target;
  if (!uuid(receipt?.mutation_id) || !uuid(receipt.bootstrap_job_id) || target?.target_kind !== "model_setup" ||
      !/^[0-9a-f]{64}$/.test(target.specification) || Object.keys(target).length !== 3) throw Error("prefix.bootstrapInvalid");
  validatePrefixRevision(target.setup_revision, { allowZero: true, maximum: BigInt(Number.MAX_SAFE_INTEGER) });
  return Object.freeze({ mutation: receipt.mutation_id, job: receipt.bootstrap_job_id, specification: target.specification, revision: String(target.setup_revision) });
}
export function providerBootstrapStatus(target, job) {
  if (job?.job_id !== target.job || job.target_kind !== "model_setup" || job.specification !== target.specification ||
      String(job.setup_revision) !== target.revision || job.operation !== "bootstrap_setup" || job.mode !== "target") throw Error("prefix.bootstrapInvalid");
  if (job.internal_receipt == null) return null;
  const result = JSON.parse(job.internal_receipt);
  if (result?.format !== "brassclaw-bootstrap-setup-result/1" || result.job_id !== target.job || result.origin_mutation !== target.mutation ||
      result.target?.specification !== target.specification || result.executing_pair?.specification !== target.specification ||
      String(result.setup?.revision) !== target.revision || result.readiness?.scope !== "configuration-prefill-only" ||
      result.readiness.generate !== "blocked" || result.readiness.design_acceptance !== "unknown" || result.readiness.serving !== "unmeasured") throw Error("prefix.bootstrapInvalid");
  return { setup: parseSetupDraft(result.setup, target.specification), text: job.internal_receipt };
}
