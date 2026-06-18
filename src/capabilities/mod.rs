pub mod extensions;
pub mod filesystem;
pub mod images;
pub mod jobs;
pub mod memory;
pub mod messaging;
pub mod network;
pub mod pairing;
pub mod routines;
pub mod secrets;
pub mod shell;
pub mod skills;
pub mod system;

pub use extensions::{
    ExtensionsCapabilityError, ExtensionsContext,
    EXTENSION_INFO_CAPABILITY_ID, TOOL_AUTH_CAPABILITY_ID, TOOL_INFO_CAPABILITY_ID,
    TOOL_INSTALL_CAPABILITY_ID, TOOL_LIST_CAPABILITY_ID, TOOL_PERMISSION_SET_CAPABILITY_ID,
    TOOL_REMOVE_CAPABILITY_ID, TOOL_SEARCH_CAPABILITY_ID, TOOL_UPGRADE_CAPABILITY_ID,
};
pub use filesystem::{
    FilesystemCapabilityError, FilesystemCapabilityState, FilesystemContext,
    APPLY_PATCH_CAPABILITY_ID, FILE_UNDO_CAPABILITY_ID, GLOB_CAPABILITY_ID,
    GREP_CAPABILITY_ID, LIST_DIR_CAPABILITY_ID, READ_FILE_CAPABILITY_ID,
    WRITE_FILE_CAPABILITY_ID,
};
pub use images::{
    ImagesCapabilityError, ImagesContext,
    IMAGE_ANALYZE_CAPABILITY_ID, IMAGE_EDIT_CAPABILITY_ID, IMAGE_GENERATE_CAPABILITY_ID,
};
pub use jobs::{
    JobsCapabilityError, JobsContext,
    CANCEL_JOB_CAPABILITY_ID, CREATE_JOB_CAPABILITY_ID, JOB_EVENTS_CAPABILITY_ID,
    JOB_PROMPT_CAPABILITY_ID, JOB_STATUS_CAPABILITY_ID, LIST_JOBS_CAPABILITY_ID,
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
pub use pairing::{
    PairingCapabilityError, PairingContext,
    PAIRING_APPROVE_CAPABILITY_ID,
};
pub use routines::{
    RoutinesCapabilityError, RoutinesContext,
    EVENT_EMIT_CAPABILITY_ID, ROUTINE_CREATE_CAPABILITY_ID, ROUTINE_DELETE_CAPABILITY_ID,
    ROUTINE_FIRE_CAPABILITY_ID, ROUTINE_HISTORY_CAPABILITY_ID, ROUTINE_LIST_CAPABILITY_ID,
    ROUTINE_UPDATE_CAPABILITY_ID,
};
pub use secrets::{
    SecretsCapabilityError, SecretsContext,
    SECRET_DELETE_CAPABILITY_ID, SECRET_LIST_CAPABILITY_ID,
};
pub use shell::{
    ShellCapabilityError, ShellContext,
    SHELL_CAPABILITY_ID,
};
pub use skills::{
    SkillsCapabilityError, SkillsContext,
    SKILL_INSTALL_CAPABILITY_ID, SKILL_LIST_CAPABILITY_ID, SKILL_REMOVE_CAPABILITY_ID,
    SKILL_SEARCH_CAPABILITY_ID,
};
pub use system::{
    SystemCapabilityError, SystemContext,
    ECHO_CAPABILITY_ID, JSON_CAPABILITY_ID, PLAN_UPDATE_CAPABILITY_ID,
    RESTART_CAPABILITY_ID, SYSTEM_TOOLS_LIST_CAPABILITY_ID, SYSTEM_VERSION_CAPABILITY_ID,
    TIME_CAPABILITY_ID,
};

use brassclaw_host_api::CapabilityDescriptor;

pub fn register_all() -> Vec<CapabilityDescriptor> {
    let mut descriptors = Vec::new();
    descriptors.extend(filesystem::descriptors());
    descriptors.extend(shell::descriptors());
    descriptors.extend(network::descriptors());
    descriptors.extend(memory::descriptors());
    descriptors.extend(messaging::descriptors());
    descriptors.extend(jobs::descriptors());
    descriptors.extend(routines::descriptors());
    descriptors.extend(skills::descriptors());
    descriptors.extend(extensions::descriptors());
    descriptors.extend(secrets::descriptors());
    descriptors.extend(images::descriptors());
    descriptors.extend(system::descriptors());
    descriptors.extend(pairing::descriptors());
    descriptors
}
