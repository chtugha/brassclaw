# WARNING: This file is a CPython reference implementation ONLY.
# The production code is inline in default.py (Monty-safe subset).
#
# This file mirrors the exact algorithm used inside default.py, but uses the
# full Python language (classes, comprehensions, f-strings, exception handling,
# etc.) for readability, review, and unit tests under CPython. It is loaded
# only by tests; the orchestrator Python VM is never pointed at this file.
#
# Keeping the two implementations in lock-step is mandatory: divergences
# here mean behaviour differences between the test harness and the
# production orchestrator. Update both files when rule behaviour changes.

from __future__ import annotations

from collections.abc import Iterable, Sequence
from dataclasses import dataclass, field
from typing import Any


CHARS_PER_TOKEN = 4
MESSAGE_OVERHEAD_CHARS = 4


class ConfigError(ValueError):
    """Raised when a segment reduction config dict contains invalid values."""


@dataclass(frozen=True)
class ReductionRule:
    """Discriminated union of every supported reduction rule."""

    rule_type: str
    field: str = ""
    max_chars: int = 0
    keep_recent_n: int = 0
    priority: tuple = ()

    def __post_init__(self) -> None:
        if self.rule_type not in VALID_REDUCE_TYPES:
            raise ConfigError(f"unknown rule type: {self.rule_type!r}")


VALID_REDUCE_TYPES: frozenset[str] = frozenset(
    {"truncate", "summarize", "drop", "priority", "history_compact"}
)


def estimate_context_tokens(messages: Sequence[dict[str, Any]]) -> int:
    total_chars = 0
    for msg in messages:
        total_chars += len(msg.get("content", ""))
        total_chars += len(msg.get("action_name", "") or "")
        total_chars += MESSAGE_OVERHEAD_CHARS
    return (total_chars + CHARS_PER_TOKEN - 1) // CHARS_PER_TOKEN


def make_truncate_rule(field_name: str, max_chars: int) -> ReductionRule:
    return ReductionRule(rule_type="truncate", field=field_name, max_chars=int(max_chars))


def make_summarize_rule(field_name: str) -> ReductionRule:
    return ReductionRule(rule_type="summarize", field=field_name)


def make_drop_rule(field_name: str) -> ReductionRule:
    return ReductionRule(rule_type="drop", field=field_name)


def make_priority_rule(fields_priority_list: Iterable[str]) -> ReductionRule:
    return ReductionRule(rule_type="priority", priority=tuple(fields_priority_list))


def make_history_compact_rule(keep_recent_n: int) -> ReductionRule:
    return ReductionRule(rule_type="history_compact", keep_recent_n=int(keep_recent_n))


def _truncate_field(message: dict[str, Any], field_name: str, max_chars: int) -> dict[str, Any]:
    raw = message.get(field_name, "")
    if not isinstance(raw, str):
        return message
    if max_chars <= 0:
        new_message = dict(message)
        new_message[field_name] = ""
        return new_message
    if len(raw) <= max_chars:
        return message
    suffix = "..."
    keep = max(0, max_chars - len(suffix))
    truncated = raw[:keep] + suffix
    new_message = dict(message)
    new_message[field_name] = truncated
    return new_message


def _drop_field(message: dict[str, Any], field_name: str) -> dict[str, Any]:
    if field_name not in message:
        return message
    return {key: value for key, value in message.items() if key != field_name}


def _mark_summarize(message: dict[str, Any], field_name: str) -> dict[str, Any]:
    new_message = dict(message)
    flags = dict(new_message.get("_reduction_flags", {}))
    flags[field_name] = "summarize"
    new_message["_reduction_flags"] = flags
    return new_message


def _priority_drop(messages: list[dict[str, Any]], priority_fields: tuple[str, ...], budget: int) -> str | None:
    if not messages or not priority_fields:
        return None
    for candidate in reversed(priority_fields):
        messages[-1] = _drop_field(messages[-1], candidate)
        if estimate_context_tokens(messages) <= budget:
            return candidate
    return None


def _history_compact(messages: list[dict[str, Any]], keep_recent_n: int) -> list[dict[str, Any]]:
    if keep_recent_n <= 0 or len(messages) <= keep_recent_n:
        return messages
    system_prefix = [m for m in messages if m.get("role") in ("System", "system")]
    body = [m for m in messages if m.get("role") not in ("System", "system")]
    if len(body) <= keep_recent_n:
        return messages
    return system_prefix + body[-keep_recent_n:]


def _apply_rule(messages: list[dict[str, Any]], rule: ReductionRule, budget: int) -> list[dict[str, Any]]:
    if rule.rule_type == "truncate":
        if rule.field and messages:
            messages[-1] = _truncate_field(messages[-1], rule.field, rule.max_chars)
        return messages
    if rule.rule_type == "summarize":
        if rule.field and messages:
            messages[-1] = _mark_summarize(messages[-1], rule.field)
        return messages
    if rule.rule_type == "drop":
        if rule.field and messages:
            messages[-1] = _drop_field(messages[-1], rule.field)
        return messages
    if rule.rule_type == "priority":
        _priority_drop(messages, rule.priority, budget)
        return messages
    if rule.rule_type == "history_compact":
        return _history_compact(messages, rule.keep_recent_n)
    return messages


# NOTE: This function is named `reduce_prompt` (no prefix) in this CPython
# reference implementation. The production Monty orchestrator (`default.py`)
# names it `_reduce_prompt` (underscore prefix) to signal "internal to the
# module" in an environment without a real import system. The logic is
# otherwise identical.
def reduce_prompt(
    messages: list[dict[str, Any]],
    rules: Sequence[ReductionRule],
    budget_tokens: int,
) -> list[dict[str, Any]]:
    """Apply `rules` in order until `messages` fit `budget_tokens`.

    Adopts any new message list returned by `_apply_rule` so rules
    that rebuild the list (e.g. `history_compact`) are honoured
    downstream.
    """
    if estimate_context_tokens(messages) <= budget_tokens:
        return messages
    for rule in rules:
        if estimate_context_tokens(messages) <= budget_tokens:
            return messages
        updated = _apply_rule(messages, rule, budget_tokens)
        if updated is not messages:
            messages = updated
    return messages
