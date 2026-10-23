//! One bounded mechanical inspection of named sources. No source executes and
//! no class requirements, workflow decisions or approvals are implemented here.
use std::collections::BTreeMap;

use monty::MontyRun;
use monty_types::{CompileOptions, ExcType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{ValueBudget, VmBounds, VmError, VmFailure, source_structure::SourceStructure};

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceInspection {
    Inspected {
        structure: Box<SourceStructure>,
    },
    /// Mechanical rejection under the selected compiler profile, not proof of
    /// semantic invalidity. Exception text stays private. Native resource and
    /// infrastructure failures remain errors of the whole operation.
    CompileRejected {
        source_checksum: String,
        category: CompilerRejection,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerRejection {
    Syntax,
    Unsupported,
    Import,
}

/// Borrow complete input before cloning/encoding it. Source IDs are opaque,
/// nonempty strings; they are neither Python variable names nor paths to read.
/// The acknowledged source ceiling covers the entire operation, not each row.
pub fn sources_within_bounds<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str)>,
    bounds: VmBounds,
    frame_limit: usize,
) -> bool {
    check_sources(sources.into_iter().map(Some), bounds, frame_limit)
}

/// The native Tool can check borrowed JSON values before allocating owned
/// strings for the utility request. A nonstring is rejected, never skipped.
pub fn source_values_within_bounds(
    sources: &serde_json::Map<String, Value>,
    bounds: VmBounds,
    frame_limit: usize,
) -> bool {
    check_sources(
        sources
            .iter()
            .map(|(id, value)| value.as_str().map(|source| (id.as_str(), source))),
        bounds,
        frame_limit,
    )
}

fn check_sources<'a>(
    sources: impl IntoIterator<Item = Option<(&'a str, &'a str)>>,
    bounds: VmBounds,
    frame_limit: usize,
) -> bool {
    if !bounds.valid() || frame_limit == 0 {
        return false;
    }
    let mut source_bytes = 0usize;
    let mut budget = ValueBudget::new(bounds, VmFailure::ResourceLimit);
    if budget.node(0).is_err() {
        return false;
    }
    for source in sources {
        let Some((id, source)) = source else {
            return false;
        };
        if id.is_empty()
            || budget.node(1).is_err()
            || budget.bytes(id.len()).is_err()
            || budget.bytes(source.len()).is_err()
        {
            return false;
        }
        let Some(total) = source_bytes.checked_add(source.len()) else {
            return false;
        };
        source_bytes = total;
        if source_bytes > bounds.max_source_bytes || budget.bytes > frame_limit {
            return false;
        }
    }
    true
}

struct InventoryBudget(ValueBudget);
impl InventoryBudget {
    fn new(bounds: VmBounds) -> Result<Self, VmError> {
        let mut budget = ValueBudget::new(bounds, VmFailure::ResourceLimit);
        // Die vollständige UtilityOutput-Hülle gehört zum selben Datenbudget.
        budget.node(0)?;
        budget.bytes("kind".len() + "inspected_sources".len() + "sources".len())?;
        budget.node(1)?;
        budget.node(1)?;
        Ok(Self(budget))
    }
    fn entry(&mut self, id: &str, inspection: &SourceInspection) -> Result<(), VmError> {
        self.0.bytes(id.len())?;
        // Nur der neue, bereits begrenzte Datensatz wird vorübergehend
        // abgebildet; das bisherige Inventar wird niemals vollständig kopiert.
        let value = serde_json::to_value(inspection)
            .map_err(|_| VmError::kind(VmFailure::InvalidResult))?;
        let mut pending = vec![(&value, 2usize)];
        while let Some((value, depth)) = pending.pop() {
            self.0.node(depth)?;
            match value {
                Value::String(text) => self.0.bytes(text.len())?,
                Value::Array(values) => {
                    self.require_pending(pending.len(), values.len())?;
                    pending.extend(values.iter().map(|value| (value, depth + 1)));
                }
                Value::Object(values) => {
                    self.require_pending(pending.len(), values.len())?;
                    for (key, value) in values {
                        self.0.bytes(key.len())?;
                        pending.push((value, depth + 1));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn require_pending(&self, pending: usize, additional: usize) -> Result<(), VmError> {
        if pending
            .checked_add(additional)
            .is_none_or(|count| count > self.0.bounds.max_value_nodes - self.0.nodes)
        {
            Err(VmError::kind(VmFailure::ResourceLimit))
        } else {
            Ok(())
        }
    }
}

pub(crate) fn inspect(
    sources: &BTreeMap<String, String>,
    bounds: VmBounds,
) -> Result<BTreeMap<String, SourceInspection>, VmError> {
    let mut output = BTreeMap::new();
    let mut budget = InventoryBudget::new(bounds)?;
    let mut inspected_nodes = 0usize;
    for (id, source) in sources {
        // Ein Prozess, eine Gesamtfrist und ein Allokationslimit. Nach der
        // Übersetzung wird der Runner vor der nächsten Quelle freigegeben.
        let inspection = match MontyRun::new(
            source.clone(),
            "inspection.py",
            vec![],
            CompileOptions::default(),
        ) {
            Ok(runner) => {
                drop(runner);
                let mut remaining = bounds;
                remaining.max_value_nodes = bounds
                    .max_value_nodes
                    .checked_sub(inspected_nodes)
                    .ok_or_else(|| VmError::kind(VmFailure::ResourceLimit))?;
                let structure = crate::source_structure::inspect(source, remaining)?;
                inspected_nodes = inspected_nodes
                    .checked_add(structure.inspected_nodes)
                    .ok_or_else(|| VmError::kind(VmFailure::ResourceLimit))?;
                SourceInspection::Inspected {
                    structure: Box::new(structure),
                }
            }
            Err(exception) => {
                let category = match exception.exc_type() {
                    ExcType::SyntaxError => CompilerRejection::Syntax,
                    ExcType::NotImplementedError => CompilerRejection::Unsupported,
                    ExcType::ImportError => CompilerRejection::Import,
                    other => {
                        return Err(VmError {
                            failure: if matches!(
                                other,
                                ExcType::MemoryError | ExcType::RecursionError
                            ) {
                                VmFailure::ResourceLimit
                            } else {
                                VmFailure::Python
                            },
                            exception: Some(Box::new(exception)),
                            stdout: String::new(),
                            rejected_answer: None,
                        });
                    }
                };
                SourceInspection::CompileRejected {
                    source_checksum: format!("{:x}", Sha256::digest(source.as_bytes())),
                    category,
                }
            }
        };
        budget.entry(id, &inspection)?;
        output.insert(id.clone(), inspection);
    }
    Ok(output)
}

pub(crate) fn valid_for(
    output: &BTreeMap<String, SourceInspection>,
    sources: &BTreeMap<String, String>,
    bounds: VmBounds,
) -> bool {
    if output.len() != sources.len() {
        return false;
    }
    let Ok(mut budget) = InventoryBudget::new(bounds) else {
        return false;
    };
    let mut inspected_nodes = 0usize;
    for (id, source) in sources {
        let Some(inspection) = output.get(id) else {
            return false;
        };
        match inspection {
            SourceInspection::Inspected { structure } => {
                if !structure.valid_for(source, bounds) {
                    return false;
                }
                let Some(total) = inspected_nodes.checked_add(structure.inspected_nodes) else {
                    return false;
                };
                inspected_nodes = total;
                if inspected_nodes > bounds.max_value_nodes {
                    return false;
                }
            }
            SourceInspection::CompileRejected {
                source_checksum, ..
            } => {
                if *source_checksum != format!("{:x}", Sha256::digest(source.as_bytes())) {
                    return false;
                }
            }
        }
        if budget.entry(id, inspection).is_err() {
            return false;
        }
    }
    true
}
