/// Number of bytes sampled from the start of a file to probe for binary content.
const BINARY_PROBE_LEN: usize = 8192;
/// UTF-16 little-endian byte-order mark (BOM): 0xFF 0xFE.
const UTF16_LE_BOM: [u8; 2] = [0xFF, 0xFE];

use super::CodingCapabilityError;

use super::{
    operation_error,
    types::{FileEncoding, FuzzyMatch, LineEnding, MatchMethod},
};

pub(super) fn reject_binary_probe(bytes: &[u8]) -> Result<(), CodingCapabilityError> {
    if detect_encoding(bytes) == FileEncoding::Utf16Le {
        return Ok(());
    }
    let probe_len = bytes.len().min(BINARY_PROBE_LEN);
    if bytes[..probe_len].contains(&0) {
        return Err(operation_error());
    }
    Ok(())
}

pub(super) fn decode_text(
    bytes: &[u8],
) -> Result<(String, FileEncoding, LineEnding), CodingCapabilityError> {
    let encoding = detect_encoding(bytes);
    let raw = match encoding {
        FileEncoding::Utf8 => String::from_utf8(bytes.to_vec()).map_err(|_| operation_error())?,
        FileEncoding::Utf16Le => {
            let data = bytes.get(2..).unwrap_or_default();
            let (pairs, remainder) = data.as_chunks::<2>();
            if !remainder.is_empty() {
                // An incomplete UTF-16 unit is corrupt input, not a byte to
                // silently discard before reading or rewriting the file.
                return Err(operation_error());
            }
            let units = pairs
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect::<Vec<_>>();
            String::from_utf16(&units).map_err(|_| operation_error())?
        }
    };
    let line_ending = detect_line_ending(&raw);
    Ok((
        raw.replace("\r\n", "\n").replace('\r', "\n"),
        encoding,
        line_ending,
    ))
}

pub(super) fn encode_text(
    content: &str,
    encoding: FileEncoding,
    line_ending: LineEnding,
) -> Vec<u8> {
    let output = match line_ending {
        LineEnding::Lf => content.to_string(),
        LineEnding::CrLf => content.replace('\n', "\r\n"),
        LineEnding::Cr => content.replace('\n', "\r"),
    };
    match encoding {
        FileEncoding::Utf8 => output.into_bytes(),
        FileEncoding::Utf16Le => {
            let mut bytes = Vec::from(UTF16_LE_BOM);
            for unit in output.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            bytes
        }
    }
}

fn detect_encoding(bytes: &[u8]) -> FileEncoding {
    if bytes.len() >= 2 && bytes[0] == UTF16_LE_BOM[0] && bytes[1] == UTF16_LE_BOM[1] {
        FileEncoding::Utf16Le
    } else {
        FileEncoding::Utf8
    }
}

fn detect_line_ending(content: &str) -> LineEnding {
    let crlf = content.matches("\r\n").count();
    let cr_only = content.matches('\r').count().saturating_sub(crlf);
    let lf_only = content.matches('\n').count().saturating_sub(crlf);
    if crlf >= lf_only && crlf >= cr_only {
        if crlf == 0 {
            LineEnding::Lf
        } else {
            LineEnding::CrLf
        }
    } else if cr_only > lf_only {
        LineEnding::Cr
    } else {
        LineEnding::Lf
    }
}

/// The output limit applies to the actual file representation, including its
/// BOM and restored line endings, rather than the normalized UTF-8 workspace.
#[derive(Clone, Copy)]
pub(super) struct PatchOutputLimits {
    pub encoding: FileEncoding,
    pub line_ending: LineEnding,
    pub max_bytes: u64,
}

pub(super) fn replace_content(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
    match_count: usize,
    limits: PatchOutputLimits,
) -> Result<(String, usize), CodingCapabilityError> {
    let resource_error =
        || CodingCapabilityError::new(brassclaw_host_api::RuntimeDispatchErrorKind::Resource);
    let mut encoded_bytes = match limits.encoding {
        FileEncoding::Utf8 => 0,
        FileEncoding::Utf16Le => 2u64,
    };
    let mut workspace_bytes = 0usize;
    // Preflight before allocating either the replacement string or encoded
    // output. Keep no per-match vector: a tiny file can contain millions of
    // matches, and a small replacement can amplify it beyond available memory.
    let replacements =
        visit_replacement_parts(content, old_string, new_string, replace_all, |part| {
            workspace_bytes = workspace_bytes
                .checked_add(part.len())
                .ok_or_else(resource_error)?;
            let units = match limits.encoding {
                FileEncoding::Utf8 => part.len() as u64,
                FileEncoding::Utf16Le => (part.encode_utf16().count() as u64)
                    .checked_mul(2)
                    .ok_or_else(resource_error)?,
            };
            let extra = if limits.line_ending == LineEnding::CrLf {
                let width = if limits.encoding == FileEncoding::Utf16Le {
                    2
                } else {
                    1
                };
                (part.bytes().filter(|byte| *byte == b'\n').count() as u64)
                    .checked_mul(width)
                    .ok_or_else(resource_error)?
            } else {
                0
            };
            encoded_bytes = encoded_bytes
                .checked_add(units)
                .and_then(|bytes| bytes.checked_add(extra))
                .ok_or_else(resource_error)?;
            if encoded_bytes > limits.max_bytes || encoded_bytes > isize::MAX as u64 {
                return Err(resource_error());
            }
            Ok(())
        })?;
    if replacements == 0 || replacements != match_count {
        return Err(operation_error());
    }
    let mut rebuilt = String::new();
    rebuilt
        .try_reserve_exact(workspace_bytes)
        .map_err(|_| resource_error())?;
    visit_replacement_parts(content, old_string, new_string, replace_all, |part| {
        rebuilt.push_str(part);
        Ok(())
    })?;
    Ok((rebuilt, replacements))
}

fn visit_replacement_parts(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
    mut visit: impl FnMut(&str) -> Result<(), CodingCapabilityError>,
) -> Result<usize, CodingCapabilityError> {
    let mut count = 0;
    let mut last = 0;
    while let Some(item) = find_match_from(content, old_string, last) {
        if item.end <= item.start {
            return Err(operation_error());
        }
        visit(&content[last..item.start])?;
        visit(new_string)?;
        last = item.end;
        count += 1;
        if !replace_all {
            break;
        }
    }
    visit(&content[last..])?;
    Ok(count)
}

fn find_match_from(haystack: &str, needle: &str, start_offset: usize) -> Option<FuzzyMatch> {
    let search = haystack.get(start_offset..)?;
    if let Some(index) = search.find(needle) {
        let start = start_offset + index;
        return Some(FuzzyMatch {
            start,
            end: start + needle.len(),
            method: MatchMethod::Exact,
        });
    }
    let needle_stripped = strip_trailing_whitespace(needle);
    let haystack_stripped = strip_trailing_whitespace(search);
    if let Some((start, end)) = find_normalized_span(search, &haystack_stripped, &needle_stripped) {
        return Some(FuzzyMatch {
            start: start_offset + start,
            end: start_offset + end,
            method: MatchMethod::TrailingWhitespace,
        });
    }
    let needle_normalized = normalize_quotes(needle);
    let haystack_normalized = normalize_quotes(search);
    if let Some(index) = haystack_normalized.find(&needle_normalized) {
        let char_start = haystack_normalized[..index].chars().count();
        let char_len = needle_normalized.chars().count();
        let start = char_to_byte_idx(search, char_start)?;
        let end = char_to_byte_idx(search, char_start + char_len)?;
        return Some(FuzzyMatch {
            start: start_offset + start,
            end: start_offset + end,
            method: MatchMethod::QuoteNormalization,
        });
    }
    let needle_both = normalize_quotes(&needle_stripped);
    let haystack_both = normalize_quotes(&haystack_stripped);
    find_normalized_span(search, &haystack_both, &needle_both).map(|(start, end)| FuzzyMatch {
        start: start_offset + start,
        end: start_offset + end,
        method: MatchMethod::Both,
    })
}

pub(super) fn count_matches(haystack: &str, needle: &str) -> (usize, MatchMethod) {
    let mut count = 0usize;
    let mut method = MatchMethod::Exact;
    let mut search_offset = 0usize;
    while let Some(item) = find_match_from(haystack, needle, search_offset) {
        if item.end <= item.start {
            break;
        }
        if count == 0 {
            method = item.method;
        }
        count += 1;
        search_offset = item.end;
    }
    (count, method)
}

fn strip_trailing_whitespace(value: &str) -> String {
    value
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_quotes(value: &str) -> String {
    value
        .replace(['\u{2018}', '\u{2019}', '\u{2032}'], "'")
        .replace(['\u{201C}', '\u{201D}', '\u{2033}'], "\"")
}

fn find_normalized_span(original: &str, normalized: &str, needle: &str) -> Option<(usize, usize)> {
    let index = normalized.find(needle)?;
    let char_index = normalized[..index].chars().count();
    let char_len = needle.chars().count();
    let start = map_normalized_char_to_original_byte(original, char_index)?;
    let end = map_normalized_char_to_original_byte(original, char_index + char_len)?;
    Some((start, end))
}

fn char_to_byte_idx(value: &str, char_index: usize) -> Option<usize> {
    if char_index == value.chars().count() {
        return Some(value.len());
    }
    value.char_indices().nth(char_index).map(|(index, _)| index)
}

fn map_normalized_char_to_original_byte(
    original: &str,
    normalized_char_index: usize,
) -> Option<usize> {
    if normalized_char_index == 0 {
        return Some(0);
    }
    let mut normalized_seen = 0usize;
    let mut original_byte = 0usize;
    for segment in original.split_inclusive('\n') {
        let (line, has_newline) = if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, true)
        } else {
            (segment, false)
        };
        let trimmed = line.trim_end();
        let trimmed_chars = trimmed.chars().count();
        if normalized_char_index <= normalized_seen + trimmed_chars {
            let within_line = normalized_char_index - normalized_seen;
            return Some(original_byte + char_to_byte_idx(line, within_line)?);
        }
        normalized_seen += trimmed_chars;
        original_byte += line.len();
        if has_newline {
            if normalized_char_index == normalized_seen + 1 {
                return Some(original_byte);
            }
            normalized_seen += 1;
            original_byte += 1;
        }
    }
    if normalized_char_index == normalized_seen {
        Some(original_byte)
    } else {
        None
    }
}

pub(super) fn previous_char_boundary(value: &str, mut end: usize) -> usize {
    end = end.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_limit_counts_the_encoded_file_and_accepts_its_exact_boundary() {
        for (content, old, new, encoding, line_ending, max_bytes) in [
            ("x\n", "x", "é", FileEncoding::Utf8, LineEnding::CrLf, 4),
            ("x\n", "x", "😀", FileEncoding::Utf8, LineEnding::Lf, 5),
            ("x\n", "x", "😀", FileEncoding::Utf8, LineEnding::Cr, 5),
            (
                "x\n",
                "x",
                "😀",
                FileEncoding::Utf16Le,
                LineEnding::CrLf,
                10,
            ),
            ("x\n", "x", "界", FileEncoding::Utf16Le, LineEnding::Cr, 6),
            // The normalized UTF-8 workspace exceeds the encoded file budget.
            (
                "x界界界界",
                "x",
                "y",
                FileEncoding::Utf16Le,
                LineEnding::Lf,
                12,
            ),
        ] {
            let limits = PatchOutputLimits {
                encoding,
                line_ending,
                max_bytes,
            };
            let (replaced, count) = replace_content(content, old, new, false, 1, limits).unwrap();
            assert_eq!(count, 1);
            assert_eq!(
                encode_text(&replaced, encoding, line_ending).len() as u64,
                max_bytes
            );
            let error = replace_content(
                content,
                old,
                new,
                false,
                1,
                PatchOutputLimits {
                    max_bytes: max_bytes - 1,
                    ..limits
                },
            )
            .unwrap_err();
            assert_eq!(
                error.kind(),
                brassclaw_host_api::RuntimeDispatchErrorKind::Resource
            );
        }
    }

    #[test]
    fn bounded_patch_preserves_exact_and_fuzzy_replacement_order() {
        let limits = PatchOutputLimits {
            encoding: FileEncoding::Utf8,
            line_ending: LineEnding::Lf,
            max_bytes: 1024,
        };
        // Existing behavior prefers later exact matches before earlier fuzzy
        // matches. The limit check must not change which spans are replaced.
        let content = "‘x’ 'x' ‘x’";
        let (count, _) = count_matches(content, "'x'");
        assert_eq!(count, 2);
        let (replaced, replacements) =
            replace_content(content, "'x'", "done", true, count, limits).unwrap();
        assert_eq!(replacements, count);
        assert_eq!(replaced, "‘x’ done done");
    }

    #[test]
    fn count_matches_ignores_unlocatable_trailing_whitespace_normalization() {
        let (count, method) = count_matches("\na", "\n ");

        assert_eq!(count, 0);
        assert_eq!(method, MatchMethod::Exact);
    }
}
