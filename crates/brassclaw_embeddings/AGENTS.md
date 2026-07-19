# brassclaw_embeddings

Pure trait-and-utilities layer for vector embeddings (S10 refactor, revision 17).

Concrete provider implementations, the `EmbeddingsConfig` shape, the `create_provider` factory, and `ProviderDeps` have moved to `brassclaw_reborn_composition::embedding_providers` — they depend on `brassclaw_llm` runtime objects and DB-backed config that belongs in the composition layer.

## Responsibilities

- Define `EmbeddingProvider`, `EmbeddingError`, and re-export them as the workspace-wide trait contract.
- Provide `CachedEmbeddingProvider` + `EmbeddingCacheConfig` (LRU decorator).
- Expose `url_check::check_base_url` — the AlwaysBlocked IP-class floor check (not a full SSRF policy).
- Expose `default_dimension_for_model` — model-name → embedding dimension helper.
- Expose `DEFAULT_EMBEDDING_CACHE_SIZE` constant.
- Expose `MockEmbeddings` behind the `testing` cargo feature (deterministic test double).

## Non-responsibilities

- Do not implement concrete HTTP providers. Those live in `brassclaw_reborn_composition::embedding_providers`.
- Do not read `Settings`, env vars, or DB rows.
- Do not define `EmbeddingsConfig` or the `create_provider` factory.
- Do not depend on `brassclaw_llm`, `reqwest`, AWS SDKs, or any runtime dependency.

## Public surface

| Symbol | Use |
|--------|-----|
| `EmbeddingProvider` trait | Trait object for all providers |
| `EmbeddingError` | Error type returned by every provider |
| `CachedEmbeddingProvider`, `EmbeddingCacheConfig` | LRU caching decorator |
| `url_check::check_base_url` | AlwaysBlocked URL floor check (used by `embedding_providers`) |
| `default_dimension_for_model` | Model → dimension helper |
| `DEFAULT_EMBEDDING_CACHE_SIZE` | Default LRU cap |
| `MockEmbeddings` | Deterministic test double (gated: `testing` feature) |

## Where the factory lives

`brassclaw_reborn_composition::embedding_providers` owns `EmbeddingsConfig`, `ProviderDeps`, `create_provider`, and the concrete provider structs (`OpenAiEmbeddings`, `NearAiEmbeddings`, `OllamaEmbeddings`). The `EmbeddingRoleAdapter` in `brassclaw_reborn_composition::embedding_role_adapter` bridges the `brassclaw_embeddings::EmbeddingProvider` seam to `brassclaw_memory::EmbeddingProvider`.
