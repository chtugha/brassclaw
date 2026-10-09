//! Qualified library declarations from the exact retained component graph.
//! This is assembly metadata, never activation evidence or Tool authority.
use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportDeclaration {
    pub symbol: String,
    pub parameters: Vec<String>,
    pub mapping: bool,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreloadDeclaration {
    pub format: String,
    pub exports: BTreeMap<String, ExportDeclaration>,
    pub private_functions: BTreeMap<String, Vec<String>>,
    pub constants: Vec<String>,
    pub imports: Vec<String>,
    pub dependencies: Vec<Uuid>,
    pub default_export: Option<String>,
}

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
impl PreloadDeclaration {
    pub fn parse(
        document: &Value,
        dependencies: &BTreeSet<Uuid>,
    ) -> Result<Option<Self>, &'static str> {
        let Some(raw) = document.get("preload") else {
            return Ok(None);
        };
        let declaration: Self =
            serde_json::from_value(raw.clone()).map_err(|_| "invalid preload declaration")?;
        if declaration.format != "python-preload/2"
            || declaration
                .dependencies
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                != *dependencies
            || declaration.dependencies.len() != dependencies.len()
            || declaration
                .default_export
                .as_ref()
                .is_some_and(|name| !declaration.exports.contains_key(name))
        {
            return Err("preload identity, dependency or default-export mismatch");
        }
        let mut symbols = BTreeSet::new();
        for (name, export) in &declaration.exports {
            if !identifier(name)
                || name.starts_with('_')
                || !identifier(&export.symbol)
                || !symbols.insert(export.symbol.clone())
                || (export.mapping && export.parameters != ["inputs"])
                || (!export.mapping
                    && export
                        .parameters
                        .iter()
                        .any(|name| name != "inputs" && !identifier(name)))
                || export.parameters.iter().collect::<BTreeSet<_>>().len()
                    != export.parameters.len()
            {
                return Err("invalid or conflicting export signature");
            }
        }
        for (name, parameters) in &declaration.private_functions {
            if !identifier(name)
                || !symbols.insert(name.clone())
                || parameters
                    .iter()
                    .any(|name| name != "inputs" && !identifier(name))
                || parameters.iter().collect::<BTreeSet<_>>().len() != parameters.len()
            {
                return Err("invalid private function declaration");
            }
        }
        for name in &declaration.constants {
            if !identifier(name) || !symbols.insert(name.clone()) {
                return Err("invalid constant declaration");
            }
        }
        if declaration.imports.iter().collect::<BTreeSet<_>>().len() != declaration.imports.len() {
            return Err("duplicate qualified imports");
        }
        Ok(Some(declaration))
    }
    pub fn symbols(&self) -> BTreeSet<String> {
        self.exports
            .values()
            .map(|e| e.symbol.clone())
            .chain(self.private_functions.keys().cloned())
            .chain(self.constants.iter().cloned())
            .collect()
    }
    pub fn invocation(&self, name: &str, input_contract: &Value) -> Result<String, &'static str> {
        let export = self.exports.get(name).ok_or("unknown invocation export")?;
        if export.mapping {
            return Ok(format!("result = {}(inputs=inputs)", export.symbol));
        }
        let names = input_contract
            .as_object()
            .ok_or("input contract missing")?
            .keys()
            .collect::<BTreeSet<_>>();
        if export.parameters.iter().collect::<BTreeSet<_>>() != names {
            return Err("export signature differs from input contract");
        }
        Ok(format!(
            "result = {}({})",
            export.symbol,
            export
                .parameters
                .iter()
                .map(|name| format!("{name}=inputs['{name}']"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// Only pinned declarations and the interpreter's qualified pure builtins may
/// satisfy free symbols. Imports are validated independently by source inspection.
pub fn builtin(name: &str) -> bool {
    matches!(
        name,
        "host"
            | "abs"
            | "all"
            | "any"
            | "bool"
            | "bytes"
            | "bytearray"
            | "chr"
            | "dict"
            | "divmod"
            | "enumerate"
            | "filter"
            | "float"
            | "format"
            | "frozenset"
            | "hash"
            | "hex"
            | "int"
            | "isinstance"
            | "issubclass"
            | "iter"
            | "len"
            | "list"
            | "map"
            | "max"
            | "min"
            | "next"
            | "ord"
            | "pow"
            | "range"
            | "repr"
            | "reversed"
            | "round"
            | "set"
            | "slice"
            | "sorted"
            | "str"
            | "sum"
            | "tuple"
            | "type"
            | "zip"
            | "Exception"
            | "ValueError"
            | "TypeError"
            | "KeyError"
            | "IndexError"
            | "RuntimeError"
            | "AssertionError"
            | "ZeroDivisionError"
            | "StopIteration"
    )
}
