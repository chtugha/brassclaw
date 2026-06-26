//! Sandboxed filesystem daemon.
//!
//! Reads NDJSON requests from stdin, dispatches file-system and shell tools
//! constrained to `BRASSCLAW_SANDBOX_BASE_DIR`, and writes NDJSON responses
//! to stdout.  One response line is emitted per request line.  The daemon
//! exits cleanly when it receives a `shutdown` request or stdin reaches EOF.
//!
//! ## Protocol
//!
//! Requests:
//! ```json
//! {"id":"<string>","method":"<method>","params":{...}}
//! ```
//!
//! Responses:
//! ```json
//! {"id":"<string>","result":{...}}
//! {"id":"<string>","error":{"code":"<code>","message":"<msg>"}}
//! ```
//!
//! ## Methods
//! - `health`        — returns `{"status":"ok","tools":[...]}`
//! - `execute_tool`  — `params.name` + `params.input`
//! - `shutdown`      — graceful exit

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use brassclaw::bridge::sandbox::protocol::{Request, Response, RpcError, SUPPORTED_TOOLS};
use brassclaw::capabilities::filesystem::{
    FilesystemCapabilityState, FilesystemContext, execute_apply_patch, execute_file_undo,
    execute_glob, execute_grep, execute_list_dir, execute_read_file, execute_write_file,
};

fn respond(id: Option<&str>, result: Option<Value>, error: Option<RpcError>) {
    let resp = Response {
        id: id.map(str::to_owned),
        result,
        error,
    };
    let line = serde_json::to_string(&resp).expect("serialize response");
    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(line.as_bytes()).expect("write stdout");
    out.write_all(b"\n").expect("write newline");
    out.flush().expect("flush stdout");
}

fn tool_error(id: &str, msg: impl std::fmt::Display) {
    respond(
        Some(id),
        None,
        Some(RpcError::new("tool_error", msg.to_string())),
    );
}

fn base_dir() -> PathBuf {
    std::env::var("BRASSCLAW_SANDBOX_BASE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("cwd"))
}

fn make_fs_ctx(base: &Path) -> FilesystemContext {
    FilesystemContext {
        base_dir: base.to_path_buf(),
        state: Arc::new(FilesystemCapabilityState::new()),
    }
}

#[tokio::main]
async fn main() {
    let base = base_dir();
    let ctx = make_fs_ctx(&base);

    let stdin = io::stdin();
    for raw in stdin.lock().lines() {
        let raw = match raw {
            Ok(line) => line,
            Err(_) => break,
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: Request = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                respond(
                    None,
                    None,
                    Some(RpcError::new("parse_error", format!("invalid JSON: {e}"))),
                );
                continue;
            }
        };

        match req.method.as_str() {
            "health" => {
                let tools: Vec<&str> = SUPPORTED_TOOLS
                    .iter()
                    .copied()
                    .filter(|t| !t.starts_with("read_") && !t.starts_with("write_"))
                    .collect();
                respond(
                    Some(&req.id),
                    Some(json!({ "status": "ok", "tools": tools })),
                    None,
                );
            }

            "shutdown" => {
                respond(Some(&req.id), Some(json!({ "status": "ok" })), None);
                break;
            }

            "execute_tool" => {
                let name = req.params.get("name").and_then(Value::as_str);
                let input = req.params.get("input").unwrap_or(&Value::Null);

                let name = match name {
                    Some(n) => n,
                    None => {
                        tool_error(&req.id, "missing params.name");
                        continue;
                    }
                };

                // Dispatch to the appropriate capability execute function.
                let result = match name {
                    "file_read" | "read_file" => execute_read_file(input, &ctx).await,
                    "file_write" | "write_file" => execute_write_file(input, &ctx).await,
                    "list_dir" => execute_list_dir(input, &ctx).await,
                    "apply_patch" => execute_apply_patch(input, &ctx).await,
                    "glob" => execute_glob(input, &ctx).await,
                    "grep" => execute_grep(input, &ctx).await,
                    "file_undo" => execute_file_undo(input, &ctx).await,
                    "shell" => {
                        // Shell is not implemented in this daemon;
                        // it requires the container runtime.
                        respond(
                            Some(&req.id),
                            None,
                            Some(RpcError::new(
                                "tool_error",
                                "shell tool is not available in the filesystem-only daemon",
                            )),
                        );
                        continue;
                    }
                    other => {
                        respond(
                            Some(&req.id),
                            None,
                            Some(RpcError::new(
                                "tool_error",
                                format!("unknown tool: '{other}'"),
                            )),
                        );
                        continue;
                    }
                };

                match result {
                    Ok(output) => {
                        respond(
                            Some(&req.id),
                            Some(json!({ "output": output })),
                            None,
                        );
                    }
                    Err(e) => {
                        tool_error(&req.id, e);
                    }
                }
            }

            other => {
                respond(
                    Some(&req.id),
                    None,
                    Some(RpcError::new(
                        "unknown_method",
                        format!("unknown method: '{other}'"),
                    )),
                );
            }
        }
    }
}
