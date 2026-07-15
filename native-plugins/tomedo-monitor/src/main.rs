//! Tomedo Monitor — macOS background daemon for BrassClaw.
//!
//! Monitors the active window of the Tomedo practice management client
//! and exposes the current patient ID via a local HTTP endpoint on
//! `http://127.0.0.1:49152/tomedo`.
//!
//! # Endpoints
//!
//! - `GET /tomedo` — JSON: `{"patient_id": "12345", "window_title": "...", "timestamp": "..."}` or `null` when no patient is active
//! - `GET /health` — `{"ok": true}`
//!
//! # Configuration
//!
//! Use `--port` to change the HTTP port (default 49152).
//! Use `--pattern` to set a regex that extracts the patient ID from the window
//! title. The regex must contain a named capture group `id`.
//! Default pattern: `(?i)(?:patient|pat\.?|patid|pid)[:\s#]+(?P<id>\d+)`

use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Parser;
use log::{debug, error, info, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tiny_http::{Response, Server};

#[cfg(target_os = "macos")]
mod macos;

#[cfg(not(target_os = "macos"))]
mod macos {
    pub fn get_tomedo_window_info() -> Option<WindowInfo> {
        None
    }

    #[allow(dead_code)]
    pub struct WindowInfo {
        pub title: String,
        pub pid: i32,
    }
}

#[derive(Parser, Debug)]
#[command(name = "tomedo-monitor", about = "BrassClaw Tomedo window monitor daemon")]
struct Args {
    #[arg(long, default_value = "49152", help = "Local HTTP port to listen on")]
    port: u16,

    #[arg(
        long,
        default_value = r"(?i)(?:patient(?:en)?(?:nummer|nr|id)?|pat(?:id)?|pid)[:\s#\-]+(?P<id>\d{3,10})",
        help = "Regex with named group 'id' to extract patient ID from window title"
    )]
    pattern: String,

    #[arg(
        long,
        default_value = "500",
        help = "Polling interval in milliseconds"
    )]
    interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivePatient {
    pub patient_id: String,
    pub window_title: String,
    pub timestamp_unix: u64,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    let pattern = match Regex::new(&args.pattern) {
        Ok(r) => r,
        Err(e) => {
            error!("Invalid --pattern regex: {}", e);
            std::process::exit(1);
        }
    };

    let state: Arc<Mutex<Option<ActivePatient>>> = Arc::new(Mutex::new(None));

    let state_monitor = Arc::clone(&state);
    let interval = Duration::from_millis(args.interval_ms);

    thread::spawn(move || {
        info!("Monitor thread started (interval: {}ms)", interval.as_millis());
        loop {
            let info = macos::get_tomedo_window_info();
            let new_patient = info.and_then(|w| extract_patient_id(&pattern, &w.title));

            {
                let mut guard = state_monitor.lock().unwrap();
                let changed = match (&*guard, &new_patient) {
                    (None, None) => false,
                    (Some(prev), Some(next)) => prev.patient_id != next.patient_id,
                    _ => true,
                };
                if changed {
                    if let Some(ref p) = new_patient {
                        info!("Active patient changed: {} ({})", p.patient_id, p.window_title);
                    } else {
                        info!("No active Tomedo patient window detected");
                    }
                }
                *guard = new_patient;
            }

            thread::sleep(interval);
        }
    });

    let addr = format!("127.0.0.1:{}", args.port);
    let server = match Server::http(&addr) {
        Ok(s) => {
            info!("HTTP server listening on http://{}", addr);
            s
        }
        Err(e) => {
            error!("Failed to start HTTP server on {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        debug!("Request: {} {}", request.method(), url);

        let response = match url.as_str() {
            "/health" | "/health/" => {
                let body = r#"{"ok":true}"#;
                Response::from_data(body.as_bytes().to_vec())
                    .with_header("Content-Type: application/json".parse().unwrap())
                    .with_status_code(200)
            }
            "/tomedo" | "/tomedo/" => {
                let guard = state.lock().unwrap();
                let body = match &*guard {
                    Some(p) => serde_json::to_string(p).unwrap_or_else(|_| "null".into()),
                    None => "null".into(),
                };
                drop(guard);
                Response::from_data(body.as_bytes().to_vec())
                    .with_header("Content-Type: application/json".parse().unwrap())
                    .with_status_code(200)
            }
            _ => {
                let body = r#"{"error":"not found"}"#;
                Response::from_data(body.as_bytes().to_vec())
                    .with_header("Content-Type: application/json".parse().unwrap())
                    .with_status_code(404)
            }
        };

        if let Err(e) = request.respond(response) {
            warn!("Failed to send response: {}", e);
        }
    }
}

fn extract_patient_id(pattern: &Regex, window_title: &str) -> Option<ActivePatient> {
    let caps = pattern.captures(window_title)?;
    let patient_id = caps.name("id").map(|m| m.as_str().to_string())?;

    let timestamp_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Some(ActivePatient {
        patient_id,
        window_title: window_title.to_string(),
        timestamp_unix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_pattern() -> Regex {
        Regex::new(r"(?i)(?:patient(?:en)?(?:nummer|nr|id)?|pat(?:id)?|pid)[:\s#\-]+(?P<id>\d{3,10})")
            .unwrap()
    }

    #[test]
    fn test_extract_standard_title() {
        let pat = default_pattern();
        let result = extract_patient_id(&pat, "Tomedo - Patient: 12345 - Max Mustermann");
        assert!(result.is_some());
        assert_eq!(result.unwrap().patient_id, "12345");
    }

    #[test]
    fn test_extract_pid_format() {
        let pat = default_pattern();
        let result = extract_patient_id(&pat, "tomedo [pid:98765]");
        assert!(result.is_some());
        assert_eq!(result.unwrap().patient_id, "98765");
    }

    #[test]
    fn test_no_match_on_non_patient_window() {
        let pat = default_pattern();
        let result = extract_patient_id(&pat, "Tomedo - Stammdaten");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_german_patientennummer() {
        let pat = default_pattern();
        let result = extract_patient_id(&pat, "Patientennr: 54321 | Mustermann, Max");
        assert!(result.is_some());
        assert_eq!(result.unwrap().patient_id, "54321");
    }
}
