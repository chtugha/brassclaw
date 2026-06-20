use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, reload};

use brassclaw_common::{AppEvent, DynEventPublisher};
use brassclaw_safety::LeakDetector;

const HISTORY_CAP: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub level: String,
    pub target: String,
    pub message: String,
    pub timestamp: String,
}

pub struct LogBroadcaster {
    tx: broadcast::Sender<LogEntry>,
    recent: Mutex<VecDeque<LogEntry>>,
    leak_detector: LeakDetector,
}

impl LogBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(512);
        Self {
            tx,
            recent: Mutex::new(VecDeque::with_capacity(HISTORY_CAP)),
            leak_detector: LeakDetector::new(),
        }
    }

    pub fn send(&self, mut entry: LogEntry) {
        entry.message = self
            .leak_detector
            .scan_and_clean(&entry.message)
            .unwrap_or_else(|_| "[log message redacted: contained blocked secret]".to_string());

        if let Ok(mut buf) = self.recent.lock() {
            if buf.len() >= HISTORY_CAP {
                buf.pop_front();
            }
            buf.push_back(entry.clone());
        }
        let _ = self.tx.send(entry);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.tx.subscribe()
    }

    pub fn recent_entries(&self) -> Vec<LogEntry> {
        self.recent
            .lock()
            .map(|buf| buf.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for LogBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LogLevelHandle {
    handle: reload::Handle<EnvFilter, tracing_subscriber::Registry>,
    current_level: Mutex<String>,
    base_filter: String,
}

impl LogLevelHandle {
    pub fn new(
        handle: reload::Handle<EnvFilter, tracing_subscriber::Registry>,
        initial_level: String,
        base_filter: String,
    ) -> Self {
        Self {
            handle,
            current_level: Mutex::new(initial_level),
            base_filter,
        }
    }

    pub fn set_level(&self, level: &str) -> Result<(), String> {
        const VALID: &[&str] = &["trace", "debug", "info", "warn", "error"];
        let level = level.to_lowercase();
        if !VALID.contains(&level.as_str()) {
            return Err(format!(
                "invalid level '{}', must be one of: {}",
                level,
                VALID.join(", ")
            ));
        }

        let filter_str = if self.base_filter.is_empty() {
            format!("brassclaw={}", level)
        } else {
            format!("brassclaw={},{}", level, self.base_filter)
        };

        let new_filter = EnvFilter::new(&filter_str);
        self.handle
            .reload(new_filter)
            .map_err(|e| format!("failed to reload filter: {}", e))?;

        if let Ok(mut current) = self.current_level.lock() {
            *current = level;
        }
        Ok(())
    }

    pub fn current_level(&self) -> String {
        self.current_level
            .lock()
            .map(|l| l.clone())
            .unwrap_or_else(|_| "info".to_string())
    }
}

pub fn init_tracing(
    log_broadcaster: Arc<LogBroadcaster>,
    suppress_stderr: bool,
) -> Arc<LogLevelHandle> {
    let raw_filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "brassclaw=info,tower_http=warn".to_string());

    let mut brassclaw_level = String::from("info");
    let mut base_parts: Vec<&str> = Vec::new();

    for part in raw_filter.split(',') {
        let trimmed = part.trim();
        if trimmed.starts_with("brassclaw=") {
            if let Some(lvl) = trimmed.strip_prefix("brassclaw=") {
                brassclaw_level = lvl.to_string();
            }
        } else if !trimmed.is_empty() {
            base_parts.push(trimmed);
        }
    }
    let base_filter = base_parts.join(",");

    let env_filter = EnvFilter::new(&raw_filter);
    let (reload_layer, reload_handle) = reload::Layer::new(env_filter);

    let handle = Arc::new(LogLevelHandle::new(
        reload_handle,
        brassclaw_level,
        base_filter,
    ));

    let fmt_layer = if suppress_stderr {
        None
    } else {
        Some(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_writer(crate::tracing_fmt::TruncatingStderr::default()),
        )
    };

    tracing_subscriber::registry()
        .with(reload_layer)
        .with(fmt_layer)
        .with(WebLogLayer::new(log_broadcaster))
        .init();

    handle
}

pub fn spawn_warning_bridge(
    broadcaster: Arc<LogBroadcaster>,
    event_publisher: DynEventPublisher,
    owner_id: Option<String>,
) {
    let mut rx = broadcaster.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(entry) => {
                    if entry.level != "WARN" && entry.level != "ERROR" {
                        continue;
                    }
                    if !event_publisher.has_verbose_receivers() {
                        continue;
                    }
                    let event = AppEvent::Warning {
                        source: entry.target,
                        message: entry.message,
                        thread_id: None,
                    };
                    match &owner_id {
                        Some(uid) => event_publisher.broadcast_for_user(uid, event),
                        None => event_publisher.broadcast(event),
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

struct MessageVisitor {
    message: String,
    fields: Vec<String>,
}

impl MessageVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
            fields: Vec::new(),
        }
    }

    fn finish(self) -> String {
        if self.fields.is_empty() {
            self.message
        } else {
            format!("{} {}", self.message, self.fields.join(" "))
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
            if self.message.starts_with('"') && self.message.ends_with('"') {
                self.message = self.message[1..self.message.len() - 1].to_string();
            }
        } else {
            self.fields.push(format!("{}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.push(format!("{}={}", field.name(), value));
        }
    }
}

pub struct WebLogLayer {
    broadcaster: Arc<LogBroadcaster>,
}

impl WebLogLayer {
    pub fn new(broadcaster: Arc<LogBroadcaster>) -> Self {
        Self { broadcaster }
    }
}

impl<S: tracing::Subscriber> Layer<S> for WebLogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();

        if *metadata.level() > tracing::Level::DEBUG {
            return;
        }

        let mut visitor = MessageVisitor::new();
        event.record(&mut visitor);

        let entry = LogEntry {
            level: metadata.level().to_string().to_uppercase(),
            target: metadata.target().to_string(),
            message: visitor.finish(),
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        };

        self.broadcaster.send(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_broadcaster_creation() {
        let broadcaster = LogBroadcaster::new();
        broadcaster.send(LogEntry {
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "hello".to_string(),
            timestamp: "2024-01-01T00:00:00.000Z".to_string(),
        });
    }

    #[test]
    fn test_log_broadcaster_subscribe() {
        let broadcaster = LogBroadcaster::new();
        let mut rx = broadcaster.subscribe();

        broadcaster.send(LogEntry {
            level: "WARN".to_string(),
            target: "brassclaw::test".to_string(),
            message: "test warning".to_string(),
            timestamp: "2024-01-01T00:00:00.000Z".to_string(),
        });

        let entry = rx.try_recv().expect("should receive entry");
        assert_eq!(entry.level, "WARN");
        assert_eq!(entry.message, "test warning");
    }

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry {
            level: "ERROR".to_string(),
            target: "brassclaw::agent".to_string(),
            message: "something broke".to_string(),
            timestamp: "2024-01-01T00:00:00.000Z".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("\"level\":\"ERROR\""));
        assert!(json.contains("something broke"));
    }

    #[test]
    fn test_recent_entries_buffer() {
        let broadcaster = LogBroadcaster::new();

        for i in 0..5 {
            broadcaster.send(LogEntry {
                level: "INFO".to_string(),
                target: "test".to_string(),
                message: format!("msg {}", i),
                timestamp: "2024-01-01T00:00:00.000Z".to_string(),
            });
        }

        let recent = broadcaster.recent_entries();
        assert_eq!(recent.len(), 5);
        assert_eq!(recent[0].message, "msg 0");
        assert_eq!(recent[4].message, "msg 4");
    }

    #[test]
    fn test_recent_entries_cap() {
        let broadcaster = LogBroadcaster::new();

        for i in 0..(HISTORY_CAP + 50) {
            broadcaster.send(LogEntry {
                level: "INFO".to_string(),
                target: "test".to_string(),
                message: format!("msg {}", i),
                timestamp: "2024-01-01T00:00:00.000Z".to_string(),
            });
        }

        let recent = broadcaster.recent_entries();
        assert_eq!(recent.len(), HISTORY_CAP);
        assert_eq!(recent[0].message, "msg 50");
    }

    #[test]
    fn test_recent_entries_available_without_subscribers() {
        let broadcaster = LogBroadcaster::new();
        broadcaster.send(LogEntry {
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "before anyone listened".to_string(),
            timestamp: "2024-01-01T00:00:00.000Z".to_string(),
        });

        let recent = broadcaster.recent_entries();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "before anyone listened");
    }

    #[test]
    fn test_message_visitor_finish_message_only() {
        let v = MessageVisitor {
            message: "hello world".to_string(),
            fields: vec![],
        };
        assert_eq!(v.finish(), "hello world");
    }

    #[test]
    fn test_message_visitor_finish_with_fields() {
        let v = MessageVisitor {
            message: "Request completed".to_string(),
            fields: vec![
                "url=http://localhost:8080".to_string(),
                "status=200".to_string(),
            ],
        };
        let result = v.finish();
        assert_eq!(
            result,
            "Request completed url=http://localhost:8080 status=200"
        );
    }

    #[test]
    fn test_message_visitor_finish_empty() {
        let v = MessageVisitor::new();
        assert_eq!(v.finish(), "");
    }

    #[test]
    fn test_broadcaster_has_leak_detector() {
        let broadcaster = LogBroadcaster::new();
        assert!(broadcaster.leak_detector.pattern_count() > 0);
    }

    #[test]
    fn test_leak_detector_scrubs_api_key_in_log() {
        let detector = brassclaw_safety::LeakDetector::new();
        let msg = "Connecting with token sk-proj-test1234567890abcdefghij";
        let result = detector.scan_and_clean(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_leak_detector_passes_clean_log() {
        let detector = brassclaw_safety::LeakDetector::new();
        let msg = "Request completed status=200 url=https://api.example.com/data";
        let result = detector.scan_and_clean(msg);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), msg);
    }
}
