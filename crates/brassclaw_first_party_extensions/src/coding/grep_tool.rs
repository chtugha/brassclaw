//! Reborn first-party port of the v1 grep coding tool.

use std::{
    cmp::Reverse,
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

use brassclaw_filesystem::{FileStat, FileType, FilesystemOperation};
use brassclaw_host_api::{RuntimeDispatchErrorKind, ScopedPath, VirtualPath};
use glob::Pattern;
use regex::RegexBuilder;
use serde_json::{Value, json};

use super::{CodingCapabilityError, CodingCapabilityRequest};

use super::{
    input_error,
    inputs::{optional_usize, optional_usize_allow_zero, required_str},
    paths::{
        filesystem_error, is_excluded_name, is_sensitive_scoped_path, list_walk_entries,
        operation_allowed, resolve_optional_path, scoped_child_path, type_filter_matches,
        validate_relative_pattern, virtual_to_relative,
    },
    text::{decode_text, previous_char_boundary, reject_binary_probe},
    types::{GrepFileResult, GrepLine, ResolvedPath},
};

pub(super) async fn grep(
    request: &CodingCapabilityRequest<'_>,
) -> Result<Value, CodingCapabilityError> {
    let resolved = resolve_optional_path(request, FilesystemOperation::Stat)?;
    if !operation_allowed(&resolved.grant.permissions, FilesystemOperation::ReadFile) {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::FilesystemDenied,
        ));
    }
    let root_stat = request
        .filesystem
        .stat(&resolved.virtual_path)
        .await
        .map_err(filesystem_error)?;
    if root_stat.sensitive {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::FilesystemDenied,
        ));
    }
    if root_stat.file_type == FileType::Directory
        && !operation_allowed(&resolved.grant.permissions, FilesystemOperation::ListDir)
    {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::FilesystemDenied,
        ));
    }
    if !matches!(root_stat.file_type, FileType::File | FileType::Directory) {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::Resource,
        ));
    }
    let pattern = required_str(request.input, "pattern")?;
    let output_mode = request
        .input
        .get("output_mode")
        .and_then(Value::as_str)
        .unwrap_or("files_with_matches");
    if !matches!(output_mode, "content" | "files_with_matches" | "count") {
        return Err(input_error());
    }
    let glob_filter = request.input.get("glob").and_then(Value::as_str);
    if let Some(filter) = glob_filter {
        validate_relative_pattern(filter)?;
    }
    let glob_filter = glob_filter
        .map(Pattern::new)
        .transpose()
        .map_err(|_| input_error())?;
    let type_filter = request.input.get("type_filter").and_then(Value::as_str);
    let case_insensitive = request
        .input
        .get("case_insensitive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let multiline = request
        .input
        .get("multiline")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .multi_line(multiline)
        .dot_matches_new_line(multiline)
        .build()
        .map_err(|_| input_error())?;
    let context = optional_usize(request.input, "context")?;
    let before_context = if let Some(context) = context {
        context
    } else {
        optional_usize(request.input, "before_context")?.unwrap_or(0)
    };
    let after_context = if let Some(context) = context {
        context
    } else {
        optional_usize(request.input, "after_context")?.unwrap_or(0)
    };
    let head_limit = optional_usize_allow_zero(request.input, "head_limit")?;
    let offset = optional_usize(request.input, "offset")?.unwrap_or(0);
    let mut search_results = Vec::new();

    let walk_result = walk_files(
        request,
        &resolved,
        root_stat,
        |relative| {
            if let Some(filter) = &glob_filter
                && !filter.matches(relative)
            {
                return false;
            }
            if let Some(type_filter) = type_filter
                && !type_filter_matches(relative, type_filter)
            {
                return false;
            }
            true
        },
        |relative, bytes, modified| {
            if reject_binary_probe(bytes).is_err() {
                return Ok(true);
            }
            let Ok((content, _encoding, _line_ending)) = decode_text(bytes) else {
                return Ok(true);
            };
            if !regex.is_match(&content) {
                return Ok(true);
            }
            let (line_matches, count) = match output_mode {
                "files_with_matches" => (Vec::new(), 0),
                "count" => (Vec::new(), count_matches_only(&content, &regex, multiline)),
                _ => line_matches(&content, &regex, before_context, after_context, multiline),
            };
            search_results.push(GrepFileResult {
                relative: relative.to_string(),
                modified,
                count,
                lines: line_matches,
            });
            Ok(true)
        },
    )
    .await?;

    if output_mode == "files_with_matches" {
        search_results.sort_by(|left, right| {
            Reverse(left.modified.unwrap_or(UNIX_EPOCH))
                .cmp(&Reverse(right.modified.unwrap_or(UNIX_EPOCH)))
                .then_with(|| left.relative.cmp(&right.relative))
        });
    } else {
        search_results.sort_by(|left, right| left.relative.cmp(&right.relative));
    }
    Ok(build_grep_output(
        output_mode,
        search_results,
        offset,
        head_limit,
        before_context > 0 || after_context > 0,
        walk_result,
        &request.limits,
    ))
}

#[derive(Debug, Clone, Copy, Default)]
struct WalkFilesResult {
    scan_limit: Option<ScanLimit>,
    bytes_scanned: u64,
}

#[derive(Debug, Clone, Copy)]
enum ScanLimit {
    AggregateBytes,
    FileBytes { file_bytes: u64 },
}

async fn walk_files(
    request: &CodingCapabilityRequest<'_>,
    root: &ResolvedPath,
    root_stat: FileStat,
    mut include: impl FnMut(&str) -> bool,
    mut visit: impl FnMut(&str, &[u8], Option<SystemTime>) -> Result<bool, CodingCapabilityError>,
) -> Result<WalkFilesResult, CodingCapabilityError> {
    let mut walk_result = WalkFilesResult::default();
    if root_stat.file_type == FileType::File {
        let relative = root_file_relative(&root.scoped_path);
        if include(&relative)
            && !visit_file(
                request,
                &root.virtual_path,
                &relative,
                root_stat,
                &mut walk_result,
                &mut visit,
                false,
            )
            .await?
        {
            walk_result
                .scan_limit
                .get_or_insert(ScanLimit::AggregateBytes);
        }
        return Ok(walk_result);
    }

    let mut stack = vec![root.virtual_path.clone()];
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        let entries = list_walk_entries(request, &dir, visited).await?;
        for entry in entries {
            visited += 1;
            if visited > request.limits.max_coding_visited_entries as usize {
                return Err(CodingCapabilityError::new(
                    RuntimeDispatchErrorKind::Resource,
                ));
            }
            let relative = virtual_to_relative(&root.virtual_path, &entry.path)?;
            let scoped_path = scoped_child_path(&root.scoped_path, &relative);
            if is_sensitive_scoped_path(&scoped_path) {
                continue;
            }
            match entry.file_type {
                FileType::Directory => {
                    if !is_excluded_name(entry.name.as_str()) {
                        stack.push(entry.path);
                    }
                }
                FileType::File => {
                    if !include(&relative) {
                        continue;
                    }
                    // silent-ok: grep directory search skips entries that disappear or fail stat.
                    let Ok(stat) = request.filesystem.stat(&entry.path).await else {
                        tracing::debug!(
                            path = entry.path.as_str(),
                            "skipping grep file after stat failed"
                        );
                        continue;
                    };
                    if stat.sensitive {
                        continue;
                    }
                    if !visit_file(
                        request,
                        &entry.path,
                        &relative,
                        stat,
                        &mut walk_result,
                        &mut visit,
                        true,
                    )
                    .await?
                    {
                        walk_result
                            .scan_limit
                            .get_or_insert(ScanLimit::AggregateBytes);
                        return Ok(walk_result);
                    }
                }
                FileType::Symlink | FileType::Other => {}
            }
        }
    }
    Ok(walk_result)
}

async fn visit_file(
    request: &CodingCapabilityRequest<'_>,
    path: &VirtualPath,
    relative: &str,
    stat: FileStat,
    walk_result: &mut WalkFilesResult,
    visit: &mut impl FnMut(&str, &[u8], Option<SystemTime>) -> Result<bool, CodingCapabilityError>,
    skip_read_errors: bool,
) -> Result<bool, CodingCapabilityError> {
    if stat.len > request.limits.max_coding_read_bytes {
        tracing::debug!(
            path = path.as_str(),
            len = stat.len,
            max_read_size = request.limits.max_coding_read_bytes,
            "skipping oversized grep file"
        );
        if !skip_read_errors {
            walk_result.scan_limit = Some(ScanLimit::FileBytes {
                file_bytes: stat.len,
            });
            return Ok(false);
        }
        return Ok(true);
    }
    let next_total = walk_result.bytes_scanned.saturating_add(stat.len);
    if next_total > request.limits.max_coding_scan_bytes {
        tracing::debug!(
            path = path.as_str(),
            total_bytes = walk_result.bytes_scanned,
            next_file_bytes = stat.len,
            max_total_bytes = request.limits.max_coding_scan_bytes,
            "stopping grep after aggregate scan budget"
        );
        walk_result.scan_limit = Some(ScanLimit::AggregateBytes);
        return Ok(false);
    }
    let remaining = request.limits.max_coding_scan_bytes - walk_result.bytes_scanned;
    let read_bound = remaining.min(request.limits.max_coding_read_bytes);
    walk_result.bytes_scanned = next_total;
    let bytes = match request
        .filesystem
        .read_file_bounded(path, read_bound as usize)
        .await
    {
        Ok(Some(bytes)) => bytes,
        Ok(None) if read_bound < request.limits.max_coding_read_bytes => {
            walk_result.scan_limit = Some(ScanLimit::AggregateBytes);
            return Ok(false);
        }
        Ok(None) if skip_read_errors => return Ok(true),
        Ok(None) => {
            return Err(CodingCapabilityError::new(
                RuntimeDispatchErrorKind::Resource,
            ));
        }
        Err(_error) if skip_read_errors => {
            // silent-ok: grep directory search skips files that disappear or fail read.
            tracing::debug!(path = path.as_str(), "skipping grep file after read failed");
            return Ok(true);
        }
        Err(error) => return Err(filesystem_error(error)),
    };
    // The stream may have grown after stat. Account the larger observed size;
    // the bounded read has already kept it within the remaining scan budget.
    walk_result.bytes_scanned += (bytes.len() as u64).saturating_sub(stat.len);
    visit(relative, &bytes, stat.modified)
}

fn root_file_relative(path: &ScopedPath) -> String {
    path.as_str()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn build_grep_output(
    output_mode: &str,
    mut results: Vec<GrepFileResult>,
    offset: usize,
    head_limit: Option<usize>,
    had_context: bool,
    walk_result: WalkFilesResult,
    limits: &brassclaw_host_api::MontyExecutionLimits,
) -> Value {
    let effective_limit = match head_limit {
        Some(0) => usize::MAX,
        Some(value) => value,
        None if limits.default_coding_head_lines == 0 => usize::MAX,
        None => limits.default_coding_head_lines as usize,
    };
    match output_mode {
        "files_with_matches" => {
            let total = results.len();
            let files = results
                .into_iter()
                .skip(offset)
                .take(effective_limit)
                .map(|result| result.relative)
                .collect::<Vec<_>>();
            let mut output = json!({
                "files": files,
                "count": files.len(),
                "truncated": walk_result.scan_limit.is_some()
                    || total > offset.saturating_add(effective_limit)
            });
            add_scan_limit_metadata(&mut output, walk_result, limits);
            output
        }
        "count" => {
            let total_count = results.len();
            let page = results
                .drain(..)
                .skip(offset)
                .take(effective_limit)
                .collect::<Vec<_>>();
            let total = page.iter().map(|result| result.count).sum::<usize>();
            let mut output = json!({
                "counts": page.into_iter().map(|result| json!({
                    "file": result.relative,
                    "count": result.count
                })).collect::<Vec<_>>(),
                "total": total,
                "truncated": walk_result.scan_limit.is_some()
                    || total_count > offset.saturating_add(effective_limit)
            });
            add_scan_limit_metadata(&mut output, walk_result, limits);
            output
        }
        _ => {
            let mut lines = Vec::new();
            for result in results {
                for line in result.lines {
                    let separator = if line.is_match || !had_context {
                        ':'
                    } else {
                        '-'
                    };
                    lines.push(format!(
                        "{}{}{}{}{}",
                        result.relative, separator, line.number, separator, line.text
                    ));
                }
            }
            let raw_len = lines.iter().fold(0usize, |total, line| {
                total.saturating_add(line.len()).saturating_add(1)
            });
            let page = lines
                .iter()
                .skip(offset)
                .take(effective_limit)
                .cloned()
                .collect::<Vec<_>>();
            let mut content = page.join("\n");
            let mut truncated = walk_result.scan_limit.is_some()
                || raw_len > limits.max_coding_output_bytes as usize
                || lines.len() > offset.saturating_add(effective_limit);
            if content.len() > limits.max_coding_output_bytes as usize {
                content.truncate(previous_char_boundary(
                    &content,
                    limits.max_coding_output_bytes as usize,
                ));
                truncated = true;
            }
            let mut output = json!({ "content": content, "truncated": truncated });
            add_scan_limit_metadata(&mut output, walk_result, limits);
            output
        }
    }
}

fn add_scan_limit_metadata(
    output: &mut Value,
    walk_result: WalkFilesResult,
    limits: &brassclaw_host_api::MontyExecutionLimits,
) {
    match walk_result.scan_limit {
        Some(ScanLimit::AggregateBytes) => {
            output["limit_reason"] = json!("aggregate_scan_bytes");
            output["bytes_scanned"] = json!(walk_result.bytes_scanned);
            output["max_scan_bytes"] = json!(limits.max_coding_scan_bytes);
        }
        Some(ScanLimit::FileBytes { file_bytes }) => {
            output["limit_reason"] = json!("file_size_bytes");
            output["file_bytes"] = json!(file_bytes);
            output["max_file_bytes"] = json!(limits.max_coding_read_bytes);
        }
        None => {}
    }
}

fn count_matches_only(content: &str, regex: &regex::Regex, multiline: bool) -> usize {
    if multiline {
        regex.find_iter(content).count()
    } else {
        content.lines().filter(|line| regex.is_match(line)).count()
    }
}

fn line_matches(
    content: &str,
    regex: &regex::Regex,
    before_context: usize,
    after_context: usize,
    multiline: bool,
) -> (Vec<GrepLine>, usize) {
    if multiline {
        return multiline_matches(content, regex, before_context, after_context);
    }

    let lines = content.lines().collect::<Vec<_>>();
    let mut include = BTreeSet::new();
    let mut matched = BTreeSet::new();
    for (index, line) in lines.iter().enumerate() {
        if regex.is_match(line) {
            matched.insert(index);
            let start = index.saturating_sub(before_context);
            let end = index
                .saturating_add(after_context)
                .saturating_add(1)
                .min(lines.len());
            for item in start..end {
                include.insert(item);
            }
        }
    }
    let count = matched.len();
    let lines = include
        .into_iter()
        .map(|index| GrepLine {
            number: index + 1,
            text: lines[index].to_string(),
            is_match: matched.contains(&index) || (before_context == 0 && after_context == 0),
        })
        .collect::<Vec<_>>();
    (lines, count)
}

fn multiline_matches(
    content: &str,
    regex: &regex::Regex,
    before_context: usize,
    after_context: usize,
) -> (Vec<GrepLine>, usize) {
    let indexed = indexed_lines(content);
    let mut include = BTreeSet::new();
    let mut matched = BTreeSet::new();
    let mut count = 0usize;

    for item in regex.find_iter(content) {
        count += 1;
        for (index, (_text, start, end)) in indexed.iter().enumerate() {
            if spans_overlap(item.start(), item.end(), *start, *end) {
                matched.insert(index);
                let context_start = index.saturating_sub(before_context);
                let context_end = index
                    .saturating_add(after_context)
                    .saturating_add(1)
                    .min(indexed.len());
                for line_index in context_start..context_end {
                    include.insert(line_index);
                }
            }
        }
    }

    let lines = include
        .into_iter()
        .map(|index| GrepLine {
            number: index + 1,
            text: indexed[index].0.clone(),
            is_match: matched.contains(&index) || (before_context == 0 && after_context == 0),
        })
        .collect::<Vec<_>>();
    (lines, count)
}

fn indexed_lines(content: &str) -> Vec<(String, usize, usize)> {
    let mut output = Vec::new();
    let mut start = 0usize;
    for segment in content.split_inclusive('\n') {
        let end = start + segment.len();
        let text = segment.trim_end_matches('\n').to_string();
        output.push((text, start, end));
        start = end;
    }
    if output.is_empty() && content.is_empty() {
        output.push((String::new(), 0, 0));
    } else if start < content.len() {
        output.push((content[start..].to_string(), start, content.len()));
    }
    output
}

fn spans_overlap(match_start: usize, match_end: usize, line_start: usize, line_end: usize) -> bool {
    if match_start == match_end {
        return match_start >= line_start && match_start <= line_end;
    }
    match_start < line_end && match_end > line_start
}
