//! Finite representation limits of the supported PostgreSQL protocol owner.
//! Not operational graph quotas: larger logical selections are batched whole.

/// PostgreSQL 17 utils/memutils.h MaxAllocSize (varlena/StringInfo allocation),
/// with one-dimensional binary-array framing. A whole outbound request below
/// this bound also keeps each contained Bind/parameter below it. This does not
/// promise allocation success or reserve server/host memory.
const MAX_ALLOC_SIZE: usize = 0x3fff_ffff;

pub fn binary_selection_batch_capacity(query_bytes: usize) -> Option<usize> {
    let max_alloc_size = MAX_ALLOC_SIZE;
    let overhead = query_bytes.checked_add(106)?;
    let by_request = max_alloc_size.checked_sub(overhead)? / 32;
    // uuid[]:20 header + (4 length+16 UUID)*n; bigint[] is smaller.
    let by_array = max_alloc_size.checked_sub(20)? / 20;
    Some(by_request.min(by_array).min((i32::MAX as usize - 66) / 32))
}

/// PostgreSQL varlena text content must leave its four-byte header room.
/// Intrinsic representation, not an application record quota.
pub const fn maximum_text_content_bytes() -> usize {
    MAX_ALLOC_SIZE - 4
}

/// Supported tokio-postgres 0.7 extended-query representation. The prepared
/// name is `s{AtomicUsize}`; Parse/Describe/Sync and Bind/Execute/Sync frames are
/// counted before the driver allocates their buffers. Every parameter here is
/// its actual binary payload size (text UTF-8, UUID16, bytea checksum32, etc.).
/// Counting the two requests together is conservative for server StringInfo.
pub fn text_record_fits(query_bytes: usize, parameters: &[usize]) -> bool {
    let Some(name_bytes) = usize::MAX.to_string().len().checked_add(1) else {
        return false;
    };
    if parameters.len() > u16::MAX as usize
        || parameters.iter().any(|n| *n > maximum_text_content_bytes())
    {
        return false;
    }
    // Parse: tag/length/nameNUL/queryNUL/typeCount+OIDs.
    let Some(parse) = (1usize + 4 + name_bytes + 1 + 1 + 2).checked_add(query_bytes) else {
        return false;
    };
    // Describe statement: tag/length/variant/nameNUL. Sync: tag/length.
    let describe = 1usize + 4 + 1 + name_bytes + 1 + 1 + 4;
    // Bind: tag/length/emptyPortalNUL/nameNUL/formatCount/paramCount/
    // resultFormatCount+one binary format. Execute empty portal + Sync.
    let bind = 1usize + 4 + 1 + name_bytes + 1 + 2 + 2 + 2 + 2;
    let execute_sync = 1usize + 4 + 1 + 4 + 1 + 4;
    parse
        .checked_add(describe)
        .and_then(|n| n.checked_add(bind))
        .and_then(|n| n.checked_add(execute_sync))
        .and_then(|n| {
            parameters
                .len()
                .checked_mul(10)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            parameters
                .iter()
                .try_fold(n, |sum, len| sum.checked_add(*len))
        })
        .is_some_and(|n| n <= MAX_ALLOC_SIZE && n <= i32::MAX as usize)
}

#[cfg(test)]
mod text_tests {
    use super::*;
    #[test]
    fn record_capacity_checks_whole_request_and_overflow_without_large_buffers() {
        assert!(text_record_fits(128, &[16, 32, 1024, 64]));
        assert!(!text_record_fits(128, &[maximum_text_content_bytes()]));
        assert!(!text_record_fits(128, &[usize::MAX]));
        assert!(!text_record_fits(usize::MAX, &[16]));
        assert!(!text_record_fits(
            128,
            &[
                maximum_text_content_bytes() / 2,
                maximum_text_content_bytes() / 2
            ]
        ));
    }
}
