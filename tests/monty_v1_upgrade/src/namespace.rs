//! Actual tagged-1.0 host object used by the migration acceptance checks.
//! Routing identity is not an authority grant; dispatch still belongs to Rust.

use monty_types::{MontyObject, MontyUuid};

pub const HOST_INSTANCE_ID: MontyUuid = MontyUuid::from_u128(0x484f_5354);

pub fn host_namespace() -> MontyObject {
    MontyObject::class_instance(
        MontyObject::class_type(
            "BrassClawHost",
            MontyUuid::from_u128(0x484f_5354_5459_5045),
            true,
            true,
            [],
        ),
        HOST_INSTANCE_ID,
        [],
    )
}
