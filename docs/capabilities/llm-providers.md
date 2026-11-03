---
title: Inference Providers
description: Configure LLM providers for BrassClaw
---

BrassClaw routes inference through a **database-backed provider catalogue**.
Provider definitions, credentials, and model selections are managed via
**Settings → Inference** in the WebUI and stored in PostgreSQL. Provider files
and LLM environment variables do not override an active database definition.

<Note>
Legacy environment variables (`LLM_BACKEND`, `LLM_API_KEY`, `LLM_BASE_URL`,
`LLM_MODEL`, and provider-specific vars like `ANTHROPIC_API_KEY`) may still be
recognised by the underlying provider resolution layer when no database
definition is present. They are a fallback for first-run bootstrap and the
onboarding wizard — not the primary configuration mechanism for a running
instance. Use Settings → Inference to manage providers after initial setup.
</Note>

---

## Adding a provider

1. Open **Settings → Inference** with your instance operator session.
2. Choose **Add provider** and select the protocol (Anthropic, OpenAI,
   OpenAI-compatible, Bedrock, Gemini OAuth, GitHub Copilot, Ollama, etc.).
3. Enter the endpoint and model. Supply any required credential through the
   credential field — credentials are stored through the encrypted secret store,
   not as plaintext.
4. Test the provider connection or load its available models.
5. Select the provider and model for the **primary** role. Configure **Sempai**
   separately if you enable post-turn review.

A provider's stable identifier is independent of its protocol. Changing the
display name does not change the protocol adapter. Removing a provider hides and
deactivates its retained definition; re-adding it does not reinstate the previous
role selection automatically.

---

## First-run / bootstrap configuration

For initial setup and the onboarding wizard, the following environment variables
are read before a database definition is established:

```bash
# Select provider backend
LLM_BACKEND=anthropic     # anthropic | openai | openai_compatible |
                          # gemini_oauth | bedrock | github_copilot |
                          # ollama | nearai | minimax | mistral |
                          # yandex | cloudflare | ionet

# API key (provider-specific vars also work — see per-provider sections below)
LLM_API_KEY=sk-...

# Model selection
LLM_MODEL=claude-sonnet-4-20250514

# For OpenAI-compatible endpoints
LLM_BASE_URL=https://api.openrouter.ai/v1
```

Run the onboarding wizard to write these into the database:

```bash
brassclaw onboard
```

---

## Supported providers

| Provider | Backend value | Auth |
|----------|---------------|------|
| Anthropic (Claude) | `anthropic` | `ANTHROPIC_API_KEY` |
| OpenAI (GPT) | `openai` | `OPENAI_API_KEY` |
| NEAR AI | `nearai` | OAuth (browser) |
| Google Gemini | `gemini_oauth` | OAuth (browser) |
| GitHub Copilot | `github_copilot` | `GITHUB_COPILOT_TOKEN` |
| Ollama (local) | `ollama` | None |
| AWS Bedrock | `bedrock` | AWS credentials |
| Mistral | `mistral` | `MISTRAL_API_KEY` |
| MiniMax | `minimax` | `MINIMAX_API_KEY` |
| Yandex AI Studio | `yandex` | `YANDEX_API_KEY` |
| Cloudflare Workers AI | `cloudflare` | `CLOUDFLARE_API_KEY` |
| io.net | `ionet` | `IONET_API_KEY` |
| OpenAI-compatible | `openai_compatible` | `LLM_API_KEY` + `LLM_BASE_URL` |

---

## Provider-specific notes

### Anthropic

```env
LLM_BACKEND=anthropic
ANTHROPIC_API_KEY=sk-ant-...
```

Popular models: `claude-sonnet-4-20250514`, `claude-3-5-sonnet-20241022`,
`claude-3-5-haiku-20241022`

### OpenAI

```env
LLM_BACKEND=openai
OPENAI_API_KEY=sk-...
```

Popular models: `gpt-4o`, `gpt-4o-mini`, `o3-mini`

### Google Gemini (OAuth)

Uses Google OAuth with PKCE (S256) for authentication — no API key required.
On first run, a browser opens for Google account login. Credentials are stored
in the encrypted secret store.

```env
LLM_BACKEND=gemini_oauth
LLM_MODEL=gemini-2.5-flash
```

| Feature | Status |
|---------|--------|
| Function calling | ✅ |
| `generationConfig` | ✅ |
| `thinkingConfig` | ✅ |
| SSE streaming | ✅ |
| Token refresh | ✅ |

Popular models: `gemini-3.1-pro-preview`, `gemini-2.5-pro`, `gemini-2.5-flash`

### GitHub Copilot

```env
LLM_BACKEND=github_copilot
GITHUB_COPILOT_TOKEN=gho_...
LLM_MODEL=gpt-4o
```

`brassclaw onboard` can acquire this token via GitHub device login.

### Ollama (local)

```env
LLM_BACKEND=ollama
LLM_MODEL=llama3.2
# OLLAMA_BASE_URL=http://localhost:11434   # default
```

Pull a model first: `ollama pull llama3.2`

### AWS Bedrock

Requires `--features bedrock` at build time. Uses the native AWS Converse API.

```env
LLM_BACKEND=bedrock
LLM_MODEL=anthropic.claude-opus-4-6-v1
BEDROCK_REGION=us-east-1
```

Popular models: `anthropic.claude-opus-4-6-v1`,
`anthropic.claude-sonnet-4-5-20250929-v1:0`, `amazon.nova-pro-v1:0`

> **Build prerequisite:** The AWS SDK requires **CMake**.
> macOS: `brew install cmake` · Ubuntu: `sudo apt install cmake`

### OpenAI-compatible endpoints

All OpenAI-compatible providers use `LLM_BACKEND=openai_compatible`:

```env
LLM_BACKEND=openai_compatible
LLM_BASE_URL=https://openrouter.ai/api/v1
LLM_API_KEY=sk-or-...
LLM_MODEL=anthropic/claude-sonnet-4
```

Supported endpoints: OpenRouter (300+ models), Together AI, Fireworks AI,
vLLM, LiteLLM, LM Studio, and any OpenAI-compatible server.

---

## Provider technical settings

After a provider is configured, tune its reliability via Settings → Inference:

| Setting | Meaning |
|---------|---------|
| Request timeout | Maximum time for an individual HTTP request |
| Additional retries | Attempts after the initial request for eligible transient failures |
| Retry delay and jitter | Backoff and randomisation between retries |
| Provider-hint ceiling | Maximum wait from a provider's retry-after hint |
| Circuit threshold and recovery | Temporary circuit break after repeated failures |
| Response bytes | Optional model-response size ceiling |
| Completed-response cache | Retention of identical responses; zero disables |
