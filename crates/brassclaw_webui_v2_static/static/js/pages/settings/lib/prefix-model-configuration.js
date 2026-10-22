// Display the owning store's exact-config observation. Never derive runtime
// capabilities or cache settings from a model name, dtype or weight label.
export function parseModelConfigurationObservation(detail, specification, artifactSet) {
  const summary = detail?.summary;
  const original = artifactSet?.files?.find(file => file.name === "config.json");
  if (!original || summary?.specification !== specification || summary?.id !== artifactSet.id ||
      typeof detail.configuration_observation !== "string" || detail.configuration_observation.length > 2 * 1024 * 1024) throw Error("prefix.modelConfigInvalid");
  let report;
  try { report = JSON.parse(detail.configuration_observation); }
  catch { throw Error("prefix.modelConfigInvalid"); }
  const file = summary.files?.find(file => file.name === "config.json");
  if (report?.format !== "brassclaw-prefix-model-config-observation/1" ||
      report.qualification !== "declared-config-only" || report.specification !== specification ||
      report.artifact_set !== artifactSet.id || report.config_sha256 !== original.sha256 ||
      file?.sha256 !== original.sha256 || file?.bytes !== original.bytes ||
      !["observed-declarations", "unsupported-or-invalid-config", "inspection-capacity-exceeded"].includes(report.status) ||
      !report.fields || typeof report.fields !== "object" || Array.isArray(report.fields) ||
      report.runtime_cache !== null || report.weights_identity !== null || report.serving_context_limit !== null ||
      report.attention_kv_cache !== null || report.recurrent_state !== null) throw Error("prefix.modelConfigInvalid");
  // Parse identities/status only; display original observation JSON so browser
  // Number conversion cannot round large declaration values during reserialization.
  return { status: report.status, text: detail.configuration_observation };
}
