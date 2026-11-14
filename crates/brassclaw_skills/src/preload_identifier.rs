//! Shared retained-preload name rule. Moved unchanged from the engine owner;
//! lexical acceptance grants no export resolution, qualification or authority.

pub fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|b| b == b'_' || b.is_ascii_alphabetic())
        && bytes.all(|b| b == b'_' || b.is_ascii_alphanumeric())
        && !matches!(
            value,
            "host"
                | "inputs"
                | "result"
                | "exec"
                | "eval"
                | "open"
                | "__import__"
                | "False"
                | "None"
                | "True"
                | "and"
                | "as"
                | "assert"
                | "async"
                | "await"
                | "break"
                | "class"
                | "continue"
                | "def"
                | "del"
                | "elif"
                | "else"
                | "except"
                | "finally"
                | "for"
                | "from"
                | "global"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "nonlocal"
                | "not"
                | "or"
                | "pass"
                | "raise"
                | "return"
                | "try"
                | "while"
                | "with"
                | "yield"
        )
}

/// A selectable public export uses the same lexical rule as private symbols,
/// with the existing additional prohibition on a leading underscore.
pub fn public_export(value: &str) -> bool {
    identifier(value) && !value.starts_with('_')
}
