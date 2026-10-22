//! Reborn first-party port of the v1 file coding tools.
//!
//! The v1 `Tool`/`JobContext`/local-filesystem boundary is replaced here with
//! `CodingCapabilityRequest`, scoped mounts, and `RootFilesystem`.

use std::fmt::Write;

use brassclaw_filesystem::{CasExpectation, FileType, FilesystemOperation};
use brassclaw_host_api::RuntimeDispatchErrorKind;
use serde_json::{Value, json};

use super::{CodingCapabilityError, CodingCapabilityOutput, CodingCapabilityRequest};

use super::{
    diff_preview::{file_diff_preview, will_use_large_diff_path},
    input_error,
    inputs::{optional_usize, required_str},
    operation_error_with_summary,
    paths::{
        deny_sensitive_existing_path, filesystem_error, is_excluded_name, is_sensitive_scoped_path,
        is_workspace_path, list_walk_entries, operation_allowed, resolve_optional_path,
        resolve_required_path, scoped_child_path, stat_optional, validate_parent_directory,
        virtual_to_relative,
    },
    state::{SharedCodingEditLocks, read_scope_key},
    text::{
        PatchOutputLimits, count_matches, decode_text, encode_text, reject_binary_probe,
        replace_content,
    },
    types::{ListEntry, MatchMethod, ResolvedPath},
};

pub(super) async fn read_file(
    request: &CodingCapabilityRequest<'_>,
) -> Result<Value, CodingCapabilityError> {
    let fields = request.input.as_object().ok_or_else(input_error)?;
    // Native contained callers do not execute the metadata's JSON Schema.
    // Enforce its closed argument set here as well, before any path I/O.
    if fields
        .keys()
        .any(|key| !matches!(key.as_str(), "path" | "representation" | "offset" | "limit"))
    {
        return Err(input_error());
    }
    let raw_snapshot = match fields.get("representation") {
        None => false,
        Some(Value::String(value)) if value == "numbered_lines" => false,
        Some(Value::String(value)) if value == "raw_utf8_preserved" => true,
        _ => return Err(input_error()),
    };
    // A complete source capture cannot also select a line range, even an
    // explicit zero/default. Reject before any filesystem operation.
    if raw_snapshot
        && (request.input.get("offset").is_some() || request.input.get("limit").is_some())
    {
        return Err(input_error());
    }
    let resolved = resolve_required_path(request, "path", FilesystemOperation::ReadFile)?;
    let offset = optional_usize(request.input, "offset")?.unwrap_or(0);
    let limit = optional_usize(request.input, "limit")?;
    let has_explicit_range = offset > 0 || limit.is_some();
    let stat = request
        .filesystem
        .stat(&resolved.virtual_path)
        .await
        .map_err(|error| {
            filesystem_error_with_summary("read_file", resolved.scoped_path.as_str(), error)
        })?;
    if stat.sensitive {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::FilesystemDenied,
        ));
    }
    if stat.file_type != FileType::File || stat.len > request.limits.max_coding_read_bytes {
        return Err(CodingCapabilityError::with_safe_summary(
            RuntimeDispatchErrorKind::Resource,
            format!(
                "read_file failed for {}: target is not a readable file or exceeds the size limit",
                safe_summary_path(resolved.scoped_path.as_str())
            ),
        ));
    }

    let bytes = request
        .filesystem
        .read_file_bounded(
            &resolved.virtual_path,
            usize::try_from(request.limits.max_coding_read_bytes).unwrap_or(usize::MAX),
        )
        .await
        .map_err(|error| {
            filesystem_error_with_summary("read_file", resolved.scoped_path.as_str(), error)
        })?
        .ok_or_else(|| CodingCapabilityError::new(RuntimeDispatchErrorKind::Resource))?;
    reject_binary_probe(&bytes)?;
    if raw_snapshot {
        // Move the actual bounded read into UTF-8; no display formatting,
        // BOM stripping, replacement decoding or newline normalization.
        let content = String::from_utf8(bytes).map_err(|_| {
            operation_error_with_summary("read_file raw snapshot requires valid UTF-8")
        })?;
        let path = resolved.scoped_path.as_str().as_bytes();
        let mut identity = blake3::Hasher::new_derive_key("brassclaw.file-read-snapshot.1");
        identity.update(&(path.len() as u64).to_le_bytes());
        identity.update(path);
        identity.update(content.as_bytes());
        // Correlate immutable captured bytes with this host-assigned invocation.
        // This is not a mutable path/refetch handle or a filesystem checkpoint.
        // Retained execution durably keeps the actual output in its native journal.
        let source_ref = format!(
            "file-read-snapshot/1:{}:{}",
            request.scope.invocation_id,
            identity.finalize().to_hex(),
        );
        return Ok(json!({
            "source_ref": source_ref,
            "content": content,
            "complete": true,
            "representation": "raw_utf8_preserved",
        }));
    }
    let (content, _encoding, _line_ending) = decode_text(&bytes)?;
    let start_line = offset.saturating_sub(1);
    let default_window = !has_explicit_range && request.limits.default_coding_read_lines > 0;
    let window = limit.unwrap_or(if default_window {
        request.limits.default_coding_read_lines as usize
    } else {
        usize::MAX
    });
    let mut total_lines = 0;
    let mut lines_shown = 0;
    let mut selected = String::new();
    // Count all eligible lines, but retain only the formatted output. A full
    // read of a newline-heavy file must not allocate millions of temporary
    // strings or a second vector containing every line reference.
    for (index, line) in content.lines().enumerate() {
        total_lines += 1;
        if index < start_line || lines_shown >= window {
            continue;
        }
        let number = index + 1;
        let number_width = (number.ilog10() as usize + 1).max(6);
        let additional = number_width
            .checked_add("│ ".len())
            .and_then(|bytes| bytes.checked_add(line.len()))
            .and_then(|bytes| bytes.checked_add(usize::from(lines_shown > 0)))
            .ok_or_else(|| CodingCapabilityError::new(RuntimeDispatchErrorKind::Resource))?;
        selected
            .try_reserve(additional)
            .map_err(|_| CodingCapabilityError::new(RuntimeDispatchErrorKind::Resource))?;
        if lines_shown > 0 {
            selected.push('\n');
        }
        write!(&mut selected, "{number:>6}│ {line}")
            .map_err(|_| CodingCapabilityError::new(RuntimeDispatchErrorKind::Resource))?;
        lines_shown += 1;
    }
    let truncated_by_default =
        default_window && total_lines > request.limits.default_coding_read_lines as usize;

    Ok(json!({
        "content": selected,
        "total_lines": total_lines,
        "lines_shown": lines_shown,
        "truncated_by_default": truncated_by_default,
        "path": resolved.scoped_path.as_str()
    }))
}

pub(super) async fn write_file(
    request: &CodingCapabilityRequest<'_>,
    edit_locks: &SharedCodingEditLocks,
) -> Result<CodingCapabilityOutput, CodingCapabilityError> {
    let path_str = required_str(request.input, "path")?;
    if is_workspace_path(path_str) {
        return Err(input_error());
    }
    let resolved = resolve_required_path(request, "path", FilesystemOperation::WriteFile)?;
    let content = required_str(request.input, "content")?;
    if content.len() > request.limits.max_coding_write_bytes as usize {
        return Err(input_error());
    }
    let scope = read_scope_key(request);
    let _edit_guard = edit_locks
        .lock_edit(&scope, resolved.virtual_path.as_str())
        .await;
    let existing_stat = stat_optional(request, &resolved.virtual_path).await?;
    if let Some(stat) = &existing_stat
        && stat.sensitive
    {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::FilesystemDenied,
        ));
    }
    // Skip reading the old file when the write-only permission is absent or when
    // new content alone would trigger the large-diff fast path in file_diff_preview
    // (the old file read would be wasted).
    let old_content =
        if !operation_allowed(&resolved.grant.permissions, FilesystemOperation::ReadFile)
            || will_use_large_diff_path(content, &request.limits)
        {
            None
        } else {
            existing_text_for_preview(request, &resolved, existing_stat.as_ref()).await
        };
    validate_parent_directory(request, &resolved.virtual_path).await?;
    let mut body = Vec::new();
    body.try_reserve_exact(content.len())
        .map_err(|_| CodingCapabilityError::new(RuntimeDispatchErrorKind::Resource))?;
    body.extend_from_slice(content.as_bytes());
    request
        .filesystem
        .put_opaque(&resolved.virtual_path, body, CasExpectation::Any)
        .await
        .map_err(filesystem_error)?;
    let output = json!({
        "path": resolved.scoped_path.as_str(),
        "bytes_written": content.len(),
        "success": true
    });
    let display_preview = old_content.map(|old_content| {
        file_diff_preview(
            resolved.scoped_path.as_str(),
            &old_content,
            content,
            &request.limits,
        )
    });
    Ok(CodingCapabilityOutput::with_display_preview(
        output,
        display_preview,
    ))
}

pub(super) async fn list_dir(
    request: &CodingCapabilityRequest<'_>,
) -> Result<Value, CodingCapabilityError> {
    let resolved = resolve_optional_path(request, FilesystemOperation::ListDir)?;
    deny_sensitive_existing_path(request, &resolved.virtual_path).await?;
    let recursive = request
        .input
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_depth = optional_usize(request.input, "max_depth")?
        .unwrap_or(request.limits.default_coding_directory_depth as usize);
    let mut entries = collect_list_entries(request, &resolved, recursive, max_depth).await?;
    sort_list_entries(&mut entries);
    let truncated = entries.len() > request.limits.max_coding_directory_entries as usize;
    entries.truncate(request.limits.max_coding_directory_entries as usize);
    let count = entries.len();
    Ok(json!({
        "path": resolved.scoped_path.as_str(),
        "entries": entries.into_iter().map(|entry| entry.display).collect::<Vec<_>>(),
        "count": count,
        "truncated": truncated
    }))
}

async fn collect_list_entries(
    request: &CodingCapabilityRequest<'_>,
    root: &ResolvedPath,
    recursive: bool,
    max_depth: usize,
) -> Result<Vec<ListEntry>, CodingCapabilityError> {
    let mut output = Vec::new();
    let mut stack = vec![(root.virtual_path.clone(), 0usize)];
    let mut visited = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let entries = list_walk_entries(request, &dir, visited).await?;
        for entry in entries {
            visited += 1;
            if visited > request.limits.max_coding_visited_entries as usize {
                return Err(CodingCapabilityError::new(
                    RuntimeDispatchErrorKind::Resource,
                ));
            }
            let relative = virtual_to_relative(&root.virtual_path, &entry.path)?;
            let is_dir = entry.file_type == FileType::Directory;
            let scoped_path = scoped_child_path(&root.scoped_path, &relative);
            let is_sensitive = is_sensitive_scoped_path(&scoped_path);
            // silent-ok: list_dir is best-effort for entries that disappear or fail stat.
            let Ok(stat) = request.filesystem.stat(&entry.path).await else {
                tracing::debug!(
                    path = entry.path.as_str(),
                    "skipping list_dir entry after stat failed"
                );
                continue;
            };
            let is_sensitive = is_sensitive || stat.sensitive;
            let display = if is_dir && recursive && is_sensitive {
                format!("{relative} [sensitive - access blocked]")
            } else if is_dir && is_sensitive {
                continue;
            } else if is_dir {
                format!("{relative}/")
            } else {
                if is_sensitive {
                    continue;
                }
                format!("{} ({})", relative, format_size(stat.len))
            };
            output.push(ListEntry { display, is_dir });
            if recursive
                && is_dir
                && depth < max_depth
                && !is_sensitive
                && !is_excluded_name(entry.name.as_str())
            {
                stack.push((entry.path, depth + 1));
            }
            if output.len() > request.limits.max_coding_directory_entries as usize {
                return Ok(output);
            }
        }
    }
    Ok(output)
}

fn sort_list_entries(entries: &mut [ListEntry]) {
    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.display.cmp(&right.display),
    });
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
    }
}

pub(super) async fn apply_patch(
    request: &CodingCapabilityRequest<'_>,
    edit_locks: &SharedCodingEditLocks,
) -> Result<CodingCapabilityOutput, CodingCapabilityError> {
    let path_str = required_str(request.input, "path")?;
    if is_workspace_path(path_str) {
        return Err(input_error());
    }
    let resolved = resolve_required_path(request, "path", FilesystemOperation::ReadFile)?;
    if !operation_allowed(&resolved.grant.permissions, FilesystemOperation::WriteFile) {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::FilesystemDenied,
        ));
    }
    let old_string = required_str(request.input, "old_string")?;
    let new_string = required_str(request.input, "new_string")?;
    if old_string == new_string {
        return Err(input_error());
    }
    let replace_all = request
        .input
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let scope = read_scope_key(request);
    let _edit_guard = edit_locks
        .lock_edit(&scope, resolved.virtual_path.as_str())
        .await;
    let stat = request
        .filesystem
        .stat(&resolved.virtual_path)
        .await
        .map_err(|error| {
            filesystem_error_with_summary("apply_patch", resolved.scoped_path.as_str(), error)
        })?;
    if stat.sensitive {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::FilesystemDenied,
        ));
    }
    if stat.file_type != FileType::File || stat.len > request.limits.max_coding_patch_bytes {
        return Err(CodingCapabilityError::with_safe_summary(
            RuntimeDispatchErrorKind::Resource,
            format!(
                "apply_patch failed for {}: target is not a file or exceeds the patch size limit",
                safe_summary_path(resolved.scoped_path.as_str())
            ),
        ));
    }
    let bytes = request
        .filesystem
        .read_file_bounded(
            &resolved.virtual_path,
            request.limits.max_coding_patch_bytes as usize,
        )
        .await
        .map_err(|error| {
            filesystem_error_with_summary("apply_patch", resolved.scoped_path.as_str(), error)
        })?
        .ok_or_else(|| CodingCapabilityError::new(RuntimeDispatchErrorKind::Resource))?;
    reject_binary_probe(&bytes)?;
    let (content, encoding, line_ending) = decode_text(&bytes)?;
    let (match_count, match_method) = count_matches(&content, old_string);
    if match_count == 0 {
        return Err(operation_error_with_summary(format!(
            "apply_patch failed for {}: old_string matched 0 times",
            safe_summary_path(resolved.scoped_path.as_str())
        )));
    }
    if !replace_all && match_count > 1 {
        return Err(operation_error_with_summary(format!(
            "apply_patch failed for {}: old_string matched {match_count} times; set replace_all=true or provide a unique old_string",
            safe_summary_path(resolved.scoped_path.as_str())
        )));
    }

    let (new_content, replacements) = replace_content(
        &content,
        old_string,
        new_string,
        replace_all,
        match_count,
        PatchOutputLimits {
            encoding,
            line_ending,
            max_bytes: request.limits.max_coding_patch_bytes,
        },
    )?;
    let output = encode_text(&new_content, encoding, line_ending);
    if output.len() as u64 > request.limits.max_coding_patch_bytes {
        return Err(CodingCapabilityError::new(
            RuntimeDispatchErrorKind::Resource,
        ));
    }
    request
        .filesystem
        .put_opaque(&resolved.virtual_path, output, CasExpectation::Any)
        .await
        .map_err(|error| {
            filesystem_error_with_summary("apply_patch", resolved.scoped_path.as_str(), error)
        })?;
    let mut result = json!({
        "path": resolved.scoped_path.as_str(),
        "replacements": replacements,
        "success": true
    });
    if match_method != MatchMethod::Exact {
        result["match_method"] = json!(format!("{match_method:?}"));
    }
    let display_preview = file_diff_preview(
        resolved.scoped_path.as_str(),
        &content,
        &new_content,
        &request.limits,
    );
    Ok(CodingCapabilityOutput::with_display_preview(
        result,
        Some(display_preview),
    ))
}

fn filesystem_error_with_summary(
    operation: &str,
    scoped_path: &str,
    error: brassclaw_filesystem::FilesystemError,
) -> CodingCapabilityError {
    let scoped_path = safe_summary_path(scoped_path);
    let summary = match &error {
        brassclaw_filesystem::FilesystemError::NotFound { .. } => {
            format!("{operation} failed for {scoped_path}: file not found")
        }
        brassclaw_filesystem::FilesystemError::PermissionDenied { .. }
        | brassclaw_filesystem::FilesystemError::MountNotFound { .. }
        | brassclaw_filesystem::FilesystemError::PathOutsideMount { .. }
        | brassclaw_filesystem::FilesystemError::SymlinkEscape { .. }
        | brassclaw_filesystem::FilesystemError::MountConflict { .. }
        | brassclaw_filesystem::FilesystemError::VersionMismatch { .. }
        | brassclaw_filesystem::FilesystemError::Unsupported { .. }
        | brassclaw_filesystem::FilesystemError::IndexConflict { .. } => {
            format!("{operation} failed for {scoped_path}: permission denied or unsupported path")
        }
        brassclaw_filesystem::FilesystemError::Backend { .. }
        | brassclaw_filesystem::FilesystemError::BackendInfrastructure { .. } => {
            format!("{operation} failed for {scoped_path}: filesystem backend error")
        }
        brassclaw_filesystem::FilesystemError::Contract(_) => {
            format!("{operation} failed for {scoped_path}: invalid path")
        }
        _ => format!("{operation} failed for {scoped_path}: filesystem error"),
    };
    let kind = filesystem_error(error).kind();
    CodingCapabilityError::with_safe_summary(kind, summary)
}

fn safe_summary_path(scoped_path: &str) -> String {
    let path_hint = scoped_path
        .trim_start_matches('/')
        .replace(['/', '\\'], " ");
    format!("path {path_hint}")
}

async fn existing_text_for_preview(
    request: &CodingCapabilityRequest<'_>,
    resolved: &ResolvedPath,
    stat: Option<&brassclaw_filesystem::FileStat>,
) -> Option<String> {
    let Some(stat) = stat else {
        return Some(String::new());
    };
    if stat.file_type != FileType::File || stat.len > request.limits.max_coding_write_bytes {
        return None;
    }
    let bytes = request
        .filesystem
        .read_file_bounded(
            &resolved.virtual_path,
            request.limits.max_coding_write_bytes as usize,
        )
        .await
        // silent-ok: write_file display preview is best-effort; the write result is canonical.
        .ok()
        .flatten()?;
    reject_binary_probe(&bytes).ok()?;
    let (content, _encoding, _line_ending) = decode_text(&bytes).ok()?;
    Some(content)
}
