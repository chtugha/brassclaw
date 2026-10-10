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

/// Dependency-first order with the smallest stable UUID chosen at each ready
/// frontier. The complete graph is checked before any definitions are loaded.
pub(crate) fn dependency_order(
    libraries: &BTreeMap<Uuid, PreloadDeclaration>,
) -> Result<Vec<Uuid>, &'static str> {
    let mut remaining = BTreeMap::new();
    let mut dependents: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    let mut ready = BTreeSet::new();
    for (id, library) in libraries {
        remaining.insert(*id, library.dependencies.len());
        if library.dependencies.is_empty() {
            ready.insert(*id);
        }
        for dependency in &library.dependencies {
            if !libraries.contains_key(dependency) {
                return Err("preload dependency must be a qualified retained library");
            }
            dependents.entry(*dependency).or_default().push(*id);
        }
    }
    let mut order = Vec::with_capacity(libraries.len());
    while let Some(id) = ready.pop_first() {
        order.push(id);
        if let Some(children) = dependents.remove(&id) {
            for child in children {
                let count = remaining
                    .get_mut(&child)
                    .expect("dependent belongs to the checked graph");
                *count -= 1;
                if *count == 0 {
                    ready.insert(child);
                }
            }
        }
    }
    if order.len() != libraries.len() {
        return Err("preload dependency cycle");
    }
    Ok(order)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(dependencies: &[u128]) -> PreloadDeclaration {
        PreloadDeclaration {
            format: "python-preload/2".into(),
            exports: BTreeMap::new(),
            private_functions: BTreeMap::new(),
            constants: Vec::new(),
            imports: Vec::new(),
            dependencies: dependencies.iter().copied().map(Uuid::from_u128).collect(),
            default_export: None,
        }
    }

    #[test]
    fn ready_frontier_uses_stable_uuid_and_loads_shared_dependencies_once() {
        let libraries = BTreeMap::from([
            (Uuid::from_u128(1), declaration(&[3])),
            (Uuid::from_u128(2), declaration(&[3])),
            (Uuid::from_u128(3), declaration(&[5])),
            (Uuid::from_u128(4), declaration(&[])),
            (Uuid::from_u128(5), declaration(&[])),
            (Uuid::from_u128(6), declaration(&[])),
        ]);
        // A newly released smaller UUID precedes other ready nodes. Sorting
        // graph levels or insertion order instead would change this contract.
        assert_eq!(
            dependency_order(&libraries).unwrap(),
            [4, 5, 3, 1, 2, 6].map(Uuid::from_u128)
        );
        assert!(dependency_order(&BTreeMap::new()).unwrap().is_empty());
    }

    #[test]
    fn incomplete_graph_and_cycles_never_return_a_partial_load_order() {
        let mut libraries = BTreeMap::from([
            (Uuid::from_u128(1), declaration(&[2])),
            (Uuid::from_u128(2), declaration(&[1])),
            (Uuid::from_u128(3), declaration(&[])),
        ]);
        assert_eq!(
            dependency_order(&libraries),
            Err("preload dependency cycle")
        );
        libraries.remove(&Uuid::from_u128(2));
        assert_eq!(
            dependency_order(&libraries),
            Err("preload dependency must be a qualified retained library")
        );
        libraries.insert(Uuid::from_u128(1), declaration(&[1]));
        assert_eq!(
            dependency_order(&libraries),
            Err("preload dependency cycle")
        );
    }

    #[test]
    fn deep_dependency_chain_uses_no_recursive_stack_walk() {
        let count = 4096_u128;
        let mut libraries: BTreeMap<_, _> = (1..count)
            .map(|id| (Uuid::from_u128(id), declaration(&[id + 1])))
            .collect();
        libraries.insert(Uuid::from_u128(count), declaration(&[]));
        let expected: Vec<_> = (1..=count).rev().map(Uuid::from_u128).collect();
        assert_eq!(dependency_order(&libraries).unwrap(), expected);
    }
}
