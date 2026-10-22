# Model input ceilings

The writable owner is `ProviderDefinition.model_token_limits[model]` inside the
existing PostgreSQL `brassclaw_llm_providers.definition` record. Inference and
Prefix settings use the same editor/query and GET/POST
`/api/webchat/v2/llm/providers`, with the provider's `expected_revision`.

For the requested current Ornith configuration:

```json
{
  "model_token_limits": {
    "cyankiwi/Ornith-1.5-9B-AWQ-INT4": {
      "max_prefix_tokens": 100000,
      "max_prompt_tokens": 14688
    }
  }
}
```

This is the policy fragment for an ordinary provider update, not a second
configuration file or complete API request. Preserve other model entries and
include the exact provider identity, adapter and revision. Omission preserves
existing limits; `{}` clears the map; null is rejected. Stale revisions conflict.
No provider activation or credential replacement is implied.

Both values are always-on hard input limits, independent of
`token_budgets_enabled`. Disabling discretionary budgeting must preserve them and
all technical size checks. Kohai needs both the active answer target's and Sempai
target's host-resolved limits. Check the complete Sempai request before review,
then the final edited answer request with history/tools/framing and prefix after
review, and repeat technical enforcement at the shared pre-dispatch boundary.
Settings/planner support alone does not prove this runtime wiring is complete.

| Value | Owner / meaning |
| --- | --- |
| Maximum prefix tokens | Canonical per-model map above. Zero disables prefix capacity. |
| Maximum prompt excluding prefix | Same map; includes remaining rendered history, tools and input framing. Positive integer. |
| Total input ceiling | Derived checked sum, 114688 for this request. Never another stored setting. |
| Physical/deployed context | Server observation and declared provider context are evidence/bounds, not aliases for maximum prefix or prompt. Current vLLM advertises 131072. |
| Output allowance | Separate request/model constraint. The difference here is 16384; do not store another editable copy of that arithmetic. |
| Model specification/design/measurement | Immutable historical pins and observations, not settings endpoints. |
| Standalone compiler `--context-tokens` | An explicit offline build target. It does not edit live policy and cannot authorize an oversized production prefix. |
| `ProviderTokenBudget.total_input` and per-slice budgets | Optional accounting/assembly policy, disabled with token budgets. These are not per-model prefix or dynamic-prompt settings. |
| `--max-num-batched-tokens` | vLLM scheduler/prefill batch size; not the complete prompt or prefix limit. |
| Encoding worker token/frame/heap ceilings | Technical containment of one preparation operation; not a second model capacity setting. |

The audit found no `max_prefix_tokens` or `max_prompt_tokens` setting in the
previous production Rust/JavaScript/SQL paths. The 114688 prefix preview existed
in standalone Sempai usage instructions and this plan, not as a deployed per-model
database setting. Those current instructions now request a 100000-token prefix
and preserve archived experiments unchanged. No historical receipt is rewritten
to make its measured token counts agree with a new policy.

New immutable prefix model captures include the model's configured limits and
provider revision. Existing records remain historical evidence. Future compiler
and final-prompt consumers must resolve the canonical policy for new work, pin it
for retained work, and check actual target tokenization without silent truncation.
Persistence/UI support alone does not establish final-prompt enforcement or cache
qualification; the production resolver remains a prefix-plan prerequisite.

`plan-prefix-model-budget` is a reusable pure component draft for the successor
generation/design Recipes. It derives a planning budget from pinned policy,
requested design size, deployed context and explicit reservations. It reports
shortfalls rather than changing settings or silently promising less prompt/output.
It does not choose sources, verify policy identity, measure rendered tokens or
qualify activation. Existing measurement-v1 results remain total-probe diagnostics.
The detailed [design contract](../plans/prefix_v3_upgrade.md#budget-aware-design-and-generation-contract-2026-10-22)
requires accepted budget-dependent whole-unit policies, exact composite/partition
measurement and coverage/quality checks before model-facing use.

On 2026-10-22 the running `brassclaw2:3000` application exposed no provider
revisions/model limits and reported `active: null`. Its separate model endpoint
advertised Ornith with 131072 context. Do not write new definition fields into an
older application behind its API: old strict decoders may reject them. Application
upgrade/support and a successful revisioned API readback are required before
claiming these requested values are set on that instance. No inference service
change is required by this policy.
