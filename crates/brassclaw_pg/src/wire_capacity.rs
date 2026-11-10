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

/// Exact supported unnamed typed exchange: Parse, Bind (binary parameters and
/// result format), Describe portal, Execute, Sync. Negative NULL lengths carry
/// no payload. Every contained protocol message/varlena must be representable.
pub fn unnamed_typed_request_bytes(query_bytes: usize, lengths: &[usize]) -> Option<usize> {
    unnamed_typed_request_bytes_with_capacity(query_bytes, lengths, isize::MAX as usize)
}
fn unnamed_typed_request_bytes_with_capacity(
    query_bytes: usize,
    lengths: &[usize],
    native_capacity: usize,
) -> Option<usize> {
    if lengths.len() > u16::MAX as usize {
        return None;
    }
    let payload = lengths.iter().try_fold(0_usize, |total, length| {
        if *length > maximum_text_content_bytes() {
            return None;
        }
        total.checked_add(*length)
    })?;
    let parse = query_bytes
        .checked_add(9)?
        .checked_add(4_usize.checked_mul(lengths.len())?)?;
    let bind = 15_usize
        .checked_add(6_usize.checked_mul(lengths.len())?)?
        .checked_add(payload)?;
    // Each server input message owns a StringInfo payload (wire minus type and
    // length), including its NUL terminator. The exchange itself is not one
    // PostgreSQL message. Its contiguous native buffer still must fit isize.
    for message in [parse, bind] {
        if message.checked_sub(4)? > MAX_ALLOC_SIZE || message.checked_sub(1)? > i32::MAX as usize {
            return None;
        }
    }
    // Describe portal 7, Execute 10, Sync 5.
    let exchange = parse.checked_add(bind)?.checked_add(22)?;
    (exchange <= native_capacity).then_some(exchange)
}
/// Five-field exact revision DataRow: UUID16, class2, version8, checksum64,
/// five field lengths, field-count and message header. Source text is complete.
pub fn revision_row_bytes(content_bytes: usize) -> Option<usize> {
    binary_data_row_bytes(&[Some(16), Some(2), Some(8), Some(content_bytes), Some(64)])
}

/// DataRow header, field count, signed lengths and complete binary payloads.
/// PostgreSQL text payloads need their four-byte varlena header within MaxAllocSize.
pub fn binary_data_row_bytes(lengths: &[Option<usize>]) -> Option<usize> {
    if lengths.len() > u16::MAX as usize {
        return None;
    }
    let mut payload = 2_usize.checked_add(lengths.len().checked_mul(4)?)?;
    for length in lengths.iter().flatten() {
        if *length > maximum_text_content_bytes() {
            return None;
        }
        payload = payload.checked_add(*length)?;
    }
    // pq_beginmessage's StringInfo contains the field count, lengths and all
    // fields together, plus its terminating NUL. Type/length are sent outside
    // that buffer. The signed protocol length includes its four-byte header.
    if payload.checked_add(1)? > MAX_ALLOC_SIZE || payload.checked_add(4)? > i32::MAX as usize {
        return None;
    }
    payload.checked_add(5)
}

#[cfg(test)]
mod revision_wire_tests {
    use super::*;
    #[test]
    fn complete_unnamed_exchange_respects_native_contiguous_buffer_capacity() {
        // Both PG messages are individually representable, yet their combined
        // BytesMut exchange exceeds a 32-bit isize capacity. No large buffer.
        let query = MAX_ALLOC_SIZE - 9;
        let values = [MAX_ALLOC_SIZE - 17];
        let exchange = 2 * MAX_ALLOC_SIZE + 30;
        assert!(
            unnamed_typed_request_bytes_with_capacity(query, &values, i32::MAX as usize).is_none()
        );
        assert_eq!(
            unnamed_typed_request_bytes_with_capacity(query, &values, exchange),
            Some(exchange)
        );
        assert!(unnamed_typed_request_bytes_with_capacity(query, &values, exchange - 1).is_none());
        if usize::BITS == 32 {
            assert!(unnamed_typed_request_bytes(query, &values).is_none());
        } else {
            assert_eq!(unnamed_typed_request_bytes(query, &values), Some(exchange));
        }
    }
    #[test]
    fn unnamed_revision_requests_and_all_row_shapes_count_complete_payloads() {
        assert_eq!(
            unnamed_typed_request_bytes(10, &[16, 2, 8, 100, 64, 0, 0]),
            Some(10 + 46 + 70 + 16 + 2 + 8 + 100 + 64)
        );
        assert_eq!(revision_row_bytes(100), Some(217));
        assert_eq!(binary_data_row_bytes(&[Some(100)]), Some(111));
        assert_eq!(
            binary_data_row_bytes(&[
                Some(16),
                Some(2),
                Some(8),
                Some(4),
                Some(64),
                Some(1),
                None,
                None
            ]),
            Some(134)
        );
        let capture = 4567;
        assert_eq!(
            binary_data_row_bytes(&[
                Some(16),
                Some(2),
                Some(8),
                Some(4),
                Some(64),
                Some(1),
                Some(capture),
                Some(64)
            ]),
            Some(198 + capture)
        );
        assert!(unnamed_typed_request_bytes(usize::MAX, &[1]).is_none());
        assert!(binary_data_row_bytes(&[Some(maximum_text_content_bytes() + 1)]).is_none());
        assert!(binary_data_row_bytes(&[Some(maximum_text_content_bytes())]).is_none());
        let single = MAX_ALLOC_SIZE - 7; // count2 + length4 + StringInfo NUL1
        assert_eq!(
            binary_data_row_bytes(&[Some(single)]),
            Some(MAX_ALLOC_SIZE + 4)
        );
        assert!(binary_data_row_bytes(&[Some(single + 1)]).is_none());
        assert!(
            binary_data_row_bytes(&[Some(MAX_ALLOC_SIZE / 2), Some(MAX_ALLOC_SIZE / 2)]).is_none()
        );
        assert!(binary_data_row_bytes(&vec![None; u16::MAX as usize]).is_some());
        assert!(binary_data_row_bytes(&vec![None; u16::MAX as usize + 1]).is_none());
        assert!(unnamed_typed_request_bytes(10, &vec![0; u16::MAX as usize]).is_some());
        assert!(unnamed_typed_request_bytes(10, &vec![0; u16::MAX as usize + 1]).is_none());
    }
}
