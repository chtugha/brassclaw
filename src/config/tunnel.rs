use crate::config::helpers::db_first_optional_string;
use crate::error::ConfigError;
use crate::settings::Settings;

/// Tunnel configuration for exposing the agent to the internet.
///
/// Used by channels and tools that need public webhook endpoints.
/// The tunnel URL is shared across all channels (Telegram, Slack, etc.).
///
/// Resolution priority: DB/settings > env var > default.
///
/// Two modes:
/// - **Static URL** (`TUNNEL_URL`): set the public URL directly (manual tunnel)
/// - **Managed provider** (`TUNNEL_PROVIDER`): lifecycle-managed tunnel process
///
/// When a managed provider is configured _and_ no static URL is set,
/// the gateway starts the tunnel on boot and populates `public_url`.
#[derive(Debug, Clone, Default)]
pub struct TunnelConfig {
    /// Public URL from tunnel provider (e.g., "https://abc123.ngrok.io").
    /// Set statically via `TUNNEL_URL` or populated at runtime by a managed tunnel.
    pub public_url: Option<String>,
    /// Provider name for lifecycle-managed tunnels (e.g. "ngrok", "cloudflare").
    /// The v1 tunnel lifecycle implementation has been removed; this field is
    /// preserved so settings round-trips don't lose the configured provider name.
    pub provider: Option<String>,
}

impl TunnelConfig {
    pub(crate) fn resolve(settings: &Settings) -> Result<Self, ConfigError> {
        let public_url = db_first_optional_string(&settings.tunnel.public_url, "TUNNEL_URL")?;

        if let Some(ref url) = public_url
            && !url.starts_with("https://")
        {
            return Err(ConfigError::InvalidValue {
                key: "TUNNEL_URL".to_string(),
                message: "must start with https:// (webhooks require HTTPS)".to_string(),
            });
        }

        let provider = db_first_optional_string(&settings.tunnel.provider, "TUNNEL_PROVIDER")?
            .filter(|p| !p.is_empty() && p != "none");

        Ok(Self {
            public_url,
            provider,
        })
    }

    /// Check if a tunnel is configured (static URL or managed provider).
    pub fn is_enabled(&self) -> bool {
        self.public_url.is_some() || self.provider.is_some()
    }

    /// Get the webhook URL for a given path.
    pub fn webhook_url(&self, path: &str) -> Option<String> {
        self.public_url.as_ref().map(|base| {
            let base = base.trim_end_matches('/');
            let path = path.trim_start_matches('/');
            format!("{}/{}", base, path)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::config::tunnel::TunnelConfig;

    // ── Default ─────────────────────────────────────────────────────

    #[test]
    fn default_is_disabled() {
        let cfg = TunnelConfig::default();
        assert!(cfg.public_url.is_none());
        assert!(cfg.provider.is_none());
        assert!(!cfg.is_enabled());
    }

    // ── is_enabled ──────────────────────────────────────────────────

    #[test]
    fn is_enabled_with_static_url() {
        let cfg = TunnelConfig {
            public_url: Some("https://tunnel.example.com".to_string()),
            provider: None,
        };
        assert!(cfg.is_enabled());
    }

    #[test]
    fn is_enabled_with_provider() {
        let cfg = TunnelConfig {
            public_url: None,
            provider: Some("cloudflare".to_string()),
        };
        assert!(cfg.is_enabled());
    }

    #[test]
    fn is_enabled_with_both() {
        let cfg = TunnelConfig {
            public_url: Some("https://example.com".to_string()),
            provider: Some("ngrok".to_string()),
        };
        assert!(cfg.is_enabled());
    }

    // ── webhook_url ─────────────────────────────────────────────────

    #[test]
    fn webhook_url_none_when_no_public_url() {
        let cfg = TunnelConfig::default();
        assert!(cfg.webhook_url("/hook").is_none());
    }

    #[test]
    fn webhook_url_basic() {
        let cfg = TunnelConfig {
            public_url: Some("https://abc.ngrok.io".to_string()),
            provider: None,
        };
        assert_eq!(
            cfg.webhook_url("/webhook/telegram"),
            Some("https://abc.ngrok.io/webhook/telegram".to_string())
        );
    }

    #[test]
    fn webhook_url_trims_trailing_slash_on_base() {
        let cfg = TunnelConfig {
            public_url: Some("https://abc.ngrok.io/".to_string()),
            provider: None,
        };
        assert_eq!(
            cfg.webhook_url("/hook"),
            Some("https://abc.ngrok.io/hook".to_string())
        );
    }

    #[test]
    fn webhook_url_trims_leading_slash_on_path() {
        let cfg = TunnelConfig {
            public_url: Some("https://abc.ngrok.io".to_string()),
            provider: None,
        };
        // Path without leading slash should also work
        assert_eq!(
            cfg.webhook_url("hook"),
            Some("https://abc.ngrok.io/hook".to_string())
        );
    }

    #[test]
    fn webhook_url_double_slash_normalization() {
        let cfg = TunnelConfig {
            public_url: Some("https://abc.ngrok.io/".to_string()),
            provider: None,
        };
        // Both base trailing and path leading slashes trimmed
        assert_eq!(
            cfg.webhook_url("/api/webhook"),
            Some("https://abc.ngrok.io/api/webhook".to_string())
        );
    }

    #[test]
    fn webhook_url_empty_path() {
        let cfg = TunnelConfig {
            public_url: Some("https://abc.ngrok.io".to_string()),
            provider: None,
        };
        assert_eq!(
            cfg.webhook_url(""),
            Some("https://abc.ngrok.io/".to_string())
        );
    }
}
