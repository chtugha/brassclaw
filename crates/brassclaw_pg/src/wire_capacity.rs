//! Finite representation limits of the supported PostgreSQL protocol owner.
//! Not operational graph quotas: larger logical selections are batched whole.

/// PostgreSQL 17 utils/memutils.h MaxAllocSize (varlena/StringInfo allocation),
/// with one-dimensional binary-array framing. A whole outbound request below
/// this bound also keeps each contained Bind/parameter below it. This does not
/// promise allocation success or reserve server/host memory.
pub fn binary_selection_batch_capacity(query_bytes: usize) -> Option<usize> {
    let max_alloc_size = 0x3fff_ffff_usize;
    let overhead = query_bytes.checked_add(106)?;
    let by_request = max_alloc_size.checked_sub(overhead)? / 32;
    // uuid[]:20 header + (4 length+16 UUID)*n; bigint[] is smaller.
    let by_array = max_alloc_size.checked_sub(20)? / 20;
    Some(by_request.min(by_array).min((i32::MAX as usize - 66) / 32))
}
