pub mod filesystem;

pub use filesystem::{
    FilesystemCapabilityError, FilesystemCapabilityState, FilesystemContext,
    APPLY_PATCH_CAPABILITY_ID, FILE_UNDO_CAPABILITY_ID, GLOB_CAPABILITY_ID,
    GREP_CAPABILITY_ID, LIST_DIR_CAPABILITY_ID, READ_FILE_CAPABILITY_ID,
    WRITE_FILE_CAPABILITY_ID,
};

use brassclaw_host_api::CapabilityDescriptor;

pub fn register_all() -> Vec<CapabilityDescriptor> {
    let mut descriptors = Vec::new();
    descriptors.extend(filesystem::descriptors());
    descriptors
}
