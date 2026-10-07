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
}

/// Exact-source observations. All syntax is inspected, including functions and
/// unreachable branches; this cannot prove that a call will execute or that
/// several calls form the permitted dependent chain. Trusted Q1/Q2 and actual
/// behavioral validation must make those decisions separately.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceStructure {
    pub source_checksum: String,
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
        let fields = [
            "source_checksum",
            "direct_host_calls",
            "imports",
            "relative_imports",
            "host_value_references",
            "reserved_name_references",
            "result_store_sites",
            "inspected_nodes",
        ];
        let mut bytes =
            fields.iter().map(|name| name.len()).sum::<usize>() + self.source_checksum.len();
        let Some(mut nodes) = self
            .direct_host_calls
            .len()
            .checked_mul(4)
            .and_then(|n| n.checked_add(9))
        else {
            return false;
        };
        for name in self.imports.iter().chain(&self.reserved_name_references) {
            let Some(next) = bytes.checked_add(name.len()) else {
                return false;
            };
            bytes = next;
            let Some(next) = nodes.checked_add(1) else {
                return false;
            };
            nodes = next;
        }
        for site in &self.direct_host_calls {
            let Some(next) = bytes
                .checked_add(site.attribute.len())
                .and_then(|n| n.checked_add("attribute".len() + "start".len() + "end".len()))
            else {
                return false;
            };
            bytes = next;
        }
        nodes <= bounds.max_value_nodes
            && bytes <= bounds.max_value_bytes
            && bounds.max_value_depth >= 3
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
        walk_stmt(self, statement);
        self.depth -= 1;
    }
    fn visit_expr(&mut self, expression: &'a Expr) {
        if !self.enter() {
            return;
        }
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
        self.depth -= 1;
    }
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
    };
    visitor.visit_body(&parsed.syntax().body);
    if visitor.exhausted || !visitor.report.within_value_bounds(bounds) {
        return Err(VmError::kind(VmFailure::ResourceLimit));
    }
    visitor
        .report
        .direct_host_calls
        .sort_by_key(|site| site.start);
    Ok(visitor.report)
}
