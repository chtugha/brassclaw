// Phase M.6 — client-side template feedback for intent expression authoring.
//
// Pure JS port of the Rust `parse_template` (template_extractor.rs) and the
// §0.17.4 Q1 template rules (recipe_validator.rs::check_intent_expression_template).
// Runs live as the operator types so they see the `%` slot chip, the anchor
// classification (green / yellow / red), and inline Q1 errors / warnings before
// save. The dangling-`variable_patterns` and missing-template rules need the
// compiled BuildInstruction channels and run server-side at Phase N; this pass
// covers the per-expression rules a single input field can evaluate live.
//
// Kept in lock-step with the Rust validator: the `intent_example_*` unit tests
// in recipe_validator.rs and the `template-feedback.test.mjs` cases here assert
// the same outcomes for the same expressions.

/**
 * Split a template expression into `{ prefix, suffix }` literal anchors.
 * Returns `null` when `expression` contains no `%` (a plain exact-match intent).
 * Mirrors `parse_template` in template_extractor.rs (no trim):
 *   prefix = text before the FIRST `%`
 *   suffix = text after the LAST `%`
 */
export function parseTemplate(expression) {
  const expr = String(expression);
  if (!expr.includes("%")) return null;
  const first = expr.indexOf("%");
  const last = expr.lastIndexOf("%");
  const prefix = expr.slice(0, first);
  const suffix = last + 1 < expr.length ? expr.slice(last + 1) : "";
  return { prefix, suffix };
}

/**
 * Classify a template's anchor structure for live UI feedback (§0.17.5).
 *   "plain":       no `%` — exact-match intent, always valid.
 *   "anchored":     prefix non-empty (dual or prefix-anchored) — green.
 *   "suffix-only":  prefix empty, suffix non-empty (leading-`%`) — yellow.
 *   "no-anchor":    both empty — red, hard error.
 */
export function classifyTemplate(prefix, suffix, slotCount) {
  if (slotCount === 0) return "plain";
  if (prefix.trim() === "" && suffix.trim() === "") return "no-anchor";
  // Uses exact emptiness (no trim) to match parse_template / resolve_intent
  // Path 2 semantics — a whitespace prefix is prefix-anchored (Path 1).
  if (prefix === "" && suffix !== "") return "suffix-only";
  return "anchored";
}

/**
 * Run the §0.17.4 Q1 template rules on an intent expression.
 * Returns `{ isTemplate, prefix, suffix, slotCount, classification, errors, warnings }`.
 *
 * Rules (mirrors check_intent_expression_template):
 *   - no-anchor (both prefix + suffix empty)        → hard error
 *   - leading-`%` (empty prefix, non-empty suffix)  → warning
 *   - adjacent slots (empty inter-slot separator)   → hard error
 */
export function templateFeedback(expr) {
  const text = String(expr);
  const parts = text.split("%");
  const slotCount = Math.max(0, parts.length - 1);
  const errors = [];
  const warnings = [];

  if (slotCount === 0) {
    return {
      isTemplate: false,
      prefix: "",
      suffix: "",
      slotCount: 0,
      classification: "plain",
      errors,
      warnings,
    };
  }

  const parsed = parseTemplate(text);
  const prefix = parsed ? parsed.prefix : "";
  const suffix = parsed ? parsed.suffix : "";
  const classification = classifyTemplate(prefix, suffix, slotCount);

  // No-anchor (both empty) — hard error.
  if (prefix.trim() === "" && suffix.trim() === "") {
    errors.push(
      `Intent expression \`${text}\` has no anchor — both prefix and suffix are empty. Add literal text around each \`%\` slot marker (e.g. \`read % file\`).`
    );
    return { isTemplate: true, prefix, suffix, slotCount, classification, errors, warnings };
  }

  // Leading-`%` (empty prefix, non-empty suffix) — warning. Exact emptiness
  // (no trim) matches parse_template / resolve_intent Path 2.
  if (prefix === "" && suffix !== "") {
    warnings.push(
      `Intent expression \`${text}\` is a leading-\`%\` template (no prefix anchor). It is valid and indexed via the suffix anchor, but imprecise — consider adding a literal word before \`%\` (e.g. \`read % directory\` instead of \`% directory\`).`
    );
  }

  // Adjacent slots — an empty inter-slot separator (parts[1..slotCount]) means
  // two `%` with no literal between them → unextractable. Catches middle-adjacent
  // cases like `"a %% b"` (empty separator with non-empty neighbours).
  for (let i = 1; i < slotCount; i += 1) {
    if (parts[i] === "") {
      errors.push(
        `Intent expression \`${text}\` has two adjacent \`%\` slot markers with no literal between them — slots would be unextractable.`
      );
      break;
    }
  }

  return { isTemplate: true, prefix, suffix, slotCount, classification, errors, warnings };
}
