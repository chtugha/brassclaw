//! Existing IBS step-link grammar, shared with retained selection validation.

/// One segment of a parsed `step_link` formula (§0.6).
///
/// `{desc_idx}:{start}-{desc_idx}:{end}` → one [`StepRange`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRange {
    /// 0-based StepDescription index.
    pub desc_idx: usize,
    /// 1-based stepnumber, or `0` (sentinel = first step in the sequence).
    pub start: u32,
    /// End bound — either a 1-based stepnumber or `E` (last step).
    pub end: StepBound,
}

/// End bound of a [`StepRange`] (§0.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepBound {
    /// 1-based stepnumber.
    Num(u32),
    /// `E` sentinel = last step in the sequence.
    End,
}

#[derive(Debug, Clone)]
pub struct StepLinkError {
    pub formula: String,
    pub reason: String,
}

pub fn parse_step_link(step_link: &str) -> Result<Vec<StepRange>, StepLinkError> {
    let formula = step_link.to_string();
    let trimmed = step_link.trim();
    if trimmed.is_empty() {
        return Err(StepLinkError {
            formula,
            reason: "empty step_link".into(),
        });
    }

    let mut ranges = Vec::new();
    for raw_segment in trimmed.split('+') {
        let segment = raw_segment.trim();
        if segment.is_empty() {
            return Err(StepLinkError {
                formula,
                reason: "empty segment in step_link".into(),
            });
        }
        let (left, right) = segment.split_once('-').ok_or_else(|| StepLinkError {
            formula: formula.clone(),
            reason: format!("segment {segment:?} missing '-' separator"),
        })?;
        let (left_idx, start_str) = parse_bound_part(left).map_err(|reason| StepLinkError {
            formula: formula.clone(),
            reason,
        })?;
        let (right_idx, end_str) = parse_bound_part(right).map_err(|reason| StepLinkError {
            formula: formula.clone(),
            reason,
        })?;
        if left_idx != right_idx {
            return Err(StepLinkError {
                formula,
                reason: format!("segment {segment:?} desc_idx mismatch: {left_idx} vs {right_idx}"),
            });
        }
        let start: u32 = start_str.parse().map_err(|_| StepLinkError {
            formula: formula.clone(),
            reason: format!("invalid start bound {start_str:?}"),
        })?;
        let end = parse_end_bound(end_str).map_err(|reason| StepLinkError {
            formula: formula.clone(),
            reason,
        })?;
        ranges.push(StepRange {
            desc_idx: left_idx,
            start,
            end,
        });
    }

    Ok(ranges)
}

fn parse_bound_part(s: &str) -> Result<(usize, &str), String> {
    let s = s.trim();
    let (idx_str, bound_str) = s
        .split_once(':')
        .ok_or_else(|| format!("bound {s:?} missing ':' separator"))?;
    let idx: usize = idx_str
        .trim()
        .parse()
        .map_err(|_| format!("invalid desc_idx {idx_str:?}"))?;
    Ok((idx, bound_str.trim()))
}

fn parse_end_bound(s: &str) -> Result<StepBound, String> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("E") {
        return Ok(StepBound::End);
    }
    let n: u32 = s
        .parse()
        .map_err(|_| format!("invalid end bound {s:?} (expected number or 'E')"))?;
    Ok(StepBound::Num(n))
}
