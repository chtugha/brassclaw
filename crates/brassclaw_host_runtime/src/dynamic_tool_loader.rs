//! Dynamic cdylib Tool loader — the Executioner's second Tool System (Step C.3).
//!
//! Built-in Tools are precompiled into the binary and dispatched by the engine
//! orchestrator's static `match call.function_name`. Kohai/Sempai-minted
//! Tools+ToolSkills ship as separate `cdylib` crates and are dlopen'd here at
//! runtime on demand, bound into the `host` namespace by a recipe, and unloaded
//! at main-process task end. Only Q2+ validated dynamic tools are runnable, so
//! every loaded artifact is trusted (Matching-Mode security-off) — there is no
//! sidecar/sandbox branch. The JSON `extern "C"` ABI contract lives in
//! [`brassclaw_host_api::cdylib_abi`].
//!
//! The Executioner runs one turn at a time, so the loaded-tool map needs no
//! lock: `&mut self` for load/unload, `&self` for invoke/is_loaded.

use std::collections::HashMap;
use std::os::raw::c_char;
use std::path::PathBuf;

use brassclaw_host_api::cdylib_abi::{
    CDYLIB_TOOL_DROP_OUT_SYMBOL, CDYLIB_TOOL_INVOKE_SYMBOL, CdylibAbiError, CdylibRequest,
    CdylibResponse, CdylibToolDropOut, CdylibToolInvoke,
};
use serde_json::Value;
use thiserror::Error;

/// A cdylib artifact to load into the `host` namespace under `tool_name`.
///
/// The composition-mechanism (the `host.compose_orchestrator` rewrite, C.5/C.6)
/// builds these from a matched component's "rust part" and hands them to the
/// loader; the engine orchestrator's dispatch fallthrough then routes
/// `host.<tool_name>` calls here.
#[derive(Debug, Clone)]
pub struct CdylibLoadDirective {
    pub tool_name: String,
    pub artifact_path: PathBuf,
}

impl CdylibLoadDirective {
    pub fn new(tool_name: impl Into<String>, artifact_path: impl Into<PathBuf>) -> Self {
        Self {
            tool_name: tool_name.into(),
            artifact_path: artifact_path.into(),
        }
    }
}

/// Errors raised by [`DynamicToolLoader`] operations (host-side: load, bind,
/// serialize, invoke). The cdylib-side response failures are wrapped from
/// [`CdylibAbiError`].
#[derive(Debug, Error)]
pub enum DynamicToolLoaderError {
    #[error("failed to load cdylib tool '{tool}' from {path}: {reason}")]
    Load {
        tool: String,
        path: String,
        reason: String,
    },
    #[error("cdylib tool '{tool}' is missing required symbol '{symbol}': {reason}")]
    SymbolNotFound {
        tool: String,
        symbol: String,
        reason: String,
    },
    #[error("cdylib tool '{tool}' is not loaded")]
    NotLoaded { tool: String },
    #[error("failed to serialize request for cdylib tool '{tool}': {reason}")]
    RequestSerialization { tool: String, reason: String },
    #[error(transparent)]
    Invoke(#[from] CdylibAbiError),
}

/// A dlopen'd cdylib Tool: the open library + the two bound ABI fn pointers.
struct LoadedTool {
    /// Held to keep the dlopen handle open for the bound fn pointers' lifetime;
    /// never read directly. Dropping it closes the handle (unloads the cdylib).
    #[allow(dead_code)]
    library: libloading::Library,
    invoke_fn: CdylibToolInvoke,
    drop_fn: CdylibToolDropOut,
}

/// The Executioner's dynamic-Tool registry. Owns the dlopen/bind/invoke/unload
/// mechanics for kohai/sempai-minted cdylib Tools.
#[derive(Default)]
pub struct DynamicToolLoader {
    loaded: HashMap<String, LoadedTool>,
}

impl DynamicToolLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// dlopen a cdylib and bind its ABI symbols into the `host` namespace under
    /// `tool_name`. Replaces any existing binding for the same name.
    pub fn load(&mut self, directive: CdylibLoadDirective) -> Result<(), DynamicToolLoaderError> {
        let path_string = directive.artifact_path.display().to_string();
        // SAFETY: dlopen executes the library's init code; the artifact is a Q2+
        // validated cdylib Tool from the composition-mechanism (trusted path).
        let library = unsafe { libloading::Library::new(&directive.artifact_path) }.map_err(|e| {
            DynamicToolLoaderError::Load {
                tool: directive.tool_name.clone(),
                path: path_string.clone(),
                reason: e.to_string(),
            }
        })?;

        // SAFETY: `library` is a freshly-opened handle; the symbol names are the
        // exact exported strings and the fn-pointer types match the cdylib_abi
        // contract. The `Symbol` is dropped here after copying the fn pointer out.
        let invoke_fn = unsafe {
            Self::resolve_symbol::<CdylibToolInvoke>(&library, CDYLIB_TOOL_INVOKE_SYMBOL.as_bytes())
        }
        .map_err(|reason| DynamicToolLoaderError::SymbolNotFound {
            tool: directive.tool_name.clone(),
            symbol: CDYLIB_TOOL_INVOKE_SYMBOL.to_string(),
            reason,
        })?;
        let drop_fn = unsafe {
            Self::resolve_symbol::<CdylibToolDropOut>(
                &library,
                CDYLIB_TOOL_DROP_OUT_SYMBOL.as_bytes(),
            )
        }
        .map_err(|reason| DynamicToolLoaderError::SymbolNotFound {
            tool: directive.tool_name.clone(),
            symbol: CDYLIB_TOOL_DROP_OUT_SYMBOL.to_string(),
            reason,
        })?;

        self.loaded.insert(
            directive.tool_name.clone(),
            LoadedTool {
                library,
                invoke_fn,
                drop_fn,
            },
        );
        Ok(())
    }

    /// Accept a batch of load directives — the "rust part" hand-off from the
    /// composition-mechanism. Loads each in order; stops at the first failure.
    pub fn load_directives(
        &mut self,
        directives: Vec<CdylibLoadDirective>,
    ) -> Result<(), DynamicToolLoaderError> {
        for directive in directives {
            self.load(directive)?;
        }
        Ok(())
    }

    /// Invoke a loaded cdylib Tool by name with JSON `args`; returns the JSON
    /// `result` the cdylib produced.
    pub fn invoke(&self, tool_name: &str, args: Value) -> Result<Value, DynamicToolLoaderError> {
        let loaded = self
            .loaded
            .get(tool_name)
            .ok_or_else(|| DynamicToolLoaderError::NotLoaded {
                tool: tool_name.to_string(),
            })?;
        let req = CdylibRequest {
            tool: tool_name.to_string(),
            args,
        };
        let payload = serde_json::to_vec(&req).map_err(|e| DynamicToolLoaderError::RequestSerialization {
            tool: tool_name.to_string(),
            reason: e.to_string(),
        })?;
        // SAFETY: `invoke_fn`/`drop_fn` were bound from a currently-loaded cdylib
        // honoring the cdylib_abi contract; `payload` is valid UTF-8 JSON bytes.
        let result = unsafe { invoke_via_abi(loaded.invoke_fn, loaded.drop_fn, &payload, tool_name) }?;
        Ok(result)
    }

    /// Unload a single cdylib Tool (drops the `Library`, closing the handle).
    pub fn unload(&mut self, tool_name: &str) -> Result<(), DynamicToolLoaderError> {
        if self.loaded.remove(tool_name).is_some() {
            Ok(())
        } else {
            Err(DynamicToolLoaderError::NotLoaded {
                tool: tool_name.to_string(),
            })
        }
    }

    /// Unload every loaded cdylib Tool. Called at main-process task end.
    pub fn unload_all(&mut self) {
        self.loaded.clear();
    }

    /// Whether a cdylib Tool is currently loaded under `tool_name`. The engine
    /// orchestrator's dispatch fallthrough consults this before routing a
    /// `host.<name>` call to the loader.
    pub fn is_loaded(&self, tool_name: &str) -> bool {
        self.loaded.contains_key(tool_name)
    }

    /// Number of currently loaded cdylib Tools.
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    /// Resolve an `extern "C"` symbol from a freshly-opened library as a copied
    /// fn pointer.
    ///
    /// # Safety
    /// `library` must be a valid dlopen'd handle, `symbol` the exact exported
    /// name with no NUL bytes, and `T` must match the symbol's ABI signature.
    unsafe fn resolve_symbol<T: Copy>(
        library: &libloading::Library,
        symbol: &[u8],
    ) -> Result<T, String> {
        // SAFETY: covered by the fn-level safety contract.
        let sym: libloading::Symbol<T> = unsafe { library.get(symbol) }.map_err(|e| e.to_string())?;
        Ok(*sym)
    }
}

/// Drive the JSON-in/JSON-out ABI: call the cdylib, copy the response bytes,
/// free the cdylib's buffer via its destructor, deserialize the response.
///
/// # Safety
/// `invoke_fn`/`drop_fn` must be valid fn pointers exported by a currently-
/// loaded cdylib honoring the `cdylib_abi` contract, and `payload` must be
/// valid UTF-8 JSON of exactly `payload.len()` bytes.
unsafe fn invoke_via_abi(
    invoke_fn: CdylibToolInvoke,
    drop_fn: CdylibToolDropOut,
    payload: &[u8],
    tool: &str,
) -> Result<Value, CdylibAbiError> {
    let mut out: *mut c_char = std::ptr::null_mut();
    let mut out_len: usize = 0;

    // SAFETY: covered by the fn-level safety contract.
    let code = unsafe {
        invoke_fn(
            payload.as_ptr() as *const c_char,
            payload.len(),
            &mut out,
            &mut out_len,
        )
    };
    if code != 0 {
        if !out.is_null() {
            // SAFETY: `out`/`out_len` were just written by the cdylib (or null).
            unsafe { drop_fn(out, out_len) };
        }
        return Err(CdylibAbiError::InvokeReturned {
            tool: tool.to_string(),
            code,
        });
    }
    if out.is_null() {
        return Err(CdylibAbiError::ResponseInvalidJson {
            tool: tool.to_string(),
            reason: "null response buffer".to_string(),
        });
    }

    // SAFETY: `out` is non-null with `out_len` bytes the cdylib allocated.
    let bytes = unsafe { std::slice::from_raw_parts(out as *const u8, out_len) };
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => {
            // SAFETY: free the cdylib's buffer before propagating.
            unsafe { drop_fn(out, out_len) };
            return Err(CdylibAbiError::ResponseNotUtf8 {
                tool: tool.to_string(),
                reason: e.to_string(),
            });
        }
    };
    let response: CdylibResponse = match serde_json::from_str(text) {
        Ok(response) => response,
        Err(e) => {
            // SAFETY: free the cdylib's buffer before propagating.
            unsafe { drop_fn(out, out_len) };
            return Err(CdylibAbiError::ResponseInvalidJson {
                tool: tool.to_string(),
                reason: e.to_string(),
            });
        }
    };
    // SAFETY: done with the cdylib buffer — hand it back to the cdylib to free.
    unsafe { drop_fn(out, out_len) };

    if !response.ok {
        return Err(CdylibAbiError::ToolError {
            tool: tool.to_string(),
            error: response
                .error
                .unwrap_or_else(|| "unknown cdylib tool error".to_string()),
        });
    }
    response.result.ok_or_else(|| CdylibAbiError::ResponseInvalidJson {
        tool: tool.to_string(),
        reason: "ok=true but result missing".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// A self-contained cdylib fixture: std-only (no serde_json dep), exports the
    /// two `brassclaw_*` ABI symbols, and echoes the request by embedding the raw
    /// request JSON as the `"request"` value of a `CdylibResponse::ok` payload.
    /// Compiled in a tempdir by `rustc --crate-type cdylib` so the loader test
    /// exercises the real dlopen/bind/invoke/unload path.
    const FIXTURE_SOURCE: &str = r##"
use std::alloc::{Layout, alloc, dealloc};
use std::os::raw::c_char;
use std::ptr::copy_nonoverlapping;
use std::str::from_utf8;

#[no_mangle]
pub extern "C" fn brassclaw_tool_invoke(
    payload: *const c_char,
    payload_len: usize,
    out: *mut *mut c_char,
    out_len: *mut usize,
) -> i32 {
    if payload.is_null() || out.is_null() || out_len.is_null() {
        return 1;
    }
    let req_bytes = unsafe { std::slice::from_raw_parts(payload as *const u8, payload_len) };
    let req_str = match from_utf8(req_bytes) {
        Ok(s) => s,
        Err(_) => return 2,
    };
    let response = format!("{{\"ok\":true,\"result\":{{\"echoed\":true,\"request\":{}}}}}", req_str);
    let resp_bytes = response.into_bytes();
    let len = resp_bytes.len();
    let layout = match Layout::from_size_align(len, 1) {
        Ok(l) => l,
        Err(_) => return 3,
    };
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        return 4;
    }
    unsafe { copy_nonoverlapping(resp_bytes.as_ptr(), ptr, len) };
    unsafe {
        *out = ptr as *mut c_char;
        *out_len = len;
    }
    0
}

#[no_mangle]
pub extern "C" fn brassclaw_tool_drop_out(buf: *mut c_char, len: usize) {
    if buf.is_null() {
        return;
    }
    let layout = match Layout::from_size_align(len, 1) {
        Ok(l) => l,
        Err(_) => return,
    };
    unsafe { dealloc(buf as *mut u8, layout) };
}
"##;

    /// Compile the fixture cdylib into a fresh tempdir. Returns the tempdir
    /// (kept alive by the caller so the `.dylib`/`.so`/`.dll` survives the
    /// dlopen) and the artifact path, or `None` to skip the test when `rustc`
    /// is unavailable or the compile fails.
    fn compile_fixture_cdylib() -> Option<(tempfile::TempDir, PathBuf)> {
        let dir = tempfile::tempdir().ok()?;
        let src = dir.path().join("fixture_echo.rs");
        std::fs::write(&src, FIXTURE_SOURCE).ok()?;

        let ext = if cfg!(target_os = "macos") {
            "dylib"
        } else if cfg!(target_os = "windows") {
            "dll"
        } else {
            "so"
        };
        let out = dir.path().join(format!("fixture_echo.{ext}"));

        let (src_str, out_str) = (src.to_str()?, out.to_str()?);
        let output = Command::new("rustc")
            .args([
                "--edition",
                "2021",
                "--crate-type",
                "cdylib",
                "-o",
                out_str,
                src_str,
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            eprintln!(
                "skipping cdylib loader tests: rustc fixture compile failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return None;
        }
        Some((dir, out))
    }

    #[test]
    fn load_invoke_unload_round_trip() {
        let Some((_dir, fixture)) = compile_fixture_cdylib() else {
            eprintln!("skipping load_invoke_unload_round_trip: fixture cdylib unavailable");
            return;
        };
        let mut loader = DynamicToolLoader::new();
        assert!(!loader.is_loaded("host.fixture_echo"));
        assert_eq!(loader.loaded_count(), 0);

        loader
            .load(CdylibLoadDirective::new("host.fixture_echo", &fixture))
            .expect("load fixture cdylib");
        assert!(loader.is_loaded("host.fixture_echo"));
        assert_eq!(loader.loaded_count(), 1);

        let result = loader
            .invoke(
                "host.fixture_echo",
                serde_json::json!({"x": 2, "name": "monty"}),
            )
            .expect("invoke fixture echo");

        assert_eq!(result["echoed"], serde_json::json!(true));
        assert_eq!(result["request"]["tool"], serde_json::json!("host.fixture_echo"));
        assert_eq!(result["request"]["args"]["x"], serde_json::json!(2));
        assert_eq!(result["request"]["args"]["name"], serde_json::json!("monty"));

        loader.unload("host.fixture_echo").expect("unload fixture");
        assert!(!loader.is_loaded("host.fixture_echo"));
        assert_eq!(loader.loaded_count(), 0);
    }

    #[test]
    fn load_replaces_existing_binding() {
        let Some((_dir, fixture)) = compile_fixture_cdylib() else {
            eprintln!("skipping load_replaces_existing_binding: fixture cdylib unavailable");
            return;
        };
        let mut loader = DynamicToolLoader::new();
        loader
            .load(CdylibLoadDirective::new("host.fixture_echo", &fixture))
            .expect("first load");
        loader
            .load(CdylibLoadDirective::new("host.fixture_echo", &fixture))
            .expect("re-load replaces binding");
        assert_eq!(loader.loaded_count(), 1);
        let result = loader
            .invoke("host.fixture_echo", serde_json::json!({"x": 9}))
            .expect("invoke after re-load");
        assert_eq!(result["request"]["args"]["x"], serde_json::json!(9));
    }

    #[test]
    fn unload_all_clears_every_loaded_tool() {
        let Some((_dir, fixture)) = compile_fixture_cdylib() else {
            eprintln!("skipping unload_all_clears_every_loaded_tool: fixture cdylib unavailable");
            return;
        };
        let mut loader = DynamicToolLoader::new();
        loader
            .load(CdylibLoadDirective::new("host.fixture_echo", &fixture))
            .expect("load echo");
        loader
            .load(CdylibLoadDirective::new("host.fixture_echo_alt", &fixture))
            .expect("load echo alt (same artifact, second name)");
        assert_eq!(loader.loaded_count(), 2);
        loader.unload_all();
        assert_eq!(loader.loaded_count(), 0);
        assert!(!loader.is_loaded("host.fixture_echo"));
        assert!(!loader.is_loaded("host.fixture_echo_alt"));
    }

    #[test]
    fn invoke_not_loaded_returns_not_loaded_error() {
        let loader = DynamicToolLoader::new();
        let err = loader
            .invoke("host.never_loaded", serde_json::json!({}))
            .expect_err("invoking an unloaded tool must error");
        assert!(
            matches!(err, DynamicToolLoaderError::NotLoaded { .. }),
            "expected NotLoaded, got {err:?}"
        );
    }

    #[test]
    fn unload_not_loaded_returns_not_loaded_error() {
        let mut loader = DynamicToolLoader::new();
        let err = loader
            .unload("host.never_loaded")
            .expect_err("unloading an unloaded tool must error");
        assert!(
            matches!(err, DynamicToolLoaderError::NotLoaded { .. }),
            "expected NotLoaded, got {err:?}"
        );
    }

    #[test]
    fn load_missing_file_returns_load_error() {
        let mut loader = DynamicToolLoader::new();
        let bogus = Path::new("/nonexistent/brassclaw/does-not-exist-cdylib.dylib");
        let err = loader
            .load(CdylibLoadDirective::new("host.bogus", bogus))
            .expect_err("loading a missing file must error");
        assert!(
            matches!(err, DynamicToolLoaderError::Load { .. }),
            "expected Load, got {err:?}"
        );
        assert!(!loader.is_loaded("host.bogus"));
    }
}
