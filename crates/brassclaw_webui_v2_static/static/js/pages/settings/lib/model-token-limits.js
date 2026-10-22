// One provider revision owns the complete per-model map. Never write profile
// config, model-information snapshots or browser storage as an alternative.
export function modelTokenLimitsPayload(provider, model, prefix, prompt, revision) {
  if (!provider || !Object.hasOwn(provider, "model_token_limits") || !provider.model_token_limits || typeof provider.model_token_limits !== "object" || Array.isArray(provider.model_token_limits) || provider.revision !== revision || !Number.isSafeInteger(revision) || revision < 1 || typeof model !== "string" || !model.trim() || model !== model.trim() || /[\x00-\x1f\x7f]/.test(model)) throw new Error("llm.providerChanged");
  const integer = (value, min) => {
    if (!["string", "number"].includes(typeof value) || String(value).trim() === "" || !Number.isSafeInteger(Number(value)) || Number(value) < min) throw new Error("llm.modelTokenLimitsInvalid");
    return Number(value);
  };
  const limits = { max_prefix_tokens: integer(prefix, 0), max_prompt_tokens: integer(prompt, 1) };
  const total = limits.max_prefix_tokens + limits.max_prompt_tokens;
  if (!Number.isSafeInteger(total) || (model === provider.default_model && Number.isSafeInteger(provider.context_window_tokens) && total > provider.context_window_tokens)) throw new Error("llm.modelTokenLimitsInvalid");
  return { id: provider.id, expected_revision: revision, adapter: provider.adapter,
    name: provider.description, default_model: provider.default_model, base_url: provider.base_url,
    model_token_limits: { ...provider.model_token_limits, [model]: limits } };
}
