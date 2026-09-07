//! Variable intent templates — `%` slot marker support (Phase M, §0.17).
//!
//! Authors can write an intent expression like
//! `"show me all files in the % directory"`; `resolve_intent` (Phase M.3) matches
//! user text against the template via PostgreSQL `LIKE`, and
//! [`parse_template`] splits the expression into the literal anchor segments
//! that drive the three-path index dispatch (§0.17.1). After a template match,
//! `capture_variables` (in [`crate::memory::instruction_builder`]) walks the
//! literal segments left-to-right against the concrete user text and returns
//! the `%`-captured values as positional `slot0` / `slot1` / … pairs (§0.17.3),
//! feeding the `{{vars.slotN}}` substitution step (M.4).
//!
//! - **prefix** = text before the FIRST `%`
//! - **suffix** = text after the LAST `%`
//!
//! A template is **anchored** iff at least one of prefix/suffix is non-empty.
//! The no-anchor case (`"%"`, `"% in %"`, `"% %"`) is a Q1 hard error and never
//! reaches the DB. [`parse_template`] is anchor-agnostic — it only computes the
//! split; the Q1 anchor check lives in the validator (Phase I / component
//! validator).
//!
//! # Examples
//!
//! | expression | `parse_template` result | anchor |
//! |------------|------------------------|--------|
//! | `"no slots here"` | `None` | — (not a template) |
//! | `"show me files in the % directory"` | `Some(("show me files in the ", " directory"))` | dual |
//! | `"search for %"` | `Some(("search for ", ""))` | prefix |
//! | `"% directory"` | `Some(("", " directory"))` | suffix |
//! | `"%"` | `Some(("", ""))` | none (Q1 hard error) |
//! | `"% in %"` | `Some(("", ""))` | none (Q1 hard error) |

/// Split a template expression into `(prefix, suffix)` literal anchors.
///
/// Returns `None` when `expression` contains no `%` (a plain exact-match
/// intent, not a template). Returns `Some((prefix, suffix))` when at least one
/// `%` is present:
/// - `prefix` = everything before the FIRST `%`
/// - `suffix` = everything after the LAST `%`
///
/// Adjacent-slot validation (two `%` with no literal between) and the no-anchor
/// check (both prefix and suffix empty) are Q1 concerns handled by the caller
/// (component validator), not here.
pub fn parse_template(expression: &str) -> Option<(String, String)> {
    if !expression.contains('%') {
        return None;
    }
    // `split('%')` always yields at least one element (the whole string when
    // there is no `%`, but the `contains` guard above already ruled that out),
    // so `.next()` is infallible here — `unwrap_or("")` is defensive only.
    // `.next()` takes the first segment = text before the FIRST `%`.
    let prefix = expression.split('%').next().unwrap_or("").to_string();
    // `rsplit('%')` iterates from the right; `.next()` = text after the LAST `%`.
    let suffix = expression.rsplit('%').next().unwrap_or("").to_string();
    Some((prefix, suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_slot_is_not_a_template() {
        assert_eq!(parse_template("no slots here"), None);
    }

    #[test]
    fn empty_string_is_not_a_template() {
        assert_eq!(parse_template(""), None);
    }

    #[test]
    fn dual_anchored_template() {
        assert_eq!(
            parse_template("show me files in the % directory"),
            Some(("show me files in the ".to_string(), " directory".to_string()))
        );
    }

    #[test]
    fn prefix_anchored_trailing_slot() {
        // FIND-P9-12: trailing `%`, suffix empty, prefix-anchored (valid).
        assert_eq!(
            parse_template("search for %"),
            Some(("search for ".to_string(), "".to_string()))
        );
    }

    #[test]
    fn suffix_anchored_leading_slot() {
        assert_eq!(
            parse_template("% directory"),
            Some(("".to_string(), " directory".to_string()))
        );
    }

    #[test]
    fn bare_slot_no_anchor() {
        // Both anchors empty → Q1 hard error (no-anchor rule). parse_template
        // still returns the split; the validator rejects it.
        assert_eq!(
            parse_template("%"),
            Some(("".to_string(), "".to_string()))
        );
    }

    #[test]
    fn leading_and_trailing_slot_no_anchor() {
        // "% in %" → last `%` is at the end, so suffix is "" and prefix is "".
        // No anchor → Q1 hard error.
        assert_eq!(
            parse_template("% in %"),
            Some(("".to_string(), "".to_string()))
        );
    }

    #[test]
    fn two_slots_prefix_anchored() {
        // "search for % in %" → prefix = "search for ", suffix = "" (last `%`
        // at end). Prefix-anchored, valid.
        assert_eq!(
            parse_template("search for % in %"),
            Some(("search for ".to_string(), "".to_string()))
        );
    }

    #[test]
    fn two_slots_dual_anchored() {
        assert_eq!(
            parse_template("search for % in the % directory"),
            Some(("search for ".to_string(), " directory".to_string()))
        );
    }

    #[test]
    fn adjacent_slots_split_correctly() {
        // "% %" → first char is `%` (prefix = ""), last char is `%` (suffix = "").
        // No anchor → Q1 hard error (undefined behaviour blocked by Q1).
        assert_eq!(
            parse_template("% %"),
            Some(("".to_string(), "".to_string()))
        );
    }

    #[test]
    fn truly_adjacent_bare_slots() {
        // "%%" → prefix = "", suffix = "" (both `%` at the boundaries).
        // No anchor → Q1 hard error.
        assert_eq!(
            parse_template("%%"),
            Some(("".to_string(), "".to_string()))
        );
    }
}
