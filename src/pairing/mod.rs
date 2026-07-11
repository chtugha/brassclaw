//! Pairing helpers used by the DB layer.

/// Normalise a channel name to its canonical lowercase form.
///
/// All channel-identity lookups and pairing-code operations must go through
/// this function so that `"Telegram"`, `"TELEGRAM"`, and `"telegram"` all
/// resolve to the same identity row.
pub fn normalize_channel_name(channel: &str) -> String {
    channel.to_ascii_lowercase()
}
