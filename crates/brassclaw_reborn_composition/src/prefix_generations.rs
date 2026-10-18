//! Immutable storage primitive; no compiler execution, approval or activation.
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    ImportPrefixGenerationRequest, InterceptorConfigServiceError as Error, PrefixGenerationDetail,
    PrefixGenerationSummary,
};
use brassclaw_skills::value_contract::{ContractLimits, strict_json};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, sync::Arc};

const LIMITS: ContractLimits = ContractLimits {
    max_depth: 64,
    max_nodes: 2_000_000,
    max_bytes: 64 * 1024 * 1024,
};
fn bad() -> Error {
    Error::InvalidRequest {
        reason: "invalid or incomplete prefix generation package".into(),
    }
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn text(v: &Value) -> Result<&str, Error> {
    v.as_str().ok_or_else(bad)
}
fn hash(v: &Value) -> Result<&str, Error> {
    let s = text(v)?;
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad());
    }
    Ok(s)
}
pub(crate) fn canonical(value: &Value) -> Result<String, Error> {
    Ok(match value {
        Value::Object(map) => {
            let mut fields = map.iter().collect::<Vec<_>>();
            fields.sort_by(|a, b| a.0.cmp(b.0));
            format!(
                "{{{}}}",
                fields
                    .into_iter()
                    .map(|(k, v)| Ok(format!(
                        "{}:{}",
                        serde_json::to_string(k).map_err(|_| bad())?,
                        canonical(v)?
                    )))
                    .collect::<Result<Vec<_>, Error>>()?
                    .join(",")
            )
        }
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        Value::Number(n) if !n.is_i64() && !n.is_u64() => return Err(bad()),
        other => serde_json::to_string(other).map_err(|_| bad())?,
    })
}
// Match str.splitlines(keepends=True), including CRLF and Unicode separators,
// so source ranges never normalize or discard original bytes.
pub(crate) fn original_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut characters = text.char_indices().peekable();
    while let Some((offset, character)) = characters.next() {
        if matches!(
            character,
            '\n' | '\r'
                | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        ) {
            let mut end = offset + character.len_utf8();
            if character == '\r' && characters.peek().is_some_and(|(_, next)| *next == '\n') {
                characters.next();
                end += 1;
            }
            lines.push(&text[start..end]);
            start = end;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}
/// Check explicit archive links and original input metadata. These are byte/
/// relationship invariants, never approval or proof of which compiler executed.
fn validate_archived_source_input(
    profile: &str,
    sources: &Value,
    artifacts: &std::collections::BTreeMap<&str, Vec<u8>>,
) -> Result<(), Error> {
    let input_name = text(&sources["input_artifact"])?;
    let read = |name: &str| {
        strict_json(
            std::str::from_utf8(artifacts.get(name).ok_or_else(bad)?).map_err(|_| bad())?,
            LIMITS,
        )
        .map_err(|_| bad())
    };
    if !matches!(input_name, "input_sources.json" | "source_snapshot.json") {
        return Err(bad());
    }
    let input = read(input_name)?;
    let snapshot = input_name == "source_snapshot.json";
    if input.as_object().ok_or_else(bad)?.len() != if snapshot { 7 } else { 3 }
        || input["format"]
            != if snapshot {
                "brassclaw-prefix-source-snapshot/1"
            } else {
                "brassclaw-prefix-source-package/1"
            }
        || input["release"] != sources["release"]
        || text(&input["release"])?.trim().is_empty()
    {
        return Err(bad());
    }
    let design = read("profile_design.json")?;
    if design["format"] != "brassclaw-prefix-compiler-design/1"
        || design["profile"] != profile
        || text(&design["header"])?.trim().is_empty()
        || design["source_selection"] != "complete-mandatory"
        || !matches!(
            design["evidence_view"].as_str(),
            Some("literal-reference/v1" | "literal-reference/v2-deduplicated")
        )
    {
        return Err(bad());
    }
    if snapshot {
        if input["profile"] != profile
            || input["header"] != design["header"]
            || input["evidence_view"] != design["evidence_view"]
        {
            return Err(bad());
        }
        let procedures = input["procedures"].as_object().ok_or_else(bad)?;
        if procedures.len() != 3 {
            return Err(bad());
        }
        for name in [
            "compiler/entry.py",
            "compiler/prefix_units.py",
            "compiler/prefix_render.py",
        ] {
            let original = text(procedures.get(name).ok_or_else(bad)?)?;
            if original.is_empty()
                || artifacts
                    .get(name)
                    .is_none_or(|data| data.as_slice() != original.as_bytes())
            {
                return Err(bad());
            }
        }
    }
    let rows = sources["sources"].as_array().ok_or_else(bad)?;
    let originals = input["sources"].as_array().ok_or_else(bad)?;
    if originals.len() != rows.len() {
        return Err(bad());
    }
    let by_name = rows
        .iter()
        .map(|row| Ok((text(&row["path"])?, row)))
        .collect::<Result<std::collections::BTreeMap<_, _>, Error>>()?;
    let mut seen = BTreeSet::new();
    for original in originals {
        if original.as_object().ok_or_else(bad)?.len() != if snapshot { 5 } else { 6 } {
            return Err(bad());
        }
        let name = text(&original["path"])?;
        if !seen.insert(name) {
            return Err(bad());
        }
        let archived = by_name.get(name).ok_or_else(bad)?;
        for key in ["path", "kind", "disposition", "reason"] {
            if original[key] != archived[key] {
                return Err(bad());
            }
        }
        if snapshot {
            let body = text(&original["text"])?;
            if artifacts
                .get(text(&archived["artifact"])?)
                .is_none_or(|data| data.as_slice() != body.as_bytes())
            {
                return Err(bad());
            }
        } else if original["sha256"] != archived["sha256"]
            || text(&original["artifact"])?.is_empty()
        {
            return Err(bad());
        }
    }
    Ok(())
}

/// Strict, self-contained package integrity, not a source/teacher/cache verdict.
pub(crate) fn validate_package(profile: &str, bytes: &str) -> Result<(), Error> {
    let package = strict_json(bytes, LIMITS).map_err(|_| bad())?;
    let object = package.as_object().ok_or_else(bad)?;
    if object.len() != 3
        || package["format"] != "brassclaw-prefix-generation-package/1"
        || package["profile"] != profile
        || !matches!(profile, "base-prompt" | "prefix-maker")
    {
        return Err(bad());
    }
    let objects = package["artifacts"].as_object().ok_or_else(bad)?;
    let mut artifacts = std::collections::BTreeMap::new();
    let mut size = 0usize;
    for (name, artifact) in objects {
        if name.is_empty()
            || name.starts_with('/')
            || name.contains('\\')
            || name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
        {
            return Err(bad());
        }
        let a = artifact.as_object().ok_or_else(bad)?;
        if a.len() != 2 || artifact["encoding"] != "base64" {
            return Err(bad());
        }
        let data = STANDARD
            .decode(text(&artifact["data"])?)
            .map_err(|_| bad())?;
        size = size
            .checked_add(data.len())
            .filter(|n| *n <= LIMITS.max_bytes)
            .ok_or_else(bad)?;
        artifacts.insert(name.as_str(), data);
    }
    let read = |name: &str| -> Result<Value, Error> {
        strict_json(
            std::str::from_utf8(artifacts.get(name).ok_or_else(bad)?).map_err(|_| bad())?,
            LIMITS,
        )
        .map_err(|_| bad())
    };
    let manifest = read("manifest.json")?;
    if manifest["profile"] != profile
        || manifest["activation_eligible"] != false
        || manifest["model_quality"] != "unmeasured"
        || manifest["cache_reuse"] != "unverified"
    {
        return Err(bad());
    }
    let files = manifest["files"].as_object().ok_or_else(bad)?;
    let expected = files
        .keys()
        .map(String::as_str)
        .chain(std::iter::once("manifest.json"))
        .collect::<BTreeSet<_>>();
    if expected != artifacts.keys().copied().collect() {
        return Err(bad());
    }
    for (name, checksum) in files {
        if sha(artifacts.get(name.as_str()).ok_or_else(bad)?) != hash(checksum)? {
            return Err(bad());
        }
    }
    let identity = json!({"profile":manifest["profile"],"compiler":manifest["compiler"],"files":manifest["files"],"geometry":manifest["geometry"]});
    if sha(canonical(&identity)?.as_bytes()) != hash(&manifest["generation"])? {
        return Err(bad());
    }
    hash(&manifest["compiler"]["entry_sha256"])?;
    hash(&manifest["compiler"]["shared_evidence_sha256"])?;
    if manifest["compiler"]["format"] == "installation-reference-baseline/1"
        && (profile != "base-prompt"
            || manifest["origin"]
                != json!({"kind":"installation-qualification-baseline","coverage":"documentation-only"})
            || manifest["compiler"]["offline_compiler_executed"] != false
            || manifest["compiler"]["executed"]
                != json!(["installation-reference-baseline/1", "literal-reference/v1"])
            || manifest["geometry"] != json!({"status":"unmeasured"})
            || files.get("assembly.rs") != Some(&manifest["compiler"]["entry_sha256"])
            || files.get("assembly.rs") != Some(&manifest["compiler"]["shared_evidence_sha256"])
            || files.get("renderer.py") != Some(&manifest["compiler"]["renderer_sha256"]))
    {
        return Err(bad());
    }
    if manifest["compiler"]["format"] == "brassclaw-prefix-compiler-provenance/2" {
        for (file, property) in [
            ("compiler/entry.py", "entry_sha256"),
            ("compiler/prefix_evidence.py", "shared_evidence_sha256"),
            ("compiler/prefix_render.py", "renderer_sha256"),
        ] {
            if files.get(file) != Some(&manifest["compiler"][property]) {
                return Err(bad());
            }
        }
        if let Some(units) = manifest["compiler"].get("unit_segmenter_sha256") {
            hash(units)?;
            if files.get("compiler/prefix_units.py") != Some(units) {
                return Err(bad());
            }
        } else if files.contains_key("compiler/prefix_units.py") {
            return Err(bad());
        }
        if let Some(adapter) = manifest["compiler"].get("template_adapter_sha256") {
            hash(adapter)?;
            if files.get("compiler/prefix_template.py") != Some(adapter) {
                return Err(bad());
            }
        } else if files.contains_key("compiler/prefix_template.py") {
            return Err(bad());
        }
        if let Some(worker) = manifest["compiler"].get("worker_sha256") {
            hash(worker)?;
            if files.get("compiler/worker.py") != Some(worker) {
                return Err(bad());
            }
        } else if files.contains_key("compiler/worker.py") {
            return Err(bad());
        }
        if manifest["compiler"]["execution_attestation"] != "unqualified-development-process"
            || (profile == "prefix-maker"
                && files.get("compiler/profile_adapter.py")
                    != Some(&manifest["compiler"]["profile_adapter_sha256"]))
        {
            return Err(bad());
        }
    }
    if manifest["compiler"]["format"] == "brassclaw-monty-prefix-compiler/1" {
        for (file, key) in [
            ("compiler/entry.py", "entry_sha256"),
            ("compiler/prefix_units.py", "shared_evidence_sha256"),
            ("compiler/prefix_units.py", "unit_segmenter_sha256"),
            ("compiler/prefix_render.py", "renderer_sha256"),
        ] {
            hash(&manifest["compiler"][key])?;
            if files.get(file) != Some(&manifest["compiler"][key]) {
                return Err(bad());
            }
        }
        if manifest["compiler"]["execution_attestation"] != "unqualified-development-process"
            || (profile == "prefix-maker"
                && manifest["compiler"]["profile_adapter_sha256"]
                    != manifest["compiler"]["entry_sha256"])
        {
            return Err(bad());
        }
    }
    let geometry = &manifest["geometry"];
    match text(&geometry["status"])? {
        "unmeasured" => {}
        "local-tokenizer-validated" => {
            let rendered = geometry["rendered_probe_tokens"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(bad)?;
            let reserve = geometry["reserve_tokens"]
                .as_u64()
                .filter(|n| *n >= 8192)
                .ok_or_else(bad)?;
            let maximum = geometry["max_model_len"].as_u64().ok_or_else(bad)?;
            if rendered.checked_add(reserve).is_none_or(|n| n > maximum) {
                return Err(bad());
            }
            if !files.contains_key("server_chat_template.jinja") {
                return Err(bad());
            }
        }
        _ => return Err(bad()),
    }
    let sources = read("sources.json")?;
    let explicit_archive = sources["format"] == "brassclaw-prefix-generation-sources/1";
    if !explicit_archive && sources["format"] != "brassclaw-prefix-source-package/1" {
        return Err(bad());
    }
    let mut included = std::collections::BTreeMap::new();
    let mut names = BTreeSet::new();
    for source in sources["sources"].as_array().ok_or_else(bad)? {
        let name = text(&source["path"])?;
        if !names.insert(name) {
            return Err(bad());
        }
        let checksum = hash(&source["sha256"])?;
        let raw = format!("raw/{checksum}");
        if explicit_archive
            && (source.as_object().ok_or_else(bad)?.len() != 6 || source["artifact"] != raw)
        {
            return Err(bad());
        }
        let data = artifacts.get(raw.as_str()).ok_or_else(bad)?;
        if explicit_archive
            && (name.is_empty()
                || name.starts_with('/')
                || name.contains('\\')
                || name
                    .split('/')
                    .any(|part| part.is_empty() || part.starts_with('.'))
                || name.split('/').next_back() == Some("LOCAL_TEST_ENV.md")
                || name
                    .chars()
                    .any(|ch| ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}'))
                || !matches!(source["kind"].as_str(), Some("document" | "component"))
                || text(&source["reason"])?.trim().is_empty()
                || std::str::from_utf8(data)
                    .map_err(|_| bad())?
                    .trim()
                    .is_empty())
        {
            return Err(bad());
        }
        if sha(data) != checksum {
            return Err(bad());
        }
        match text(&source["disposition"])? {
            "included" => {
                included.insert(name, (checksum, data));
            }
            "historical" | "development-only" | "superseded" => {}
            _ => return Err(bad()),
        }
    }
    if explicit_archive {
        if sources.as_object().ok_or_else(bad)?.len() != 4
            || manifest["source_count"].as_u64() != Some(names.len() as u64)
        {
            return Err(bad());
        }
        validate_archived_source_input(profile, &sources, &artifacts)?;
    }
    for guide in [
        "skills.md",
        "recipe.md",
        "tools.md",
        "toolskills.md",
        "CLAUDE.md",
        "AGENTS.md",
        "simplified_v3.md",
    ] {
        if !included.contains_key(guide)
            || (explicit_archive
                && sources["sources"]
                    .as_array()
                    .ok_or_else(bad)?
                    .iter()
                    .find(|row| row["path"] == guide)
                    .is_none_or(|row| row["kind"] != "document"))
        {
            return Err(bad());
        }
    }
    if profile == "prefix-maker" {
        if !included.contains_key("scripts/prefix/prefix-maker-reference.md") {
            return Err(bad());
        }
        hash(&manifest["compiler"]["profile_adapter_sha256"])?;
        let design = read("profile_design.json")?;
        if design["format"] != "brassclaw-prefix-compiler-design/1"
            || design["profile"] != profile
            || !matches!(
                design["evidence_view"].as_str(),
                Some("literal-reference/v1" | "literal-reference/v2-deduplicated")
            )
            || design["source_selection"] != "complete-mandatory"
            || text(&design["header"])?.trim().is_empty()
        {
            return Err(bad());
        }
        let requirements = design["required_sources"].as_array().ok_or_else(bad)?;
        let mut coverage = BTreeSet::new();
        for source in requirements {
            let name = text(source)?;
            if !coverage.insert(name) || !included.contains_key(name) {
                return Err(bad());
            }
        }
        if !coverage.contains("scripts/prefix/prefix-maker-reference.md") {
            return Err(bad());
        }
        if coverage.len() != 8
            || coverage.iter().any(|name| {
                !matches!(
                    *name,
                    "skills.md"
                        | "recipe.md"
                        | "tools.md"
                        | "toolskills.md"
                        | "CLAUDE.md"
                        | "AGENTS.md"
                        | "simplified_v3.md"
                        | "scripts/prefix/prefix-maker-reference.md"
                )
            })
        {
            return Err(bad());
        }
    }
    let evidence = read("evidence.json")?;
    let mut evidenced = BTreeSet::new();
    let mut compact_ids = BTreeSet::new();
    let mut mapped_sources = Vec::new();
    let mut mapped_units = std::collections::BTreeMap::<String, Vec<Value>>::new();
    for card in evidence.as_array().ok_or_else(bad)? {
        let name = text(&card["path"])?;
        let (checksum, data) = included.get(name).ok_or_else(bad)?;
        if !evidenced.insert(name)
            || hash(&card["document_sha256"])? != *checksum
            || text(&card["excerpt"])?.as_bytes() != data.as_slice()
            || hash(&card["excerpt_sha256"])? != *checksum
        {
            return Err(bad());
        }
        let original = std::str::from_utf8(data).map_err(|_| bad())?;
        let lines = original_lines(original);
        if explicit_archive {
            let expected = sha(canonical(&json!([name, checksum, 1, lines.len()]))?.as_bytes());
            let id = text(&card["id"])?;
            if id != expected
                || card["line_start"] != 1
                || card["line_end"].as_u64() != Some(lines.len() as u64)
                || card["disposition"] != "included"
                || !compact_ids.insert(&id[..12])
            {
                return Err(bad());
            }
        }
        let mut start = 1usize;
        let mut parts = Vec::new();
        for section in card["sections"].as_array().ok_or_else(bad)? {
            let first = section["line_start"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(bad)?;
            let last = section["line_end"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(bad)?;
            if first != start || last < first || last > lines.len() {
                return Err(bad());
            }
            let unit = sha(lines[first - 1..last].concat().as_bytes());
            if hash(&section["excerpt_sha256"])? != unit {
                return Err(bad());
            }
            parts.push(json!({"unit_id":unit,"line_start":first,"line_end":last}));
            mapped_units
                .entry(unit)
                .or_default()
                .push(json!({"source":name,"line_start":first,"line_end":last}));
            start = last.checked_add(1).ok_or_else(bad)?;
        }
        if start != lines.len().checked_add(1).ok_or_else(bad)? {
            return Err(bad());
        }
        mapped_sources.push(json!({"source":name,"document_sha256":checksum,"parts":parts}));
    }
    if (explicit_archive && manifest["evidence_count"].as_u64() != Some(evidenced.len() as u64))
        || evidenced != included.keys().copied().collect()
        || !files.contains_key("context.md")
    {
        return Err(bad());
    }
    if files.contains_key("render_map.json") {
        let design = read("profile_design.json")?;
        if design["format"] != "brassclaw-prefix-compiler-design/1"
            || design["profile"] != profile
            || design["evidence_view"] != "literal-reference/v2-deduplicated"
            || design["source_selection"] != "complete-mandatory"
        {
            return Err(bad());
        }
        hash(&manifest["compiler"]["renderer_sha256"])?;
        let units: Vec<_> = mapped_units
            .into_iter()
            .map(|(id, origins)| json!({"unit_id":id,"canonical":origins[0],"origins":origins}))
            .collect();
        let expected = json!({"format":"brassclaw-prefix-render-map/1","sources":mapped_sources,"units":units});
        if read("render_map.json")? != expected {
            return Err(bad());
        }
    } else if files.contains_key("profile_design.json")
        && read("profile_design.json")?["evidence_view"] == "literal-reference/v2-deduplicated"
    {
        return Err(bad());
    }
    Ok(())
}
pub(crate) struct PgPrefixGenerations {
    pool: Arc<PgPool>,
}
impl PgPrefixGenerations {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    pub(crate) async fn import(
        &self,
        profile: &str,
        request: ImportPrefixGenerationRequest,
    ) -> Result<PrefixGenerationDetail, Error> {
        validate_package(profile, &request.generation_bytes)?;
        let generation = sha(request.generation_bytes.as_bytes());
        let revision = i64::try_from(request.expected_profile_revision)
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(bad)?;
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client.transaction().await.map_err(|_| Error::Unavailable)?;
        // Exact retry after an unknown commit remains a read of immutable
        // evidence even if mutable profile metadata changed in the meantime.
        if let Some(existing) = tx.query_opt("SELECT generation_bytes,profile_revision FROM reborn_prefix_generations WHERE profile_name=$1 AND generation_id=$2", &[&profile,&generation]).await.map_err(|_|Error::Unavailable)? {
            if existing.get::<_,String>(0)!=request.generation_bytes || existing.get::<_,i64>(1)!=revision {return Err(Error::Conflict);}
            tx.commit().await.map_err(|_|Error::Unavailable)?;
            return self.get(profile,&generation).await;
        }
        let row = tx
            .query_opt(
                "SELECT revision FROM reborn_prefix_profiles WHERE name=$1 FOR SHARE",
                &[&profile],
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .ok_or_else(|| Error::PrefixNotFound {
                name: profile.into(),
            })?;
        if row.get::<_, i64>(0) != revision {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO reborn_prefix_generations(profile_name,generation_id,profile_revision,generation_kind,generation_bytes) VALUES($1,$2,$3,'compiler-package',$4) ON CONFLICT DO NOTHING",&[&profile,&generation,&revision,&request.generation_bytes]).await.map_err(|_|Error::Unavailable)?;
        let row=tx.query_one("SELECT generation_bytes,profile_revision FROM reborn_prefix_generations WHERE profile_name=$1 AND generation_id=$2",&[&profile,&generation]).await.map_err(|_|Error::Unavailable)?;
        if row.get::<_, String>(0) != request.generation_bytes {
            return Err(Error::Conflict);
        }
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        self.get(profile, &generation).await
    }
    pub(crate) async fn list(&self, profile: &str) -> Result<Vec<PrefixGenerationSummary>, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        if client
            .query_opt(
                "SELECT name FROM reborn_prefix_profiles WHERE name=$1",
                &[&profile],
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .is_none()
        {
            return Err(Error::PrefixNotFound {
                name: profile.into(),
            });
        }
        let rows=client.query("SELECT generation_id,profile_revision,generation_kind,created_at FROM reborn_prefix_generations WHERE profile_name=$1 ORDER BY created_at,generation_id",&[&profile]).await.map_err(|_|Error::Unavailable)?;
        rows.into_iter().map(|r| summary(profile, &r)).collect()
    }
    pub(crate) async fn get(
        &self,
        profile: &str,
        generation: &str,
    ) -> Result<PrefixGenerationDetail, Error> {
        hash(&Value::String(generation.into()))?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row=client.query_opt("SELECT generation_id,profile_revision,generation_kind,created_at,generation_bytes FROM reborn_prefix_generations WHERE profile_name=$1 AND generation_id=$2",&[&profile,&generation]).await.map_err(|_|Error::Unavailable)?.ok_or(Error::GenerationNotFound)?;
        let bytes: String = row.get(4);
        if sha(bytes.as_bytes()) != generation {
            return Err(Error::Unavailable);
        }
        let summary = summary(profile, &row)?;
        if summary.kind == "compiler-package" {
            validate_package(profile, &bytes)?;
        }
        Ok(PrefixGenerationDetail {
            summary,
            generation_bytes: bytes,
        })
    }
}
fn summary(profile: &str, row: &tokio_postgres::Row) -> Result<PrefixGenerationSummary, Error> {
    let kind: String = row.get(2);
    Ok(PrefixGenerationSummary {
        profile: profile.into(),
        generation: row.get(0),
        profile_revision: u64::try_from(row.get::<_, i64>(1)).map_err(|_| Error::Unavailable)?,
        qualification: if kind == "legacy-bundle" {
            "legacy-unverified"
        } else {
            "quarantined"
        }
        .into(),
        kind,
        created_at: row.get::<_, chrono::DateTime<chrono::Utc>>(3).to_rfc3339(),
    })
}
