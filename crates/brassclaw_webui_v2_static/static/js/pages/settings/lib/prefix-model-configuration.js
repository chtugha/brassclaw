const MAX_OBSERVATION_CHARACTERS = 2 * 1024 * 1024;

// Display the owning store's exact-config observation. Never derive runtime
// capabilities or cache settings from a model name, dtype or weight label.
export function parseModelConfigurationObservation(detail, specification, artifactSet) {
  const summary = detail?.summary;
  const original = artifactSet?.files?.find(file => file.name === "config.json");
  if (!original || summary?.specification !== specification || summary?.id !== artifactSet.id ||
      typeof detail.configuration_observation !== "string" || detail.configuration_observation.length > MAX_OBSERVATION_CHARACTERS) throw Error("prefix.modelConfigInvalid");
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


// Source-only evidence for the saved model-design construction step. No template
// is selected, evaluated, installed or treated as an effective provider default.
export function parseModelFormatObservation(detail, specification, artifactSet) {
  if (detail?.format_observation == null) return null; // compatible older server
  if (detail.summary?.specification !== specification || detail.summary?.id !== artifactSet.id ||
      typeof detail.format_observation !== "string" || detail.format_observation.length > MAX_OBSERVATION_CHARACTERS) throw Error("prefix.modelConfigInvalid");
  let report;
  try { report = JSON.parse(detail.format_observation); } catch { throw Error("prefix.modelConfigInvalid"); }
  if (report?.format !== "brassclaw-prefix-model-format-observation/1" ||
      report.qualification !== "retained-format-sources-only" || report.specification !== specification ||
      report.artifact_set !== artifactSet.id || report.effective_template !== null || report.serving_compatibility !== null ||
      !["observed-format-sources", "no-declared-template", "unsupported-or-invalid-source", "inspection-capacity-exceeded"].includes(report.status) ||
      !Array.isArray(report.sources) || !Array.isArray(report.templates) ||
      !report.fields || typeof report.fields !== "object" || Array.isArray(report.fields) ||
      !report.source_shapes || typeof report.source_shapes !== "object" || Array.isArray(report.source_shapes)) throw Error("prefix.modelConfigInvalid");
  const names = new Set();
  const expected = artifactSet.files.filter(file => ["tokenizer_config.json", "special_tokens_map.json", "added_tokens.json", "chat_template.jinja"].includes(file.name));
  if (report.sources.length !== expected.length) throw Error("prefix.modelConfigInvalid");
  for (const source of report.sources) {
    const original = expected.find(file => file.name === source.file);
    const returned = detail.summary.files?.find(file => file.name === source.file);
    if (!original || names.has(source.file) || !/^[0-9a-f]{64}$/.test(source.sha256) ||
        !Number.isSafeInteger(source.bytes) || source.bytes <= 0 || source.sha256 !== original.sha256 || source.bytes !== original.bytes ||
        returned?.sha256 !== original.sha256 || returned?.bytes !== original.bytes) throw Error("prefix.modelConfigInvalid");
    names.add(source.file);
  }
  if (!names.has("tokenizer_config.json") || Object.keys(report.fields).some(name => !names.has(name)) ||
      Object.keys(report.source_shapes).some(name => !names.has(name))) throw Error("prefix.modelConfigInvalid");
  const candidates = new Set();
  for (const template of report.templates) {
    if (!names.has(template?.file) || typeof template.template !== "string" || !template.template.trim() ||
        !/^[0-9a-f]{64}$/.test(template.template_sha256) ||
        !(template.name === null || typeof template.name === "string" && template.name.length > 0) ||
        !(template.pointer === null && template.name === null && template.file === "chat_template.jinja" ||
          template.file === "tokenizer_config.json" && typeof template.pointer === "string" &&
          (template.name === null && template.pointer === "/chat_template" ||
           template.name !== null && /^\/chat_template\/(?:(?:[^~\/]|~[01])+|(?:0|[1-9][0-9]*)\/template)$/.test(template.pointer)))) throw Error("prefix.modelConfigInvalid");
    const key = JSON.stringify([template.file, template.pointer]);
    if (candidates.has(key)) throw Error("prefix.modelConfigInvalid");
    candidates.add(key);
  }
  if (report.status === "observed-format-sources" ? !report.templates.length : report.templates.length) throw Error("prefix.modelConfigInvalid");
  return { status: report.status, text: detail.format_observation };
}
