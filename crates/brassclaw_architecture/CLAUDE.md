# brassclaw_architecture guardrails

- This crate is test-only architecture enforcement; do not add production dependencies or runtime behavior.
- Use `cargo metadata` or equivalent workspace graph checks to enforce Reborn dependency direction.
- Boundary tests should fail loudly with the exact forbidden edge and crate name.
- The application-state filesystem guard separately recognizes the inspected
  OS executable and private artifact opens in HostRuntime's `native_image`.
  This is a retained implementation boundary, not a configuration/state store;
  other opens and read APIs in that module remain forbidden.
- Keep rules conservative and explicit; update docs when intentional architecture edges change.
