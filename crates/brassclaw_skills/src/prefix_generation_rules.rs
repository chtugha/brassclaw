//! Source-independent shape checks for retained reference-generation proposals.
//! Actual source resolution, coverage, measurement and acceptance remain with
//! the existing prefix Recipe. A valid declaration grants no serving support.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Largest integer represented exactly by all JSON/JavaScript consumers.
/// This is a protocol representation bound, never a model token budget.
const MAX_EXACT_INTEGER: u64 = (1_u64 << 53) - 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixGenerationRules {
    format: String,
    profile: String,
    header: String,
    bands: Vec<Band>,
    required_coverage: Vec<Selector>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Band {
    minimum_tokens: u64,
    maximum_tokens: u64,
    groups: Vec<Group>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Group {
    id: String,
    required: bool,
    priority: u64,
    dependencies: Vec<String>,
    selectors: Vec<Selector>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    path: String,
    r#match: String,
    first_lines: Vec<String>,
}
fn name(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
}
impl Selector {
    fn valid(&self) -> bool {
        let namespace = self.r#match == "prefix";
        if !namespace && self.r#match != "exact" {
            return false;
        }
        if self.path.is_empty() {
            if !namespace {
                return false;
            }
        } else {
            if !name(&self.path)
                || self.path.starts_with('/')
                || self.path.contains('\\')
                || (namespace && !self.path.ends_with('/'))
            {
                return false;
            }
            let path = if namespace {
                &self.path[..self.path.len() - 1]
            } else {
                &self.path
            };
            if path
                .split('/')
                .any(|part| part.is_empty() || part.starts_with('.'))
            {
                return false;
            }
        }
        self.first_lines.iter().all(|line| name(line))
            && self.first_lines.iter().collect::<BTreeSet<_>>().len() == self.first_lines.len()
    }
}
impl PrefixGenerationRules {
    /// Checks every size band, including inactive ones. Exact unit ownership and
    /// mandatory coverage require the captured sources and are checked later.
    pub fn valid_for_profile(&self, profile: &str) -> bool {
        if self.format != "brassclaw-prefix-generation-rules/1"
            || !name(profile)
            || self.profile != profile
            || self.header.trim().is_empty()
            || self.bands.is_empty()
            || self.required_coverage.is_empty()
            || !self.required_coverage.iter().all(Selector::valid)
        {
            return false;
        }
        let mut previous = None;
        for band in &self.bands {
            if band.minimum_tokens > band.maximum_tokens
                || band.maximum_tokens > MAX_EXACT_INTEGER
                || previous.is_some_and(|end| band.minimum_tokens <= end)
                || band.groups.is_empty()
                || !band.groups.iter().any(|group| group.required)
            {
                return false;
            }
            previous = Some(band.maximum_tokens);
            let names = band
                .groups
                .iter()
                .map(|group| group.id.clone())
                .collect::<BTreeSet<_>>();
            if names.len() != band.groups.len() {
                return false;
            }
            for group in &band.groups {
                if !name(&group.id)
                    || group.priority > MAX_EXACT_INTEGER
                    || group.selectors.is_empty()
                    || !group.selectors.iter().all(Selector::valid)
                    || group.dependencies.iter().collect::<BTreeSet<_>>().len()
                        != group.dependencies.len()
                    || group
                        .dependencies
                        .iter()
                        .any(|dependency| !names.contains(dependency))
                {
                    return false;
                }
            }
            // Validate large reverse-ordered graphs without repeated full scans.
            let mut incoming = band
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.dependencies.len()))
                .collect::<BTreeMap<_, _>>();
            let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for group in &band.groups {
                for dependency in &group.dependencies {
                    dependents
                        .entry(dependency.clone())
                        .or_default()
                        .push(group.id.clone());
                }
            }
            let mut ready = incoming
                .iter()
                .filter(|(_, count)| **count == 0)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            let mut position = 0;
            while position < ready.len() {
                for child in dependents.get(&ready[position]).into_iter().flatten() {
                    let Some(remaining) = incoming.get_mut(child) else {
                        return false;
                    };
                    *remaining -= 1;
                    if *remaining == 0 {
                        ready.push(child.clone());
                    }
                }
                position += 1;
            }
            if ready.len() != names.len() {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn example() -> Value {
        json!({"format":"brassclaw-prefix-generation-rules/1","profile":"sempai",
            "header":"Keep complete contracts and cite original sources.",
            "required_coverage":[{"path":"components/","match":"prefix","first_lines":[]}],
            "bands":[{"minimum_tokens":0,"maximum_tokens":100000,
                "groups":[{"id":"components","required":true,"priority":0,
                    "dependencies":[],"selectors":[{"path":"components/","match":"prefix","first_lines":[]}]}]}]})
    }
    fn valid(value: Value) -> bool {
        serde_json::from_value::<PrefixGenerationRules>(value)
            .is_ok_and(|rules| rules.valid_for_profile("sempai"))
    }
    #[test]
    fn validates_all_rule_bands_without_claiming_source_coverage() {
        let original = example();
        assert!(valid(original.clone()));
        let mut value = original.clone();
        value["bands"][0]["maximum_tokens"] = json!(MAX_EXACT_INTEGER);
        assert!(valid(value));
        let mut value = original.clone();
        let mut next = value["bands"][0].clone();
        next["minimum_tokens"] = json!(100001);
        next["maximum_tokens"] = json!(200000);
        value["bands"].as_array_mut().unwrap().push(next);
        assert!(valid(value.clone()));
        value["bands"][1]["groups"][0]["dependencies"] = json!(["missing"]);
        assert!(!valid(value));
        // The path is a declaration: its existence and all actual unit
        // coverage must be verified by the captured-source Recipe later.
        let mut value = original.clone();
        value["required_coverage"][0]["path"] = json!("future-components/");
        assert!(valid(value));
        for (path, replacement) in [
            ("/profile", json!("homeassistant")),
            ("/format", json!("unknown")),
            ("/header", json!("  ")),
            ("/required_coverage", json!([])),
            ("/required_coverage/0/path", json!("../private/")),
            ("/required_coverage/0/path", json!("components")),
            ("/required_coverage/0/path", json!(".env/")),
            ("/required_coverage/0/first_lines", json!(["A", "A"])),
            ("/bands", json!([])),
            ("/bands/0/maximum_tokens", json!(MAX_EXACT_INTEGER + 1)),
            ("/bands/0/maximum_tokens", json!(true)),
            ("/bands/0/minimum_tokens", json!(-1)),
            ("/bands/0/minimum_tokens", json!(100001)),
            ("/bands/0/groups/0/required", json!(false)),
            ("/bands/0/groups/0/priority", json!(1.5)),
            ("/bands/0/groups/0/id", json!("bad\nidentity")),
            ("/bands/0/groups/0/dependencies", json!(["components"])),
            ("/bands/0/groups/0/selectors", json!([])),
            ("/bands/0/groups/0/selectors/0/match", json!("regex")),
            ("/bands/0/groups/0/selectors/0/path", json!("components//")),
        ] {
            let mut value = original.clone();
            *value.pointer_mut(path).unwrap() = replacement;
            assert!(!valid(value), "{path}");
        }
        for path in ["", "/bands/0", "/bands/0/groups/0", "/required_coverage/0"] {
            let mut value = original.clone();
            value
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unrecognized".into(), json!(false));
            assert!(!valid(value), "{path}");
        }
        let mut value = original.clone();
        let group = value["bands"][0]["groups"][0].clone();
        value["bands"][0]["groups"]
            .as_array_mut()
            .unwrap()
            .push(group);
        assert!(!valid(value));
        let mut value = original;
        let band = value["bands"][0].clone();
        value["bands"].as_array_mut().unwrap().push(band);
        assert!(!valid(value));
    }
    #[test]
    fn reverse_ordered_large_graphs_and_cycles_are_checked_without_a_group_cutoff() {
        let mut value = example();
        value["bands"][0]["groups"] = Value::Array((0..1001).map(|index| {
            json!({"id":format!("group-{index}"),"required":index == 0,"priority":index,
                "dependencies":if index < 1000 { vec![format!("group-{}", index + 1)] } else { vec![] },
                "selectors":[{"path":format!("components/{index}.py"),"match":"exact","first_lines":[]}]})
        }).collect());
        assert!(valid(value.clone()));
        value["bands"][0]["groups"][1000]["dependencies"] = json!(["group-0"]);
        assert!(!valid(value));
    }
}
