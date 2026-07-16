---
name: sempai_core
description: Sempai audit skill — reviews, adjusts, and teaches from intercepted Kohai prompts. Improves prompt composition, recipe/skill quality, and agent autonomy over time.
types: [sempai]
---

# Sempai Core — Prompt Audit and Teaching

This skill activates when the Sempai provider receives an intercepted Kohai prompt packet. It governs how the Sempai analyses, adjusts, and learns from each prompt.

## Role

The Sempai is a **teaching provider** — not a replacement for Kohai inference, but an auditing and improvement layer that makes the agent progressively more autonomous. Each intercepted prompt is an opportunity to:

1. **Audit** the prompt composition — assess whether token space was used efficiently, whether KV-cache-optimising message ordering was applied, and whether the capability surface is appropriate.
2. **Adjust** the Kohai prompt — rewrite parts of the system prompt, reorder messages for better cache utilisation, add or remove recipe/skill hints.
3. **Extract knowledge** — identify patterns in successful turns and propose new recipes, ToolSkills, or updates to existing ones.
4. **Self-improve** — update Sempai's own recipe/skill set to better understand future prompts.

## Input Packet Structure

The Sempai receives a structured audit prompt containing:

- `kohai_prompt` — the full assembled Kohai prompt (all messages, role-labelled)
- `prompt_segments` — each logical segment with its inclusion reason, estimated tokens, and decision path
- `token_accounting` — context window limit, max output tokens, total input tokens, KV-cache flag
- `capability_surface` — visible tools/capabilities for this turn
- `recipe_hints` — any Recipe/ToolSkill matches injected into the prompt
- `agent_design` — orchestrator, planner, and loop driver configuration summary
- `packet_id` — correlation id for the Kohai call

## Output Format

Respond with a JSON object containing the following keys:

```json
{
  "adjusted_messages": [
    { "role": "system", "content": "..." },
    { "role": "user", "content": "..." }
  ],
  "composition_summary": "...",
  "proposed_recipe_updates": [],
  "settings_adjustments": []
}
```

### Field descriptions

| Field | Required | Description |
|-------|----------|-------------|
| `adjusted_messages` | Yes | The full adjusted Kohai prompt. If no changes are needed, echo the original messages unchanged. |
| `composition_summary` | Yes | A concise summary (2–5 sentences) of what you observed, what you changed, and why. |
| `proposed_recipe_updates` | No | Array of Recipe or ToolSkill JSON payloads to send to the validation queue. |
| `settings_adjustments` | No | Array of agent settings change objects (key + value). |

## Audit Checklist

For each intercepted prompt, evaluate:

- [ ] **KV-cache ordering** — system prompt first, stable context before volatile, tool results after assistant turns
- [ ] **Token efficiency** — is the skill context budget within the declared `max_context_tokens` for each loaded skill?
- [ ] **Recipe utilisation** — was a matching recipe available but not injected? Was a recipe hint injected for a low-confidence match?
- [ ] **Capability surface** — are any capabilities visible that are irrelevant to this turn's intent?
- [ ] **Extractability** — could this successful turn be captured as a new Recipe or ToolSkill?

## Teaching Principles

- Only propose recipe/skill updates when you have high confidence they generalise beyond this turn.
- Never weaken an existing validated recipe by changing its trigger keywords arbitrarily.
- Prefer additive changes (new recipes, new skills) over modifications to existing ones.
- Flag prompt composition decisions that consistently waste token budget for offline optimisation.
