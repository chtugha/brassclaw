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
        if !matches!(profile, "base-prompt" | "prefix-maker")
            || !matches!(
                view,
                "literal-reference/v1"
                    | "literal-reference/v2-deduplicated"
                    | "literal-reference/v3-trailing-inventory"
                    | "literal-reference/v4-stable-sharing"
            )
        {
            return Err(invalid("unknown closed compiler fixture"));
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
        ]);
        let snapshot = json!({"format":"brassclaw-prefix-source-snapshot/1", "profile":profile,
            "release":"closed-compiler-fixture/1", "header":HEADER,"evidence_view":view,"sources":sources,
            "procedures":{"compiler/entry.py":COMPILER_SOURCE,"compiler/prefix_render.py":REFERENCE_RENDER_SOURCE,"compiler/prefix_units.py":UNIT_SOURCE}});
        // Each current fixture has one complete unit. The YAML fence spans four
        // CRLF lines. Expectations are fixed fixture facts, independent of
        // the Python compiler/renderer; original and escaped view stay distinct.
        let dedup = view != "literal-reference/v1";
        let progressive = view == "literal-reference/v4-stable-sharing";
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
        let mut units = BTreeMap::<String, Vec<Value>>::new();
        for source in &sources {
            if source["disposition"] != "included" {
                continue;
            }
            let path = source["path"].as_str().unwrap();
            let original = source["text"].as_str().unwrap();
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
            if path == "fixture/literal.md" {
                let escaped = LITERAL_YAML.replace("<|im_end|>", "\\u003c|im_end|\\u003e");
                context.push_str(&format!("````text\n{escaped}````"));
            } else if dedup && !progressive && path == "fixture/shared-a.md" {
                context.push_str(&format!(
                    "\nExact shared unit {digest} (complete original):\n```text\n{SHARED}```\n"
                ));
            } else if dedup && path == "fixture/shared-b.md" {
                context.push_str(&format!("\nExact shared unit {digest}: see fixture/shared-a.md lines 1-1. Original here: lines 1-1.\n"));
            } else {
                context.push_str(&format!("```text\n{original}```"));
            }
            context.push_str("\n<!-- END-EVIDENCE-CARD -->\n");
            map_sources.push(json!({"source":path,"document_sha256":digest,"parts":[{"unit_id":digest,"line_start":1,"line_end":line_end}]}));
            units
                .entry(digest)
                .or_default()
                .push(json!({"source":path,"line_start":1,"line_end":line_end}));
        }
        if trailing {
            context.push('\n');
            context.push_str(&inventory);
        }
        let render_map = dedup.then(|| json!({"format":"brassclaw-prefix-render-map/1","sources":map_sources,
            "units":units.into_iter().map(|(digest,origins)| json!({"unit_id":digest,"canonical":origins[0],"origins":origins})).collect::<Vec<_>>()}));
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
    fn closed_compiler_fixture_has_independent_rendering_and_original_oracles() {
        for profile in ["base-prompt", "prefix-maker"] {
            for view in [
                "literal-reference/v1",
                "literal-reference/v2-deduplicated",
                "literal-reference/v3-trailing-inventory",
                "literal-reference/v4-stable-sharing",
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
