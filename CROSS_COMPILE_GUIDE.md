# Cross-Compilation Quick Reference Guide

This guide provides quick reference for cross-compiling brassclaw-reborn for different platforms.

## Linux AMD64 (from macOS)

### Prerequisites (One-time Setup)

```bash
# Install Rust target
rustup target add x86_64-unknown-linux-musl

# Install musl-cross toolchain (macOS)
brew install filosottile/musl-cross/musl-cross

# Configure Cargo (create .cargo/config.toml)
mkdir -p .cargo
cat > .cargo/config.toml << 'EOF'
[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"
EOF
```

### Build Command

```bash
cargo build --release --target x86_64-unknown-linux-musl -p brassclaw_reborn_cli 
```

### Output Location

```
target/x86_64-unknown-linux-musl/release/brassclaw-reborn
```

### Create Release Artifacts

```bash
# Create directory
mkdir -p release-artifacts

# Copy binary
cp target/x86_64-unknown-linux-musl/release/brassclaw-reborn \
   release-artifacts/brassclaw-reborn-linux-amd64

# Generate checksum
cd release-artifacts
shasum -a 256 brassclaw-reborn-linux-amd64 > brassclaw-reborn-linux-amd64.sha256
```

## Alternative: Using cross (requires Docker)

If Docker is available, you can use the `cross` tool:

```bash
# Install cross (one-time)
cargo install cross

# Build for Linux AMD64
cross build --release --target x86_64-unknown-linux-gnu -p brassclaw_reborn_cli 
```

## Supported Targets

### Linux
- `x86_64-unknown-linux-musl` - Linux AMD64 (static, recommended)
- `x86_64-unknown-linux-gnu` - Linux AMD64 (dynamic, requires glibc)
- `aarch64-unknown-linux-musl` - Linux ARM64 (static)
- `aarch64-unknown-linux-gnu` - Linux ARM64 (dynamic)

### macOS
- `x86_64-apple-darwin` - macOS Intel
- `aarch64-apple-darwin` - macOS Apple Silicon

### Windows
- `x86_64-pc-windows-msvc` - Windows 64-bit (MSVC)
- `x86_64-pc-windows-gnu` - Windows 64-bit (MinGW)

## Verification Commands

```bash
# Check binary type
file brassclaw-reborn-linux-amd64

# Check dependencies (for musl builds, should show "statically linked")
ldd brassclaw-reborn-linux-amd64

# Verify checksum
sha256sum -c brassclaw-reborn-linux-amd64.sha256
```

## Troubleshooting

### Issue: Linker not found

**Solution:** Install the appropriate cross-compilation toolchain:
- Linux musl: `brew install filosottile/musl-cross/musl-cross`
- Linux gnu: Install a Linux cross-compiler or use `cross` tool

### Issue: cross requires Docker

**Solution:** Either:
1. Install Docker Desktop for Mac
2. Use native cargo with musl target (as shown above)

### Issue: Build fails with linking errors

**Solution:** 
1. Ensure the linker is in PATH
2. Check `.cargo/config.toml` has correct linker path
3. Try cleaning and rebuilding: `cargo clean && cargo build ...`

## Build Matrix for CI/CD

For automated builds, consider this matrix:

```yaml
targets:
  - x86_64-unknown-linux-musl    # Linux AMD64 (static)
  - aarch64-unknown-linux-musl   # Linux ARM64 (static)
  - x86_64-apple-darwin          # macOS Intel
  - aarch64-apple-darwin         # macOS Apple Silicon
  - x86_64-pc-windows-msvc       # Windows 64-bit
```

## Performance Notes

- **musl builds** are slightly larger but more portable
- **gnu builds** are smaller but require specific glibc versions
- **Static linking** adds ~10-20MB but eliminates runtime dependencies
- **Cross-compilation** from macOS to Linux takes ~5-10 minutes

## Release Checklist

- [ ] Build binary for target platform
- [ ] Verify binary type with `file` command
- [ ] Generate SHA256 checksum
- [ ] Test binary on target platform (if possible)
- [ ] Create release notes
- [ ] Tag release in git
- [ ] Upload to GitHub releases

## Additional Resources

- [Rust Platform Support](https://doc.rust-lang.org/nightly/rustc/platform-support.html)
- [cross Documentation](https://github.com/cross-rs/cross)
- [musl-cross Toolchain](https://github.com/FiloSottile/homebrew-musl-cross)