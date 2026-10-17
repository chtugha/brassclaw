export const API_KEY_UNCHANGED = "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022";

export const ADAPTER_OPTIONS = [
  { value: "open_ai_completions", label: "OpenAI Compatible" },
  { value: "anthropic", label: "Anthropic" },
  { value: "ollama", label: "Ollama" },
  { value: "nearai", label: "NEAR AI" },
];

export function adapterLabel(adapter) {
  return ADAPTER_OPTIONS.find((item) => item.value === adapter)?.label || adapter;
}

export function parseCustomProviders(value) {
  if (Array.isArray(value)) return value;
  if (!value || typeof value !== "string") return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch (_) {
    return [];
  }
}

export function parseBuiltinOverrides(value) {
  if (value && typeof value === "object" && !Array.isArray(value)) return value;
  if (!value || typeof value !== "string") return {};
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  } catch (_) {
    return {};
  }
}

export function providerEffectiveBaseUrl(provider, overrides) {
  const override = provider.builtin ? overrides[provider.id] || {} : {};
  return override.base_url || provider.env_base_url || provider.base_url || "";
}

export function providerDisplayModel(provider, overrides, activeId, selectedModel) {
  const override = provider.builtin ? overrides[provider.id] || {} : {};
  if (provider.id === activeId) {
    return selectedModel || override.model || provider.env_model || provider.default_model || "";
  }
  return override.model || provider.env_model || provider.default_model || "";
}

export function providerDefaultModel(provider, overrides) {
  const override = provider.builtin ? overrides[provider.id] || {} : {};
  return override.model || provider.env_model || provider.default_model || "";
}

export function providerAcceptsApiKey(provider) {
  if (!provider) return false;
  if (!provider.builtin) return provider.adapter !== "ollama";
  if (provider.accepts_api_key !== undefined) return provider.accepts_api_key !== false;
  return provider.api_key_required !== false;
}

export function isProviderConfigured(provider, overrides) {
  const override = provider.builtin ? overrides[provider.id] || {} : {};
  // Use api_key_required from the server snapshot for both builtin and custom
  // providers. Custom openai-compatible providers pointed at local servers
  // (vLLM, LM Studio, etc.) have api_key_required=false and must not be
  // blocked by the key check.
  const needsKey = provider.api_key_required !== false;
  const storedKey = provider.builtin ? override.api_key : provider.api_key;
  const hasDbKey = storedKey === API_KEY_UNCHANGED || (typeof storedKey === "string" && storedKey.length > 0);
  const keyOk = !needsKey || provider.has_api_key === true || hasDbKey;
  if (!keyOk) return false;

  const needsBaseUrl = provider.builtin ? provider.base_url_required === true : true;
  if (!needsBaseUrl) return true;
  return providerEffectiveBaseUrl(provider, overrides).trim().length > 0;
}

export function providerStatus(provider, overrides, activeProviderId) {
  if (provider.id === activeProviderId) return "active";
  return isProviderConfigured(provider, overrides) ? "ready" : "setup";
}

export function groupProvidersByStatus(providers, overrides, activeProviderId) {
  const buckets = { active: [], ready: [], setup: [] };
  if (!Array.isArray(providers)) return buckets;
  for (const provider of providers) {
    const status = providerStatus(provider, overrides, activeProviderId);
    if (buckets[status]) buckets[status].push(provider);
  }
  return buckets;
}

export function providerMissingReason(provider, overrides) {
  const override = provider.builtin ? overrides[provider.id] || {} : {};
  const needsKey = provider.api_key_required !== false;
  const storedKey = provider.builtin ? override.api_key : provider.api_key;
  const hasDbKey = storedKey === API_KEY_UNCHANGED || (typeof storedKey === "string" && storedKey.length > 0);
  if (needsKey && provider.has_api_key !== true && !hasDbKey) return "api_key";

  const needsBaseUrl = provider.builtin ? provider.base_url_required === true : true;
  if (needsBaseUrl && !providerEffectiveBaseUrl(provider, overrides).trim()) return "base_url";
  return "ok";
}

export function providerRequestPolicy(form) {
  const fields = [form.requestTimeout, form.requestRetries, form.circuitThreshold, form.circuitRecovery];
  if (fields.every((value) => value === undefined)) return undefined;
  if (fields.some((value) => !["string", "number"].includes(typeof value) || String(value).trim() === "" || !Number.isSafeInteger(Number(value))) || Number(form.requestTimeout) < 1 || Number(form.circuitRecovery) < 1 || Number(form.requestRetries) < 0 || Number(form.requestRetries) > 4294967295 || Number(form.circuitThreshold) < 0 || Number(form.circuitThreshold) > 4294967295) {
    throw new RangeError("Model request limits must be valid whole numbers.");
  }
  const timingValues = [form.retryBase, form.retryMax, form.retryJitter, form.retryAfterMax];
  let timing;
  if (!timingValues.every((value) => value === undefined)) {
    if (timingValues.some((value) => !["string", "number"].includes(typeof value) || String(value).trim() === "" || !Number.isSafeInteger(Number(value))) || Number(form.retryBase) < 1 || Number(form.retryMax) < Number(form.retryBase) || Number(form.retryJitter) < 0 || Number(form.retryJitter) > 100 || Number(form.retryAfterMax) < 1) {
      throw new RangeError("Retry timing must be positive whole numbers, the maximum must cover the base and jitter must be 0–100 percent.");
    }
    timing = { base_delay_ms: Number(form.retryBase), max_backoff_ms: Number(form.retryMax), jitter_percent: Number(form.retryJitter), max_retry_after_secs: Number(form.retryAfterMax) };
  }
  const policy = { timeout_secs: Number(form.requestTimeout), max_retries: Number(form.requestRetries), circuit_breaker_threshold: Number(form.circuitThreshold) || null, circuit_breaker_recovery_secs: Number(form.circuitRecovery) };
  if (form.responseBytes !== undefined) {
    if (!["string", "number"].includes(typeof form.responseBytes) || String(form.responseBytes).trim() === "" || !Number.isSafeInteger(Number(form.responseBytes)) || Number(form.responseBytes) < 0) {
      throw new RangeError("Response capacity must be a nonnegative whole number; zero disables it.");
    }
    const bytes = Number(form.responseBytes);
    if (bytes && !["open_ai_completions", "anthropic", "ollama", "deep_seek", "gemini", "open_router", "github_copilot", "nearai", "near_ai", "gemini_oauth", "openai_codex"].includes(form.adapter)) {
      throw new RangeError("This adapter does not yet support a response-body capacity.");
    }
    policy.max_response_bytes = bytes || null;
  }
  if (timing) policy.retry_timing = timing;
  return policy;
}

export function providerEmbeddingModels(form) {
  if (form.embeddingModels === undefined) return undefined;
  if (!Array.isArray(form.embeddingModels)) throw new RangeError("Invalid embedding model contracts.");
  if (form.embeddingModels.length && form.adapter !== undefined && !["open_ai_completions", "open_router", "nearai", "near_ai", "ollama", "bedrock"].includes(form.adapter)) {
    throw new RangeError("This protocol has no supported embedding adapter.");
  }
  const names = new Set();
  const rows = form.embeddingModels.map((row) => {
    const model = row?.model;
    if (typeof model !== "string" || !model || model.trim() !== model || /[\u0000-\u001f\u007f-\u009f]/u.test(model) || names.has(model)) {
      throw new RangeError("Embedding model names must be distinct and nonempty.");
    }
    names.add(model);
    for (const value of [row.dimension, row.cacheMaxEntries]) {
      if (!["string", "number"].includes(typeof value) || String(value).trim() === "" || !Number.isSafeInteger(Number(value)) || Number(value) < 0 || Number(value) > 4294967295) {
        throw new RangeError("Embedding dimensions and cache entries must be valid whole numbers.");
      }
    }
    if (Number(row.dimension) === 0) throw new RangeError("Embedding dimensions must be positive.");
    const revision = row.vectorRevision === undefined ? "1" : row.vectorRevision;
    if (!["string", "number"].includes(typeof revision) || String(revision).trim() === "" || !Number.isSafeInteger(Number(revision)) || Number(revision) < 1 || Number(revision) > 4294967295) {
      throw new RangeError("Embedding vector revisions must be positive whole numbers.");
    }
    const contract = { dimension: Number(row.dimension), cache_max_entries: Number(row.cacheMaxEntries) };
    if (Number(revision) !== 1) contract.vector_revision = Number(revision);
    return [model, contract];
  });
  return Object.fromEntries(rows);
}

export function providerModelDiscoveryPolicy(form) {
  // Older callers may omit a policy to retain the persisted one. A partially
  // supplied or invalid policy must never become NaN/null on the wire.
  if (form.discoveryTimeout === undefined && form.discoveryBytes === undefined) return undefined;
  const values = [form.discoveryTimeout, form.discoveryBytes];
  if (values.some((value) => !["string", "number"].includes(typeof value) || String(value).trim() === "" || !Number.isSafeInteger(Number(value)) || Number(value) < 1)) {
    throw new RangeError("Model discovery limits must be positive whole numbers.");
  }
  return { timeout_secs: Number(form.discoveryTimeout), max_response_bytes: Number(form.discoveryBytes) };
}

export function providerPayload(provider, form, apiKey, overrides) {
  const baseUrl = form.baseUrl.trim();
  const model = form.model.trim();
  const payload = {
    model_discovery: providerModelDiscoveryPolicy(form),
    adapter: form.adapter,
    base_url: baseUrl || provider?.base_url || "",
    provider_id: provider?.id || form.id.trim(),
    provider_type: provider?.builtin ? "builtin" : "custom",
  };
  if (model) payload.model = model;
  if (apiKey.trim()) payload.api_key = apiKey.trim();

  const override = provider?.builtin ? overrides[provider.id] || {} : {};
  if (!payload.api_key && override.api_key === API_KEY_UNCHANGED) {
    payload.api_key = undefined;
  }
  return payload;
}

export function providerIdFromName(name) {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9_]+/g, "-")
    .replace(/^-|-$/g, "");
}

export function isValidProviderId(id) {
  return /^[a-z0-9_-]+$/.test(id);
}
