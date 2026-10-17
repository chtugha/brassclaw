# brassclaw_embeddings

Pure trait-and-utilities layer for vector embeddings (S10 refactor, revision 17).

Concrete provider implementations, the `EmbeddingsConfig` shape, the `create_provider` factory, and `ProviderDeps` have moved to `brassclaw_reborn_composition::embedding_providers` — they depend on `brassclaw_llm` runtime objects and DB-backed config that belongs in the composition layer.

## Responsibilities

- Define `EmbeddingProvider`, `EmbeddingError`, and re-export them as the workspace-wide trait contract.
- `max_input_length` returns an optional known UTF-8 byte limit. `None` imposes
  no artificial byte quota; never approximate model token limits as byte limits
  or truncate text. Actual model/service limits remain authoritative.
- Provide `CachedEmbeddingProvider` + `EmbeddingCacheConfig` (LRU decorator).
  Zero `max_entries` disables retention; the logical ceiling does not reserve
  entries upfront. `set_max_entries` changes the actual adapter's ceiling,
  evicts oldest entries on shrink and releases the map on disable. `clear`
  and changed ceilings fence inserts from requests already in flight; their
  results still reach the original callers. No lock is held across awaits or
  input hashing. Composition owns persistence and any WebUI control; a setter
  alone is not evidence of live settings wiring.
- Expose `url_check::check_base_url` — the AlwaysBlocked IP-class floor check (not a full SSRF policy).
- Vector dimensions are selected from DB model contracts by composition; do not infer them here.
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
| `DEFAULT_EMBEDDING_CACHE_SIZE` | Default LRU cap |
| `MockEmbeddings` | Deterministic test double (gated: `testing` feature) |

## Where the factory lives

`brassclaw_reborn_composition::embedding_providers` owns `EmbeddingsConfig`, `ProviderDeps`, `create_provider`, and the concrete provider structs (`OpenAiEmbeddings`, `NearAiEmbeddings`, `OllamaEmbeddings`). The `EmbeddingRoleAdapter` in `brassclaw_reborn_composition::embedding_role_adapter` bridges the `brassclaw_embeddings::EmbeddingProvider` seam to `brassclaw_memory::EmbeddingProvider`.
