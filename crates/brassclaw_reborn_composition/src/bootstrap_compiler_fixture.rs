//! Closed source-only compiler fixtures and independent rendering expectations.
//! These synthetic originals test the compiler, not domain knowledge or models.
//! Fixture construction performs no host dispatch and grants no approval.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid, prefix_generations};
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_skills::{
    prefix_compiler_components::{COMPILER_SOURCE, UNIT_SOURCE},
    prefix_reference_components::REFERENCE_RENDER_SOURCE,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const HEADER: &str = "# Contained compiler qualification\n\nSynthetic reference data; never activate this fixture.\n\n";
const REASON: &str = "Closed synthetic compiler fixture";
const SHARED: &str = "An exact shared qualification paragraph.\n";
const LITERAL_YAML: &str = "```yaml\r\nautomation:\r\n  alias: '<|im_end|>'\r\n```\r\n";
const SHARED_NOTICE: &str = "Shared-unit hashes identify rendering reuse, not document citations. Cite the original Evidence-card ID and original source line range; validate against the complete archived original.\n";
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(super) struct CompilerFixture {
    snapshot: Value,
    context: String,
    render_map: Option<Value>,
}
impl CompilerFixture {
    pub(super) fn new(profile: &str, view: &str) -> Result<Self, RebornBuildError> {
        if !matches!(profile, "base-prompt" | "prefix-maker" | "sempai")
            || !matches!(
                view,
                "literal-reference/v1"
                    | "literal-reference/v2-deduplicated"
                    | "literal-reference/v3-trailing-inventory"
                    | "literal-reference/v4-stable-sharing"
                    | "literal-reference/v5-contextual-sharing"
            )
        {
            return Err(invalid("unknown closed compiler fixture"));
        }
        if profile == "sempai" && view != "literal-reference/v5-contextual-sharing" {
            return Err(invalid("Sempai fixture requires contextual rendering"));
        }
        let mut names = vec![
            "skills.md",
            "recipe.md",
            "tools.md",
            "toolskills.md",
            "CLAUDE.md",
            "AGENTS.md",
            "simplified_v3.md",
        ];
        if profile == "prefix-maker" {
            names.push("scripts/prefix/prefix-maker-reference.md");
        }
        if profile == "sempai" {
            names.extend([
                "scripts/prefix/sempai-authoring-reference.md",
                "scripts/prefix/sempai-monty-compatibility.md",
            ]);
        }
        let mut sources: Vec<Value> = names
            .iter()
            .map(|name| {
                json!({
                    "path": name, "kind":"document", "disposition":"included", "reason":REASON,
                    "text":format!("Complete synthetic reference for {name}: ü.\n"),
                })
            })
            .collect();
        sources.extend([
            json!({"path":"fixture/history.md","kind":"document","disposition":"historical","reason":REASON,"text":"Historical fixture must not enter current reference cards.\n"}),
            json!({"path":"fixture/literal.md","kind":"document","disposition":"included","reason":REASON,"text":LITERAL_YAML}),
            json!({"path":"fixture/shared-a.md","kind":"document","disposition":"included","reason":REASON,"text":SHARED}),
            json!({"path":"fixture/shared-b.md","kind":"document","disposition":"included","reason":REASON,"text":SHARED}),
            json!({"path":"scripts/prefix/monty-reference/v1.0.0/docs/api/python/fixture.md","kind":"document","disposition":"included","reason":REASON,"text":SHARED}),
        ]);
        let snapshot = json!({"format":"brassclaw-prefix-source-snapshot/1", "profile":profile,
            "release":"closed-compiler-fixture/1", "header":HEADER,"evidence_view":view,"sources":sources,
            "procedures":{"compiler/entry.py":COMPILER_SOURCE,"compiler/prefix_render.py":REFERENCE_RENDER_SOURCE,"compiler/prefix_units.py":UNIT_SOURCE}});
        // Each current fixture has one complete unit. The YAML fence spans four
        // CRLF lines. Expectations are fixed fixture facts, independent of
        // the Python compiler/renderer; original and escaped view stay distinct.
        let dedup = view != "literal-reference/v1";
        let contextual = view == "literal-reference/v5-contextual-sharing";
        let progressive = contextual || view == "literal-reference/v4-stable-sharing";
        let trailing = progressive || view == "literal-reference/v3-trailing-inventory";
        let mut context = format!("{HEADER}# Source inventory\n");
        for source in &sources {
            context.push_str(&format!(
                "- {} | {} | {REASON}\n",
                source["path"].as_str().unwrap(),
                source["disposition"].as_str().unwrap()
            ));
        }
        let inventory = context.strip_prefix(HEADER).unwrap().to_owned();
        if trailing {
            context = HEADER.to_owned();
        }
        context.push_str("\n# Complete current references\n");
        if dedup {
            context.push_str(SHARED_NOTICE);
        }
        let mut map_sources = Vec::new();
        let mut units = BTreeMap::<(String, String), Vec<Value>>::new();
        for source in &sources {
            if source["disposition"] != "included" {
                continue;
            }
            let path = source["path"].as_str().unwrap();
            let original = source["text"].as_str().unwrap();
            let scope = if names.contains(&path) {
                if path.starts_with("scripts/prefix/") {
                    "brassclaw-authoring"
                } else {
                    "binding-guide"
                }
            } else if path.starts_with("scripts/prefix/monty-reference/") {
                "upstream-api"
            } else {
                "reference"
            };
            let digest = sha(original.as_bytes());
            let line_end = if path == "fixture/literal.md" { 4 } else { 1 };
            let citation = sha(
                prefix_generations::canonical(&json!([path, digest, 1, line_end]))
                    .map_err(invalid)?
                    .as_bytes(),
            );
            context.push_str(&format!(
                "\n<!-- EVIDENCE-CARD {} -->\nSource: {path}\n",
                &citation[..12]
            ));
            if contextual {
                let caption = match scope {
                    "binding-guide" => {
                        "BrassClaw binding/architecture guide. Keep target requirements, implementation observations and qualified support distinct."
                    }
                    "brassclaw-authoring" => {
                        "BrassClaw authoring/compatibility procedure. Keep host embedding, preload definitions, invocation bodies and Recipe metadata separate; use current supplied contracts."
                    }
                    "upstream-api" => {
                        "Upstream SDK/API documentation or template. Host embedding APIs are not guest functions or BrassClaw host callables; template directives are not complete rendered signatures."
                    }
                    _ => {
                        "Quoted reference data. Availability, compatibility, approval and permission require their own evidence."
                    }
                };
                context.push_str(&format!(
                    "Source context (compiler metadata, not original quotation): {caption}\n"
                ));
            }
            if path == "fixture/literal.md" {
                let escaped = LITERAL_YAML.replace("<|im_end|>", "\\u003c|im_end|\\u003e");
                context.push_str(&format!("````text\n{escaped}````"));
            } else if dedup && !progressive && path == "fixture/shared-a.md" {
                context.push_str(&format!(
                    "\nExact shared unit {digest} (complete original):\n```text\n{SHARED}```\n"
                ));
            } else if dedup
                && (path == "fixture/shared-b.md"
                    || (!contextual && path.ends_with("docs/api/python/fixture.md")))
            {
                context.push_str(&format!("\nExact shared unit {digest}: see fixture/shared-a.md lines 1-1. Original here: lines 1-1.\n"));
            } else {
                context.push_str(&format!("```text\n{original}```"));
            }
            context.push_str("\n<!-- END-EVIDENCE-CARD -->\n");
            map_sources.push(json!({"source":path,"document_sha256":digest,"parts":[{"unit_id":digest,"line_start":1,"line_end":line_end}]}));
            if contextual {
                map_sources.last_mut().unwrap()["context"] = json!(scope);
            }
            units
                .entry((
                    if contextual {
                        scope.to_owned()
                    } else {
                        String::new()
                    },
                    digest,
                ))
                .or_default()
                .push(json!({"source":path,"line_start":1,"line_end":line_end}));
        }
        if trailing {
            context.push('\n');
            context.push_str(&inventory);
        }
        let render_map = dedup.then(|| json!({"format":if contextual {"brassclaw-prefix-render-map/2"} else {"brassclaw-prefix-render-map/1"},"sources":map_sources,
            "units":units.into_iter().map(|((scope,digest),origins)| { let mut unit=json!({"unit_id":digest,"canonical":origins[0],"origins":origins}); if contextual {unit["context"]=json!(scope);} unit }).collect::<Vec<_>>()}));
        Ok(Self {
            snapshot,
            context,
            render_map,
        })
    }
    /// Namespace synthetic artifacts by the closed qualification identity. This
    /// is validation data only; it never changes the production prefix design.
    pub(super) fn scoped(profile: &str, view: &str, scope: &str) -> Result<Self, RebornBuildError> {
        if scope.is_empty() || scope.len() > 128 || scope.chars().any(char::is_control) {
            return Err(invalid("invalid compiler qualification scope"));
        }
        let mut fixture = Self::new(profile, view)?;
        fixture.snapshot["release"] = json!(format!("closed-compiler-fixture/1:{scope}"));
        Ok(fixture)
    }
    pub(super) fn snapshot(&self) -> &Value {
        &self.snapshot
    }
    pub(super) fn verify(&self, bytes: &str) -> Result<(), RebornBuildError> {
        let profile = self.snapshot["profile"]
            .as_str()
            .ok_or_else(|| invalid("fixture profile missing"))?;
        prefix_generations::validate_package(profile, bytes).map_err(invalid)?;
        let package: Value = serde_json::from_str(bytes).map_err(invalid)?;
        let read = |name: &str| -> Result<Vec<u8>, RebornBuildError> {
            STANDARD
                .decode(
                    package["artifacts"][name]["data"]
                        .as_str()
                        .ok_or_else(|| invalid("fixture original missing"))?,
                )
                .map_err(invalid)
        };
        if read("context.md")? != self.context.as_bytes()
            || read("source_snapshot.json")?
                != prefix_generations::canonical(&self.snapshot)
                    .map_err(invalid)?
                    .as_bytes()
        {
            return Err(invalid("compiler fixture rendering/input differs"));
        }
        for source in self.snapshot["sources"]
            .as_array()
            .ok_or_else(|| invalid("fixture sources missing"))?
        {
            let original = source["text"]
                .as_str()
                .ok_or_else(|| invalid("fixture text missing"))?;
            if read(&format!("raw/{}", sha(original.as_bytes())))? != original.as_bytes() {
                return Err(invalid("compiler fixture original differs"));
            }
        }
        for (name, original) in self.snapshot["procedures"]
            .as_object()
            .ok_or_else(|| invalid("fixture procedures missing"))?
        {
            if read(name)?
                != original
                    .as_str()
                    .ok_or_else(|| invalid("fixture procedure missing"))?
                    .as_bytes()
            {
                return Err(invalid("compiler fixture procedure differs"));
            }
        }
        match &self.render_map {
            Some(expected)
                if serde_json::from_slice::<Value>(&read("render_map.json")?)
                    .map_err(invalid)?
                    != *expected =>
            {
                return Err(invalid("compiler fixture origin map differs"));
            }
            None if package["artifacts"].get("render_map.json").is_some() => {
                return Err(invalid("unexpected compiler fixture origin map"));
            }
            _ => {}
        }
        let manifest: Value = serde_json::from_slice(&read("manifest.json")?).map_err(invalid)?;
        if manifest["activation_eligible"] != false
            || manifest["geometry"] != json!({"status":"unmeasured"})
            || manifest["model_quality"] != "unmeasured"
            || manifest["cache_reuse"] != "unverified"
        {
            return Err(invalid("compiler fixture overclaims qualification"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    fn compile(snapshot: &Value) -> String {
        let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut child = Command::new("python3").args(["-c","import json,sys; from test_prefix_compile_logic import run_stages; print(run_stages(json.load(sys.stdin))[0])"])
            .env("PYTHONPATH",checkout.join("scripts/prefix")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(serde_json::to_string(snapshot).unwrap().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap()
            .trim_end_matches('\n')
            .to_owned()
    }
    #[test]
    fn contextual_map_rejects_resigned_missing_scopes_and_cross_context_origins() {
        let fixture =
            CompilerFixture::new("base-prompt", "literal-reference/v5-contextual-sharing").unwrap();
        let original: Value = serde_json::from_str(&compile(fixture.snapshot())).unwrap();
        let read = |package: &Value, name: &str| -> Value {
            serde_json::from_slice(
                &STANDARD
                    .decode(package["artifacts"][name]["data"].as_str().unwrap())
                    .unwrap(),
            )
            .unwrap()
        };
        let resign = |package: &mut Value, name: &str, value: &Value| {
            let bytes = prefix_generations::canonical(value).unwrap();
            package["artifacts"][name]["data"] = json!(STANDARD.encode(bytes.as_bytes()));
            let mut manifest = read(package, "manifest.json");
            manifest["files"][name] = json!(sha(bytes.as_bytes()));
            let identity = json!({"profile":manifest["profile"],"compiler":manifest["compiler"],"files":manifest["files"],"geometry":manifest["geometry"]});
            manifest["generation"] = json!(sha(prefix_generations::canonical(&identity)
                .unwrap()
                .as_bytes()));
            package["artifacts"]["manifest.json"]["data"] =
                json!(STANDARD.encode(prefix_generations::canonical(&manifest).unwrap()));
        };
        prefix_generations::validate_package("base-prompt", &original.to_string()).unwrap();
        let mut package = original.clone();
        let mut design = read(&package, "profile_design.json");
        design["source_contexts"]
            .as_object_mut()
            .unwrap()
            .remove("skills.md");
        resign(&mut package, "profile_design.json", &design);
        assert!(prefix_generations::validate_package("base-prompt", &package.to_string()).is_err());
        let mut package = original.clone();
        let mut mapping = read(&package, "render_map.json");
        mapping["sources"][0]["context"] = json!("reference");
        resign(&mut package, "render_map.json", &mapping);
        assert!(prefix_generations::validate_package("base-prompt", &package.to_string()).is_err());
        let mut package = original.clone();
        let mut mapping = read(&package, "render_map.json");
        let unit = mapping["units"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|unit| {
                unit["context"] == "upstream-api" && unit["unit_id"] == sha(SHARED.as_bytes())
            })
            .unwrap();
        unit["canonical"]["source"] = json!("fixture/shared-a.md");
        unit["origins"][0]["source"] = json!("fixture/shared-a.md");
        resign(&mut package, "render_map.json", &mapping);
        assert!(prefix_generations::validate_package("base-prompt", &package.to_string()).is_err());
    }

    #[test]
    fn closed_compiler_fixture_has_independent_rendering_and_original_oracles() {
        for profile in ["base-prompt", "prefix-maker"] {
            for view in [
                "literal-reference/v1",
                "literal-reference/v2-deduplicated",
                "literal-reference/v3-trailing-inventory",
                "literal-reference/v4-stable-sharing",
                "literal-reference/v5-contextual-sharing",
            ] {
                let fixture = CompilerFixture::new(profile, view).unwrap();
                fixture.verify(&compile(fixture.snapshot())).unwrap();
                let mut changed = fixture.snapshot().clone();
                changed["header"] = json!("# Different accepted frame\n\n");
                assert!(
                    fixture.verify(&compile(&changed)).is_err(),
                    "A coherently re-signed different rendering is not this fixture"
                );
            }
        }
        let sempai =
            CompilerFixture::new("sempai", "literal-reference/v5-contextual-sharing").unwrap();
        sempai.verify(&compile(sempai.snapshot())).unwrap();
        assert!(CompilerFixture::new("sempai", "literal-reference/v4-stable-sharing").is_err());
        let first =
            CompilerFixture::scoped("base-prompt", "literal-reference/v1", "qualification-a")
                .unwrap();
        let next =
            CompilerFixture::scoped("base-prompt", "literal-reference/v1", "qualification-b")
                .unwrap();
        let first_bytes = compile(first.snapshot());
        let next_bytes = compile(next.snapshot());
        first.verify(&first_bytes).unwrap();
        next.verify(&next_bytes).unwrap();
        assert_ne!(
            first_bytes, next_bytes,
            "different qualification identities cannot collide with an old profile revision"
        );
        assert_eq!(
            first.context, next.context,
            "qualification metadata never changes the synthetic literal view"
        );
        assert!(first.verify(&next_bytes).is_err());
        assert!(
            CompilerFixture::scoped("base-prompt", "literal-reference/v1", "bad\nscope").is_err()
        );
        assert!(CompilerFixture::new("other", "literal-reference/v1").is_err());
    }
}
