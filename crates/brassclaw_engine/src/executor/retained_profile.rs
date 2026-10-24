//! Read-only facts about the existing retained host preflight. These names are
//! neither component approval nor Tool permission or a complete Monty feature
//! description. A review Recipe pins these facts with its own immutable version;
//! runtime preflight and live kernel enforcement remain independent.

macro_rules! named_set {
    ($names:ident, $visibility:vis $predicate:ident, [$($name:literal),+ $(,)?]) => {
        const $names: &[&str] = &[$($name),+];
        $visibility fn $predicate(name: &str) -> bool {
            matches!(name, $($name)|+)
        }
    };
}

named_set!(BUILTINS, pub builtin, [
    "host", "abs", "all", "any", "bool", "bytes", "bytearray", "chr", "dict",
    "divmod", "enumerate", "filter", "float", "format", "frozenset", "hash",
    "hex", "int", "isinstance", "issubclass", "iter", "len", "list", "map",
    "max", "min", "next", "ord", "pow", "range", "repr", "reversed", "round",
    "set", "slice", "sorted", "str", "sum", "tuple", "type", "zip", "Exception",
    "ValueError", "TypeError", "KeyError", "IndexError", "RuntimeError",
    "AssertionError", "ZeroDivisionError", "StopIteration",
]);
named_set!(IMPORTS, pub(crate) supported_import, [
    "asyncio", "base64", "binascii", "collections", "copy", "dataclasses",
    "datetime", "functools", "itertools", "json", "math", "pathlib", "random",
    "re", "sys", "time", "typing", "unicodedata",
]);

#[derive(serde::Serialize)]
pub struct RetainedPythonProfile {
    format: &'static str,
    builtin_names: &'static [&'static str],
    import_names: &'static [&'static str],
}

/// Construct only from the same declarations used by the actual preflight.
/// Consumers may serialize these mechanical facts as typed Recipe constants;
/// reviewed candidate data must not supply or replace that selection.
pub fn profile() -> RetainedPythonProfile {
    RetainedPythonProfile {
        format: "retained-python-profile/1",
        builtin_names: BUILTINS,
        import_names: IMPORTS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_serialization_preserves_actual_preflight_name_boundaries() {
        let value = serde_json::to_value(profile()).unwrap();
        assert_eq!(value["format"], "retained-python-profile/1");
        assert_eq!(value.as_object().unwrap().len(), 3);
        for (field, predicate) in [
            ("builtin_names", builtin as fn(&str) -> bool),
            ("import_names", supported_import as fn(&str) -> bool),
        ] {
            let names = value[field].as_array().unwrap();
            let mut seen = std::collections::BTreeSet::new();
            for name in names {
                let name = name.as_str().unwrap();
                assert!(!name.is_empty() && seen.insert(name) && predicate(name));
            }
        }
        for name in ["host", "sum", "ValueError"] {
            assert!(builtin(name));
        }
        for name in ["exec", "eval", "open", "__import__", "imaginary"] {
            assert!(!builtin(name));
        }
        for name in ["json", "re", "typing"] {
            assert!(supported_import(name));
        }
        for name in ["os", "subprocess", "json.decoder", "imaginary"] {
            assert!(!supported_import(name));
        }
    }
}
