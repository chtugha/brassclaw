//! Cdylib Tool ABI — the wire contract between the Rust Executioner and a
//! dynamically-loaded cdylib Tool (Step C.3, "Two Tool Systems").
//!
//! This module is **contract only** (types + symbol names + serde shapes). The
//! `unsafe` FFI call that drives the ABI lives in the
//! [`brassclaw_host_runtime::dynamic_tool_loader`] service, not here.
//!
//! # ABI
//!
//! Every dynamic cdylib Tool exports two `extern "C"` symbols:
//!
//! - `brassclaw_tool_invoke(payload: *const c_char, payload_len: usize,
//!    out: *mut *mut c_char, out_len: *mut usize) -> i32`
//! - `brassclaw_tool_drop_out(buf: *mut c_char, len: usize)`
//!
//! The host serializes a [`CdylibRequest`] to JSON, passes the UTF-8 bytes as
//! `(payload, payload_len)` (the cdylib does NOT free the request buffer), and
//! receives the response as a host-foreign allocation in `(*out, *out_len)` plus
//! a return code (`0` = success, non-zero = failure). The host copies the bytes
//! out, deserializes a [`CdylibResponse`], then calls `brassclaw_tool_drop_out`
//! so the cdylib frees its own allocation. JSON-in/JSON-out keeps the ABI stable
//! across rustc versions and language-agnostic.

use std::os::raw::c_char;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Exported symbol a cdylib Tool must provide: the JSON-in/JSON-out invoker.
pub const CDYLIB_TOOL_INVOKE_SYMBOL: &str = "brassclaw_tool_invoke";

/// Exported symbol a cdylib Tool must provide: the destructor for the response
/// buffer it allocated in `brassclaw_tool_invoke`'s `out` parameter.
pub const CDYLIB_TOOL_DROP_OUT_SYMBOL: &str = "brassclaw_tool_drop_out";

/// The `extern "C" fn` pointer type for [`CDYLIB_TOOL_INVOKE_SYMBOL`].
///
/// `# Safety`: the caller must pass a valid UTF-8 JSON `payload` of `payload_len`
/// bytes and writable `out`/`out_len` pointers. The callee allocates `*out` and
/// owns it until [`CdylibToolDropOut`] is called.
pub type CdylibToolInvoke =
    unsafe extern "C" fn(*const c_char, usize, *mut *mut c_char, *mut usize) -> i32;

/// The `extern "C" fn` pointer type for [`CDYLIB_TOOL_DROP_OUT_SYMBOL`].
///
/// `# Safety`: `buf`/`len` must be the exact pair the invoker wrote to `*out`.
pub type CdylibToolDropOut = unsafe extern "C" fn(*mut c_char, usize);

/// Host→cdylib request: which tool to run and its kwargs as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdylibRequest {
    pub tool: String,
    pub args: Value,
}

/// cdylib→host response: either a JSON `result` (`ok == true`) or an `error`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CdylibResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Errors raised while driving the cdylib ABI (host side). The actual FFI call
/// is in `brassclaw_host_runtime`; this is the shared vocabulary.
#[derive(Debug, Error)]
pub enum CdylibAbiError {
    #[error("cdylib tool '{tool}' returned non-zero code {code}")]
    InvokeReturned { tool: String, code: i32 },
    #[error("cdylib tool '{tool}' response was not valid UTF-8: {reason}")]
    ResponseNotUtf8 { tool: String, reason: String },
    #[error("cdylib tool '{tool}' response JSON was invalid: {reason}")]
    ResponseInvalidJson { tool: String, reason: String },
    #[error("cdylib tool '{tool}' reported an error: {error}")]
    ToolError { tool: String, error: String },
}

impl CdylibResponse {
    /// Build a success response carrying a JSON `result`.
    pub fn ok(result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    /// Build a failure response carrying an error message.
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips() {
        let req = CdylibRequest {
            tool: "host.fixture_echo".to_string(),
            args: serde_json::json!({"x": 2, "name": "monty"}),
        };
        let json = serde_json::to_string(&req).expect("serialize request");
        let back: CdylibRequest = serde_json::from_str(&json).expect("deserialize request");
        assert_eq!(back.tool, req.tool);
        assert_eq!(back.args, req.args);
    }

    #[test]
    fn response_ok_round_trips_and_skips_none_fields() {
        let resp = CdylibResponse::ok(serde_json::json!({"echoed": true, "x": 2}));
        let json = serde_json::to_string(&resp).expect("serialize response");
        assert!(!json.contains("\"error\""), "error field must be absent: {json}");
        assert!(json.contains("\"result\""), "result field present: {json}");
        let back: CdylibResponse = serde_json::from_str(&json).expect("deserialize response");
        assert_eq!(back, resp);
    }

    #[test]
    fn response_err_round_trips() {
        let resp = CdylibResponse::err("kaboom");
        let json = serde_json::to_string(&resp).expect("serialize error response");
        assert!(json.contains("\"ok\":false"), "ok must be false: {json}");
        assert!(json.contains("\"error\":\"kaboom\""), "error message present: {json}");
        let back: CdylibResponse = serde_json::from_str(&json).expect("deserialize error response");
        assert_eq!(back, resp);
    }

    #[test]
    fn response_tolerates_missing_optional_fields() {
        let json = r#"{"ok":true}"#;
        let back: CdylibResponse = serde_json::from_str(json).expect("minimal response");
        assert!(back.ok);
        assert!(back.result.is_none());
        assert!(back.error.is_none());
    }

    #[test]
    fn invoke_error_display_carry_tool_and_code() {
        let e = CdylibAbiError::InvokeReturned {
            tool: "host.fixture_echo".to_string(),
            code: 7,
        };
        let s = e.to_string();
        assert!(s.contains("host.fixture_echo"), "tool name in error: {s}");
        assert!(s.contains("7"), "code in error: {s}");
    }
}
