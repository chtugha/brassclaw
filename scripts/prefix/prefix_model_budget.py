"""Pure planning over an already pinned policy; never a settings writer.

The caller resolves policy_ref through the provider owner and verifies deployment
facts. This arithmetic neither selects knowledge nor measures rendered tokens.
Framing charged here must be outside both prefix and dynamic-prompt measurements.
"""


def budget_integer(value, minimum):
    if type(value) is not int or value < minimum or value > 9007199254740991:
        raise ValueError('exact supported budget integer required')
    return value


def plan_prefix_model_budget(inputs):
    names = ['policy_ref', 'max_prefix_tokens', 'max_prompt_tokens',
             'context_window_tokens', 'reserved_output_tokens',
             'additional_framing_tokens', 'requested_prefix_tokens']
    if not isinstance(inputs, dict) or sorted(inputs) != sorted(names):
        raise ValueError('complete pinned model budget required')
    policy = inputs['policy_ref']
    if not isinstance(policy, str) or not policy or policy.strip() != policy:
        raise ValueError('retained provider/model policy reference required')
    prefix = budget_integer(inputs['max_prefix_tokens'], 0)
    prompt = budget_integer(inputs['max_prompt_tokens'], 1)
    context = budget_integer(inputs['context_window_tokens'], 1)
    output = budget_integer(inputs['reserved_output_tokens'], 1)
    framing = budget_integer(inputs['additional_framing_tokens'], 0)
    requested = budget_integer(inputs['requested_prefix_tokens'], 0)
    reserved = budget_integer(prompt + output + framing, 1)
    remaining = max(0, context - reserved)
    effective = min(prefix, requested, remaining)
    return {
        'format': 'brassclaw-prefix-model-budget/1',
        'qualification': 'planning-only; not rendered fit, policy verification or approval',
        'policy_ref': policy,
        'max_prefix_tokens': prefix,
        'max_prompt_tokens': prompt,
        'context_window_tokens': context,
        'reserved_output_tokens': output,
        'additional_framing_tokens': framing,
        'requested_prefix_tokens': requested,
        'effective_prefix_budget': effective,
        'reservation_shortfall_tokens': max(0, reserved - context),
        'requested_shortfall_tokens': requested - effective,
        'status': ('reservation-blocked' if reserved > context else
                   'prefix-disabled' if prefix == 0 else
                   'planning-ready'),
    }
