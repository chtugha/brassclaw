//! Non-running wire-size projection. No projected member/key is an observation,
//! selected task, approval or permission. Large counts are streamed, not allocated.
use std::io::{self, Write};

use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use serde_json::{Map, Value};

use super::{PROTOCOL, ProcessSnapshot, Reply};
use crate::{ContinuationKey, VmError, VmFailure, global::PendingCallCapacity};

#[derive(Clone, Copy)]
struct Members<'a> {
    target: u32,
    retained: &'a [u32],
    borrowed: &'a [u32],
    count: u32,
}
impl Members<'_> {
    fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.target).chain(crate::global::extra_pool_ids(
            self.retained,
            self.borrowed,
            self.target,
        ))
    }
}

#[derive(Clone, Copy)]
enum ArrayKind {
    Ids,
    Waits,
    Keys,
}
struct FutureArray<'a> {
    members: Members<'a>,
    kind: ArrayKind,
    coordinator: bool,
    key: ContinuationKey,
}
impl Serialize for FutureArray<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let count = self.members.count as usize + usize::from(self.coordinator);
        let mut sequence = serializer.serialize_seq(Some(count))?;
        for id in self.members.ids() {
            match self.kind {
                ArrayKind::Ids => sequence.serialize_element(&id)?,
                ArrayKind::Waits => sequence.serialize_element(&(id, self.key))?,
                ArrayKind::Keys => sequence.serialize_element(&self.key)?,
            }
        }
        if self.coordinator {
            sequence.serialize_element(&self.key)?;
        }
        sequence.end()
    }
}

#[derive(Serialize)]
struct FutureCapacity<'a> {
    desired: u32,
    effective: u32,
    active_ids: FutureArray<'a>,
    borrowed_ids: &'a [u32],
    coordinator_waiting: bool,
}
#[derive(Serialize)]
struct FuturePool<'a> {
    capacity: FutureCapacity<'a>,
    coordinator: Option<ContinuationKey>,
    dependents: &'a Value,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FutureBoundary<'a> {
    Waiting { keys: FutureArray<'a> },
}

struct FutureSnapshot<'a> {
    base: &'a Map<String, Value>,
    members: Members<'a>,
    key: ContinuationKey,
    pending: PendingCallCapacity,
}
impl FutureSnapshot<'_> {
    fn array(&self, kind: ArrayKind, coordinator: bool) -> FutureArray<'_> {
        FutureArray {
            members: self.members,
            kind,
            coordinator,
            key: self.key,
        }
    }
}
impl Serialize for FutureSnapshot<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // All other fields come from the real DTO, including future additions.
        // Only future hosting arrays/pool metadata replace their current shape.
        let mut map = serializer.serialize_map(Some(self.base.len()))?;
        for (name, value) in self.base {
            match name.as_str() {
                "worker_pool" => map.serialize_entry(
                    name,
                    &Some(FuturePool {
                        capacity: FutureCapacity {
                            desired: self.members.target,
                            effective: self.members.count,
                            active_ids: self.array(ArrayKind::Ids, false),
                            borrowed_ids: self.members.borrowed,
                            coordinator_waiting: true,
                        },
                        coordinator: Some(self.key),
                        dependents: &self.base["worker_pool"]["dependents"],
                    }),
                )?,
                "work_waits" => map.serialize_entry(name, &self.array(ArrayKind::Waits, false))?,
                "outstanding" => map.serialize_entry(name, &self.array(ArrayKind::Keys, true))?,
                "boundary" => map.serialize_entry(
                    name,
                    &Some(FutureBoundary::Waiting {
                        keys: self.array(ArrayKind::Keys, true),
                    }),
                )?,
                "pending_call_capacity" => map.serialize_entry(name, &Some(self.pending))?,
                _ => map.serialize_entry(name, value)?,
            }
        }
        map.end()
    }
}

struct SizeCounter {
    limit: usize,
    used: usize,
}
impl Write for SizeCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let used = self
            .used
            .checked_add(bytes.len())
            .filter(|used| *used <= self.limit)
            .ok_or_else(|| {
                io::Error::other("projected worker frame exceeds configured capacity")
            })?;
        self.used = used;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn validate(
    snapshot: &ProcessSnapshot,
    target: u32,
    limit: usize,
) -> Result<(), VmError> {
    let fail = || VmError::kind(VmFailure::InvalidBounds);
    let pool = snapshot.worker_pool.as_ref().ok_or_else(fail)?;
    let mut prospective = pool.capacity.clone();
    prospective.desired = target;
    let pending_count = prospective.required_pending_calls().ok_or_else(fail)?;
    let mut pending = snapshot.pending_call_capacity.ok_or_else(fail)?;
    if pending_count > pending.limit {
        return Err(fail());
    }
    pending.pending = pending_count;
    pending.minimum = pending_count;
    let base = serde_json::to_value(snapshot).map_err(|_| fail())?;
    let base = base.as_object().ok_or_else(fail)?;
    let root = snapshot.root.ok_or_else(fail)?;
    let projected = FutureSnapshot {
        base,
        members: Members {
            target,
            retained: &pool.capacity.active_ids,
            borrowed: &pool.capacity.borrowed_ids,
            count: pending_count - 1,
        },
        // Max-width ordinals bound both the initial receipt and later idle-wait
        // receipts. Repeated keys here are size-only, never accepted wire state.
        key: ContinuationKey {
            vm_id: root.vm_id(),
            ordinal: u64::MAX,
        },
        pending,
    };
    let mut counter = SizeCounter { limit, used: 0 };
    serde_json::to_writer(
        &mut counter,
        &Reply {
            protocol: PROTOCOL,
            sequence: u64::MAX,
            max_frame_bytes: limit,
            snapshot: projected,
            failure: None,
            diagnostic: None,
        },
    )
    .map_err(|_| fail())
}
