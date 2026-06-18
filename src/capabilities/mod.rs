pub mod filesystem;
pub mod memory;
pub mod messaging;
pub mod network;
pub mod shell;

pub use filesystem::{
    FilesystemCapabilityError, FilesystemCapabilityState, FilesystemContext,
    APPLY_PATCH_CAPABILITY_ID, FILE_UNDO_CAPABILITY_ID, GLOB_CAPABILITY_ID,
    GREP_CAPABILITY_ID, LIST_DIR_CAPABILITY_ID, READ_FILE_CAPABILITY_ID,
    WRITE_FILE_CAPABILITY_ID,
};
pub use memory::{
    MemoryCapabilityError, MemoryContext,
    MEMORY_READ_CAPABILITY_ID, MEMORY_SEARCH_CAPABILITY_ID,
    MEMORY_TREE_CAPABILITY_ID, MEMORY_WRITE_CAPABILITY_ID,
};
pub use messaging::{
    MessagingCapabilityError, MessagingContext,
    MESSAGE_CAPABILITY_ID,
};
pub use network::{
    NetworkCapabilityError, NetworkContext,
    HTTP_CAPABILITY_ID,
};
pub use shell::{
    ShellCapabilityError, ShellContext,
    SHELL_CAPABILITY_ID,
};

use brassclaw_host_api::CapabilityDescriptor;

pub fn register_all() -> Vec<CapabilityDescriptor> {
    let mut descriptors = Vec::new();
    descriptors.extend(filesystem::descriptors());
    descriptors.extend(shell::descriptors());
    descriptors.extend(network::descriptors());
    descriptors.extend(memory::descriptors());
    descriptors.extend(messaging::descriptors());
    descriptors
}
