//! Contained parser observations for trusted review infrastructure.
//! These are syntactic sites, not reachable-effect counts, semantic approval,
//! an executable manifest or Tool permission. Strings/comments are never code.

use std::collections::BTreeSet;

use ruff_python_ast::{
    Expr, ExprContext, Stmt,
    visitor::{Visitor, walk_expr, walk_stmt},
};
use ruff_text_size::Ranged;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{VmBounds, VmError, VmFailure};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostCallSite {
    pub attribute: String,
    pub start: u32,
    pub end: u32,
    pub repeatable: bool,
}

/// Exact-source observations. All syntax is inspected, including functions and
/// unreachable branches; this cannot prove that a call will execute or that
/// several calls form the permitted dependent chain. Trusted Q1/Q2 and actual
/// behavioral validation must make those decisions separately.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionScope {
    pub references: BTreeSet<String>,
    pub calls: std::collections::BTreeMap<String, usize>,
    pub value_references: BTreeSet<String>,
    pub nested_functions: bool,
    pub start: u32,
    pub end: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreloadSource {
    pub scopes: std::collections::BTreeMap<String, FunctionScope>,
    pub functions: std::collections::BTreeMap<String, Vec<String>>,
    pub constants: BTreeSet<String>,
    pub references: BTreeSet<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceStructure {
    pub source_checksum: String,
    pub preload: Option<PreloadSource>,
    pub direct_host_calls: Vec<HostCallSite>,
    pub imports: BTreeSet<String>,
    pub relative_imports: bool,
    pub host_value_references: usize,
    pub reserved_name_references: BTreeSet<String>,
    pub result_store_sites: usize,
    pub inspected_nodes: usize,
}

impl SourceStructure {
    pub(crate) fn valid_for(&self, source: &str, bounds: VmBounds) -> bool {
        self.within_value_bounds(bounds)
            && self.source_checksum == format!("{:x}", Sha256::digest(source.as_bytes()))
            && self.inspected_nodes <= bounds.max_value_nodes
            && self.direct_host_calls.len() <= self.inspected_nodes
            && self.host_value_references <= self.inspected_nodes
            && self.result_store_sites <= self.inspected_nodes
            && self.direct_host_calls.iter().all(|site| {
                site.start < site.end
                    && source.get(site.start as usize..site.end as usize).is_some()
                    && !site.attribute.is_empty()
                    && site.attribute.len() <= source.len()
            })
            && self
                .imports
                .iter()
                .all(|name| !name.is_empty() && name.len() <= source.len())
            && self
                .reserved_name_references
                .iter()
                .all(|name| reserved(name))
    }

    fn within_value_bounds(&self, bounds: VmBounds) -> bool {
        let Ok(value) = serde_json::to_value(self) else {
            return false;
        };
        let mut pending = vec![(&value, 0usize)];
        let mut nodes = 0usize;
        let mut bytes = 0usize;
        while let Some((value, depth)) = pending.pop() {
            nodes = nodes.saturating_add(1);
            if depth > bounds.max_value_depth || nodes > bounds.max_value_nodes {
                return false;
            }
            match value {
                serde_json::Value::String(text) => bytes = bytes.saturating_add(text.len()),
                serde_json::Value::Array(values) => {
                    pending.extend(values.iter().map(|v| (v, depth + 1)))
                }
                serde_json::Value::Object(values) => {
                    bytes = bytes.saturating_add(values.keys().map(String::len).sum::<usize>());
                    nodes = nodes.saturating_add(values.len());
                    pending.extend(values.values().map(|v| (v, depth + 1)));
                }
                _ => {}
            }
            if bytes > bounds.max_value_bytes || nodes > bounds.max_value_nodes {
                return false;
            }
        }
        true
    }
}

fn reserved(name: &str) -> bool {
    matches!(
        name,
        "exec"
            | "eval"
            | "open"
            | "__import__"
            | "__execute_action__"
            | "__execute_code_step__"
            | "__execute_actions_parallel__"
            | "__check_budget__"
            | "__emit_event__"
    )
}

struct Inspector {
    report: SourceStructure,
    depth: usize,
    max_depth: usize,
    max_nodes: usize,
    exhausted: bool,
    unsafe_definitions: bool,
    loop_depth: usize,
}
impl Inspector {
    fn enter(&mut self) -> bool {
        if self.exhausted
            || self.depth >= self.max_depth
            || self.report.inspected_nodes >= self.max_nodes
        {
            self.exhausted = true;
            return false;
        }
        self.report.inspected_nodes += 1;
        self.depth += 1;
        true
    }
}
impl<'a> Visitor<'a> for Inspector {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        if !self.enter() {
            return;
        }
        match statement {
            Stmt::Global(_) | Stmt::Nonlocal(_) => self.unsafe_definitions = true,
            Stmt::FunctionDef(function) if definition_parameters(function).is_none() => {
                self.unsafe_definitions = true
            }
            Stmt::Import(import) => {
                self.report
                    .imports
                    .extend(import.names.iter().map(|name| name.name.to_string()));
            }
            Stmt::ImportFrom(import) => {
                self.report.relative_imports |= import.level > 0;
                if let Some(module) = &import.module {
                    self.report.imports.insert(module.to_string());
                }
            }
            _ => {}
        }
        let repeatable = matches!(statement, Stmt::For(_) | Stmt::While(_));
        self.loop_depth += usize::from(repeatable);
        walk_stmt(self, statement);
        self.loop_depth -= usize::from(repeatable);
        self.depth -= 1;
    }
    fn visit_expr(&mut self, expression: &'a Expr) {
        if !self.enter() {
            return;
        }
        let repeatable = matches!(
            expression,
            Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Generator(_)
        );
        self.loop_depth += usize::from(repeatable);
        match expression {
            Expr::Call(call)
                if matches!(call.func.as_ref(), Expr::Attribute(attr)
                if matches!(attr.value.as_ref(), Expr::Name(name) if name.id == "host" && name.ctx == ExprContext::Load)) =>
            {
                let Expr::Attribute(attr) = call.func.as_ref() else {
                    unreachable!()
                };
                self.report.direct_host_calls.push(HostCallSite {
                    attribute: attr.attr.to_string(),
                    start: call.start().to_u32(),
                    end: call.end().to_u32(),
                    repeatable: self.loop_depth > 0,
                });
                // The receiver of a direct call is not a first-class host value.
                // Inspect all arguments, including nested independent calls.
                self.visit_arguments(&call.arguments);
            }
            Expr::Name(name) => {
                if name.id == "host" {
                    self.report.host_value_references += 1;
                }
                if reserved(name.id.as_str()) {
                    self.report
                        .reserved_name_references
                        .insert(name.id.to_string());
                }
                if name.id == "result" && name.ctx == ExprContext::Store {
                    self.report.result_store_sites += 1;
                }
            }
            _ => walk_expr(self, expression),
        }
        self.loop_depth -= usize::from(repeatable);
        self.depth -= 1;
    }
}

fn definition_parameters(function: &ruff_python_ast::StmtFunctionDef) -> Option<Vec<String>> {
    if function.is_async
        || !function.decorator_list.is_empty()
        || function.type_params.is_some()
        || function.returns.is_some()
        || !function.parameters.posonlyargs.is_empty()
        || function.parameters.vararg.is_some()
        || function.parameters.kwarg.is_some()
    {
        return None;
    }
    function
        .parameters
        .args
        .iter()
        .chain(&function.parameters.kwonlyargs)
        .map(|parameter| {
            if parameter.default.is_some() || parameter.parameter.annotation.is_some() {
                None
            } else {
                Some(parameter.name().to_string())
            }
        })
        .collect()
}
fn immutable_expression(expression: &Expr) -> bool {
    match expression {
        Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_) => true,
        Expr::Tuple(tuple) => tuple.elts.iter().all(immutable_expression),
        Expr::UnaryOp(unary)
            if matches!(
                unary.op,
                ruff_python_ast::UnaryOp::UAdd | ruff_python_ast::UnaryOp::USub
            ) =>
        {
            matches!(unary.operand.as_ref(), Expr::NumberLiteral(_))
        }
        _ => false,
    }
}
#[derive(Default)]
struct FunctionNames {
    read: BTreeSet<String>,
    bound: BTreeSet<String>,
    calls: std::collections::BTreeMap<String, usize>,
    value_references: BTreeSet<String>,
    nested_functions: bool,
    loop_depth: usize,
}
impl<'a> Visitor<'a> for FunctionNames {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            Stmt::Import(import) => {
                for alias in &import.names {
                    self.bound.insert(alias.asname.as_ref().map_or_else(
                        || {
                            alias
                                .name
                                .as_str()
                                .split('.')
                                .next()
                                .unwrap_or_default()
                                .to_owned()
                        },
                        ToString::to_string,
                    ));
                }
            }
            Stmt::ImportFrom(import) => {
                for alias in &import.names {
                    self.bound
                        .insert(alias.asname.as_ref().unwrap_or(&alias.name).to_string());
                }
            }
            Stmt::FunctionDef(function) => {
                self.nested_functions = true;
                self.bound.insert(function.name.to_string());
                // A nested function has its own local scope. Its free names
                // may capture this function's locals, but its parameters and
                // assignments must never hide this function's free names.
                let mut nested = FunctionNames::default();
                if let Some(parameters) = definition_parameters(function) {
                    nested.bound.extend(parameters);
                }
                nested.visit_body(&function.body);
                self.read
                    .extend(nested.read.difference(&nested.bound).cloned());
                self.value_references
                    .extend(nested.value_references.difference(&nested.bound).cloned());
                for (name, count) in nested.calls {
                    if !nested.bound.contains(&name) {
                        let value = self.calls.entry(name).or_default();
                        *value = value.saturating_add(count).min(2);
                    }
                }
                return;
            }
            _ => {}
        }
        let repeatable = matches!(statement, Stmt::For(_) | Stmt::While(_));
        self.loop_depth += usize::from(repeatable);
        walk_stmt(self, statement);
        self.loop_depth -= usize::from(repeatable);
    }
    fn visit_expr(&mut self, expression: &'a Expr) {
        let comprehension = match expression {
            Expr::ListComp(value) => Some((value.generators.as_slice(), vec![value.elt.as_ref()])),
            Expr::SetComp(value) => Some((value.generators.as_slice(), vec![value.elt.as_ref()])),
            Expr::Generator(value) => Some((value.generators.as_slice(), vec![value.elt.as_ref()])),
            Expr::DictComp(value) => Some((
                value.generators.as_ref(),
                value
                    .key
                    .as_deref()
                    .into_iter()
                    .chain(std::iter::once(value.value.as_ref()))
                    .collect(),
            )),
            _ => None,
        };
        if let Some((generators, values)) = comprehension {
            let mut nested = FunctionNames {
                loop_depth: 1,
                ..Default::default()
            };
            for (index, generator) in generators.iter().enumerate() {
                if index == 0 {
                    self.visit_expr(&generator.iter);
                } else {
                    nested.visit_expr(&generator.iter);
                }
                nested.visit_expr(&generator.target);
                for predicate in &generator.ifs {
                    nested.visit_expr(predicate);
                }
            }
            for value in values {
                nested.visit_expr(value);
            }
            self.read
                .extend(nested.read.difference(&nested.bound).cloned());
            self.value_references
                .extend(nested.value_references.difference(&nested.bound).cloned());
            for (name, count) in nested.calls {
                if !nested.bound.contains(&name) {
                    let value = self.calls.entry(name).or_default();
                    *value = value.saturating_add(count).min(2);
                }
            }
            return;
        }
        if let Expr::Call(call) = expression
            && let Expr::Name(name) = call.func.as_ref()
        {
            self.read.insert(name.id.to_string());
            let count = self.calls.entry(name.id.to_string()).or_default();
            *count = count
                .saturating_add(if self.loop_depth > 0 { 2 } else { 1 })
                .min(2);
            self.visit_arguments(&call.arguments);
            return;
        }
        if let Expr::Name(name) = expression {
            match name.ctx {
                ExprContext::Load => {
                    self.read.insert(name.id.to_string());
                    self.value_references.insert(name.id.to_string());
                }
                ExprContext::Store | ExprContext::Del => {
                    self.bound.insert(name.id.to_string());
                }
                _ => {}
            }
        }
        walk_expr(self, expression);
    }
}

fn preload_structure(body: &[Stmt]) -> Option<PreloadSource> {
    let mut library = PreloadSource {
        scopes: Default::default(),
        functions: Default::default(),
        constants: Default::default(),
        references: Default::default(),
    };
    for statement in body {
        match statement {
            Stmt::FunctionDef(function) => {
                let name = function.name.to_string();
                let mut names = FunctionNames::default();
                names.bound.extend(definition_parameters(function)?);
                names.visit_body(&function.body);
                let references: BTreeSet<_> =
                    names.read.difference(&names.bound).cloned().collect();
                library.references.extend(references.iter().cloned());
                library.scopes.insert(
                    name.clone(),
                    FunctionScope {
                        calls: names
                            .calls
                            .into_iter()
                            .filter(|(name, _)| !names.bound.contains(name))
                            .collect(),
                        value_references: names
                            .value_references
                            .difference(&names.bound)
                            .cloned()
                            .collect(),
                        nested_functions: names.nested_functions,
                        references,
                        start: function.start().to_u32(),
                        end: function.end().to_u32(),
                    },
                );
                if library.constants.contains(&name)
                    || library
                        .functions
                        .insert(name, definition_parameters(function)?)
                        .is_some()
                {
                    return None;
                }
            }
            Stmt::Assign(assign)
                if assign.targets.len() == 1 && immutable_expression(&assign.value) =>
            {
                let Expr::Name(name) = &assign.targets[0] else {
                    return None;
                };
                if library.functions.contains_key(name.id.as_str())
                    || !library.constants.insert(name.id.to_string())
                {
                    return None;
                }
            }
            Stmt::Expr(expression)
                if matches!(expression.value.as_ref(), Expr::StringLiteral(_)) => {}
            _ => return None,
        }
    }
    (!library.functions.is_empty() || !library.constants.is_empty()).then_some(library)
}

/// Only called in the contained utility worker after actual Monty compilation.
/// Parser/traversal/native failure stays within its deadline/allocator boundary.
/// No host receiver is installed and no opcode executes in this operation.
pub(crate) fn inspect(source: &str, bounds: VmBounds) -> Result<SourceStructure, VmError> {
    let parsed =
        ruff_python_parser::parse_module(source).map_err(|_| VmError::kind(VmFailure::Python))?;

    let mut visitor = Inspector {
        report: SourceStructure {
            source_checksum: format!("{:x}", Sha256::digest(source.as_bytes())),
            preload: None,
            direct_host_calls: Vec::new(),
            imports: BTreeSet::new(),
            relative_imports: false,
            host_value_references: 0,
            reserved_name_references: BTreeSet::new(),
            result_store_sites: 0,
            inspected_nodes: 0,
        },
        depth: 0,
        max_depth: bounds.max_value_depth.min(128),
        max_nodes: bounds.max_value_nodes,
        exhausted: false,
        unsafe_definitions: false,
        loop_depth: 0,
    };
    visitor.visit_body(&parsed.syntax().body);
    if visitor.exhausted || !visitor.report.within_value_bounds(bounds) {
        return Err(VmError::kind(VmFailure::ResourceLimit));
    }
    if !visitor.unsafe_definitions {
        visitor.report.preload = preload_structure(&parsed.syntax().body);
    }
    if !visitor.report.within_value_bounds(bounds) {
        return Err(VmError::kind(VmFailure::ResourceLimit));
    }
    visitor
        .report
        .direct_host_calls
        .sort_by_key(|site| site.start);
    Ok(visitor.report)
}
