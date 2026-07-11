//! Ownership types used by the database and pairing layers.
//!
//! [`UserId`] and [`UserRole`] are the typed identity values returned by
//! `ChannelPairingStore::resolve_channel_identity`. They are kept in `src/`
//! rather than migrated to a `crates/` crate because the v2 Reborn stack uses
//! `brassclaw_host_api::UserId` and these types exist only to satisfy the
//! remaining v1-adjacent DB trait surface.

use std::fmt;

/// Role carried on every authenticated [`UserId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Owner,
    Admin,
    Regular,
}

impl UserRole {
    /// Parse a role string persisted in the users table.
    pub fn from_db_role(role: &str) -> Self {
        match role.trim().to_ascii_lowercase().as_str() {
            "owner" => Self::Owner,
            "admin" => Self::Admin,
            _ => Self::Regular,
        }
    }

    /// Returns `true` when the role has administrative privileges.
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin | Self::Owner)
    }

    /// Returns `true` for the deployment owner.
    pub fn is_owner(&self) -> bool {
        matches!(self, Self::Owner)
    }

    #[doc(hidden)]
    pub fn regular_default_if_missing() -> Self {
        Self::Regular
    }
}

/// Typed wrapper over `users.id` with its [`UserRole`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserId {
    id: String,
    #[serde(default = "UserRole::regular_default_if_missing")]
    role: UserRole,
}

impl PartialEq for UserId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for UserId {}

impl std::hash::Hash for UserId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl UserId {
    /// Opt-out for values sourced from a trusted upstream (DB row, registry entry, etc.).
    pub fn from_trusted(id: String, role: UserRole) -> Self {
        Self { id, role }
    }

    /// Borrow the raw user id string.
    pub fn as_str(&self) -> &str {
        &self.id
    }

    /// The attached role.
    pub fn role(&self) -> UserRole {
        self.role
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}
