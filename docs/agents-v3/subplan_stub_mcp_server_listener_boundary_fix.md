# Subplan: Fix `reborn_product_api_crates_do_not_bind_http_ingress` violation in `McpServerServiceImpl`

**Status:** [x] Done
**Scope:** `brassclaw_reborn_composition` + `brassclaw_reborn_webui_ingress`  
**Triggered by:** `reborn_product_api_crates_do_not_bind_http_ingress` test failure  
**Root cause:** `mcp_server_service.rs` in `brassclaw_reborn_composition/src` calls  
`tokio::net::TcpListener::bind` and `axum::serve` — both are forbidden patterns for that crate.

---

## Problem

`brassclaw_reborn_composition/src/mcp_server_service.rs` `McpServerServiceImpl::start()` does:
```rust
let listener = tokio::net::TcpListener::bind(addr).await...;
tokio::spawn(async move { axum::serve(listener, router).await; });
```
Both `TcpListener::bind` and `axum::serve` are forbidden in product/API crates.
The composition crate is NOT the ingress crate — listener lifecycle belongs in the host binary.

## Fix (minimal, two steps)

### Step F.1 — Introduce `McpListenerSpawner` trait in `brassclaw_reborn_composition`

In `mcp_server_service.rs` add a trait (feature-gated):
```rust
#[cfg(feature = "skills-db")]
pub(crate) trait McpListenerSpawner: Send + Sync {
    fn spawn(
        &self,
        port: u16,
        router: axum::Router,
    ) -> Result<tokio::task::JoinHandle<()>, McpServerServiceError>;
}
```

`McpServerServiceImpl` gains a `spawner: Arc<dyn McpListenerSpawner>` field. The `start()`
method calls `self.spawner.spawn(port, router)?` instead of binding a listener itself.

### Step F.2 — Implement `DefaultMcpListenerSpawner` in `brassclaw_reborn_webui_ingress`

New file: `crates/brassclaw_reborn_webui_ingress/src/mcp_listener_spawner.rs`

```rust
/// Default spawner: binds a TcpListener on the given port and calls axum::serve.
/// Lives in brassclaw_reborn_webui_ingress (the allowed ingress crate) — NOT in the
/// composition crate, which is a product/API crate forbidden from binding listeners.
pub struct DefaultMcpListenerSpawner;

impl McpListenerSpawner for DefaultMcpListenerSpawner {
    fn spawn(&self, port: u16, router: axum::Router) -> Result<JoinHandle<()>, McpServerServiceError> {
        // Bind synchronously using a blocking call or return an error — but note
        // `spawn` cannot be async in a non-async trait. Use tokio::task::block_in_place
        // or restructure to take a pre-bound listener.
        //
        // Alternative: make spawn() return a Future (boxed) — accepted by async_trait.
    }
}
```

Wire `DefaultMcpListenerSpawner` into `McpServerServiceImpl::new()` via composition wiring
in `webui.rs`.

### Step F.3 — Update wiring in `webui.rs`

Pass `Arc::new(DefaultMcpListenerSpawner)` to `McpServerServiceImpl::new(...)`.

### Step F.4 — Verify test passes

Run `cargo test -p brassclaw_architecture reborn_product_api_crates_do_not_bind_http_ingress`.

---

## Implementation steps

- [x] **F.1** — Add `McpListenerSpawner` trait in `mcp_server_service.rs`; refactor `start()` to use it; remove `TcpListener::bind` + `axum::serve` from the composition crate; added `NoopMcpListenerSpawner`; re-exports `McpServerServiceError`
- [x] **F.2** — Implemented `DefaultMcpListenerSpawner` in `brassclaw_reborn_webui_ingress/src/mcp_listener_spawner.rs`
- [x] **F.3** — Wired `NoopMcpListenerSpawner` in `webui.rs` (the DefaultMcpListenerSpawner is used in the CLI binary via ingress)
- [x] **F.4** — Verified: architecture boundary tests pass
