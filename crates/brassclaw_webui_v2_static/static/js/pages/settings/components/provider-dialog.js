import { Button } from "../../../design-system/button.js";
import { Input, Select } from "../../../design-system/input.js";
import { Modal, ModalBody, ModalFooter } from "../../../design-system/modal.js";
import { html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import {
  ADAPTER_OPTIONS,
} from "../lib/llm-providers.js";
import { useProviderDialogForm } from "../hooks/useProviderDialogForm.js";
import { TokenBudgetForm } from "./token-budget-form.js";

export function ProviderDialog({
  provider,
  allProviderIds,
  builtinOverrides,
  responseCacheDefaults,
  modelDiscoveryDefaults,
  requestPolicyDefaults,
  open,
  onClose,
  onSave,
  onTest,
  onListModels,
}) {
  const t = useT();
  const formState = useProviderDialogForm({
    provider,
    allProviderIds,
    builtinOverrides,
    responseCacheDefaults,
    modelDiscoveryDefaults,
    requestPolicyDefaults,
    open,
    onClose,
    onSave,
    onTest,
    onListModels,
    t,
  });

  if (!open) return null;

  const { form, apiKey, models, message, busy, isBuiltin, isEditing } = formState;
  const title = isBuiltin
    ? t("llm.configureProvider", { name: provider.name || provider.id })
    : isEditing
    ? t("llm.editProvider")
    : t("llm.newProvider");

  return html`
    <${Modal} open=${open} onClose=${onClose} title=${title} size="lg">
      <${ModalBody} className="space-y-4">
        ${html`
          <div className="grid gap-4 sm:grid-cols-2">
            <label className="space-y-2 text-sm text-[var(--v2-text-strong)]">
              ${t("llm.providerName")}
              <${Input} value=${form.name} onChange=${(e) => formState.update("name", e.target.value)} />
            </label>
            <label className="space-y-2 text-sm text-[var(--v2-text-strong)]">
              ${t("llm.providerId")}
              <${Input}
                value=${form.id}
                disabled=${isEditing}
                onChange=${(e) => {
                  formState.markIdEdited();
                  formState.update("id", e.target.value);
                }}
              />
            </label>
          </div>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.adapter")}
            <${Select} value=${form.adapter} onChange=${(e) => formState.update("adapter", e.target.value)}>
              ${!ADAPTER_OPTIONS.some((item) => item.value === form.adapter) && html`<option value=${form.adapter}>${form.adapter}</option>`}
              ${ADAPTER_OPTIONS.map((item) => html`<option key=${item.value} value=${item.value}>${item.label}</option>`)}
            <//>
          </label>
        `}

        <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
          ${t("llm.baseUrl")}
          <${Input} value=${form.baseUrl} placeholder=${provider?.base_url || ""} onChange=${(e) => formState.update("baseUrl", e.target.value)} />
        </label>

        <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
          ${t("llm.apiKey")}
          <${Input} type="password" value=${apiKey} placeholder=${t("llm.apiKeyPlaceholder")} onChange=${(e) => formState.setApiKey(e.target.value)} />
        </label>

        <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
          ${t("llm.defaultModel")}
          <div className="flex gap-2">
            <${Input}
              list="provider-model-list"
              value=${form.model}
              onChange=${(e) => formState.update("model", e.target.value)}
              className="flex-1"
            />
            ${models.length > 0 && html`
              <datalist id="provider-model-list">
                ${models.map((model) => html`<option key=${model} value=${model} />`)}
              </datalist>
            `}
            <${Button} type="button" variant="secondary" size="sm" disabled=${busy !== ""} onClick=${formState.fetchModels}>
              ${busy === "models" ? t("llm.fetchingModels") : t("llm.fetchModels")}
            <//>
          </div>
        </label>

        ${message &&
        html`
          <div className=${message.tone === "error" ? "text-sm text-red-200" : "text-sm text-mint"} role="status">
            ${message.text}
          </div>
        `}

        <fieldset className="space-y-3 rounded-md border border-[var(--v2-panel-border)] p-3">
          <legend className="px-1 text-sm font-medium text-[var(--v2-text-strong)]">${t("llm.embeddingModelsTitle")}</legend>
          <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.embeddingModelsHelp")}</p>
          ${form.embeddingModels.map((row, index) => html`
            <div key=${index} className="grid gap-3 rounded-md border border-[var(--v2-panel-border)] p-3 sm:grid-cols-3">
              ${[["model", "llm.embeddingModelName", "text", undefined], ["dimension", "llm.embeddingDimension", "number", 1], ["cacheMaxEntries", "llm.embeddingCacheEntries", "number", 0], ["vectorRevision", "llm.embeddingVectorRevision", "number", 1]].map(([field, label, type, min]) => html`
                <label className="space-y-2 text-sm text-[var(--v2-text-strong)]">
                  ${t(label)}
                  <${Input} type=${type} min=${min} max=${type === "number" ? 4294967295 : undefined} step=${type === "number" ? "1" : undefined} value=${row[field]} disabled=${busy !== ""}
                    onChange=${(event) => formState.update("embeddingModels", form.embeddingModels.map((item, i) => i === index ? { ...item, [field]: event.target.value } : item))} />
                  ${field === "dimension" && html`<p className="text-xs text-[var(--v2-text-muted)]">${t("llm.embeddingDimensionHelp")}</p>`}
                  ${field === "vectorRevision" && html`<p className="text-xs text-[var(--v2-text-muted)]">${t("llm.embeddingVectorRevisionHelp")}</p>`}
                  ${field === "cacheMaxEntries" && html`<p className="text-xs text-[var(--v2-text-muted)]">${t("llm.embeddingCacheEntriesHelp")}</p>`}
                </label>
              `)}
              <${Button} type="button" variant="secondary" size="sm" disabled=${busy !== ""} onClick=${() => formState.update("embeddingModels", form.embeddingModels.filter((_, i) => i !== index))}>${t("llm.embeddingRemoveModel")}<//>
            </div>
          `)}
          <${Button} type="button" variant="secondary" size="sm" disabled=${busy !== ""} onClick=${() => formState.update("embeddingModels", [...form.embeddingModels, { model: "", dimension: "", cacheMaxEntries: "0", vectorRevision: "1" }])}>${t("llm.embeddingAddModel")}<//>
        </fieldset>

        <fieldset className="space-y-3 rounded-md border border-[var(--v2-panel-border)] p-3">
          <legend className="px-1 text-sm font-medium text-[var(--v2-text-strong)]">${t("llm.requestPolicyTitle")}</legend>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.requestTimeout")}
            <${Input} type="number" min="1" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.requestTimeout} onChange=${(event) => formState.update("requestTimeout", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.requestTimeoutHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.responseBytes")}
            <${Input} type="number" min="0" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.responseBytes} onChange=${(event) => formState.update("responseBytes", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.responseBytesHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.requestRetries")}
            <${Input} type="number" min="0" max=${4294967295} step="1" value=${form.requestRetries} onChange=${(event) => formState.update("requestRetries", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.requestRetriesHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.circuitThreshold")}
            <${Input} type="number" min="0" max=${4294967295} step="1" value=${form.circuitThreshold} onChange=${(event) => formState.update("circuitThreshold", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.circuitThresholdHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.circuitRecovery")}
            <${Input} type="number" min="1" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.circuitRecovery} onChange=${(event) => formState.update("circuitRecovery", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.circuitRecoveryHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.retryBase")}
            <${Input} type="number" min="1" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.retryBase} onChange=${(event) => formState.update("retryBase", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.retryBaseHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.retryMax")}
            <${Input} type="number" min="1" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.retryMax} onChange=${(event) => formState.update("retryMax", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.retryMaxHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.retryJitter")}
            <${Input} type="number" min="0" max=${100} step="1" value=${form.retryJitter} onChange=${(event) => formState.update("retryJitter", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.retryJitterHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.retryAfterMax")}
            <${Input} type="number" min="1" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.retryAfterMax} onChange=${(event) => formState.update("retryAfterMax", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.retryAfterMaxHelp")}</p>
          </label>
        </fieldset>

        <fieldset className="space-y-3 rounded-md border border-[var(--v2-panel-border)] p-3">
          <legend className="px-1 text-sm font-medium text-[var(--v2-text-strong)]">${t("llm.modelDiscoveryTitle")}</legend>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.modelDiscoveryTimeout")}
            <${Input} type="number" min="1" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.discoveryTimeout} onChange=${(event) => formState.update("discoveryTimeout", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.modelDiscoveryTimeoutHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.modelDiscoveryBytes")}
            <${Input} type="number" min="1" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.discoveryBytes} onChange=${(event) => formState.update("discoveryBytes", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.modelDiscoveryBytesHelp")}</p>
          </label>
        </fieldset>

        <fieldset className="space-y-3 rounded-md border border-[var(--v2-panel-border)] p-3">
          <legend className="px-1 text-sm font-medium text-[var(--v2-text-strong)]">${t("llm.responseCacheTitle")}</legend>
          <label className="flex items-center gap-2 text-sm text-[var(--v2-text-strong)]">
            <input type="checkbox" checked=${form.cacheEnabled} onChange=${(event) => formState.update("cacheEnabled", event.target.checked)} />
            ${t("llm.responseCacheEnabled")}
          </label>
          <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.responseCacheEnabledHelp")}</p>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.responseCacheEntries")}
            <${Input} type="number" min="0" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.cacheEntries} onChange=${(event) => formState.update("cacheEntries", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.responseCacheEntriesHelp")}</p>
          </label>
          <label className="block space-y-2 text-sm text-[var(--v2-text-strong)]">
            ${t("llm.responseCacheTtl")}
            <${Input} type="number" min="0" max=${Number.MAX_SAFE_INTEGER} step="1" value=${form.cacheTtlSecs} onChange=${(event) => formState.update("cacheTtlSecs", event.target.value)} />
            <p className="text-xs text-[var(--v2-text-muted)]">${t("llm.responseCacheTtlHelp")}</p>
          </label>
        </fieldset>

        ${provider?.id && html`
          <details>
            <summary className="mt-4 cursor-pointer text-sm font-medium text-[var(--v2-text-strong)]">
              ${t("llm.tokenLimits")}
            </summary>
            <div className="mt-3">
              <${TokenBudgetForm}
                providerId=${provider.id}
                queryKey=${["provider-tokens", provider.id]}
              />
            </div>
          </details>
        `}
      <//>
      <${ModalFooter}>
        <${Button} type="button" variant="secondary" disabled=${busy !== ""} onClick=${formState.runTest}>
          ${busy === "test" ? t("llm.testing") : t("llm.testConnection")}
        <//>
        <${Button} type="button" variant="ghost" disabled=${busy !== ""} onClick=${onClose}>${t("common.cancel")}<//>
        <${Button} type="button" disabled=${busy !== ""} onClick=${formState.submit}>
          ${busy === "save" ? t("common.saving") : t("common.save")}
        <//>
      <//>
    <//>
  `;
}
