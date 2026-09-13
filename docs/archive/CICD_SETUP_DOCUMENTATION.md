# GitHub Actions CI/CD Setup Documentation

## Overview

BrassClaw now uses GitHub Actions to automatically build and release binaries for multiple platforms. This document describes the automated release workflow and how to use it.

## Workflow Details

### File Location
`.github/workflows/release.yml`

### Trigger
The workflow is triggered automatically when a tag matching the pattern `v*` is pushed to the repository (e.g., `v0.29.9`, `v1.0.0`, `v2.1.3-beta`).

### Build Matrix

The workflow builds binaries for three platforms:

| Platform | Target Triple | Artifact Name | Notes |
|----------|--------------|---------------|-------|
| Linux x86_64 | `x86_64-unknown-linux-musl` | `brassclaw-linux-amd64` | Statically linked with musl for maximum compatibility |
| macOS ARM64 | `aarch64-apple-darwin` | `brassclaw-macos-arm64` | For Apple Silicon (M1/M2/M3) |
| macOS x86_64 | `x86_64-apple-darwin` | `brassclaw-macos-amd64` | For Intel Macs |

### Build Process

For each platform, the workflow:

1. **Checks out the repository** with submodules
2. **Installs Rust toolchain** with the appropriate target
3. **Installs platform-specific dependencies**:
   - Linux: musl-tools for static linking
4. **Caches build artifacts** to speed up subsequent builds:
   - Cargo registry
   - Cargo index
   - Build target directory
5. **Builds the release binary** using `cargo build --release --target <target>`
6. **Generates SHA256 checksum** for the binary
7. **Uploads artifacts** (binary + checksum) to GitHub Actions

### Release Creation

After all builds complete successfully:

1. **Downloads all artifacts** from the build jobs
2. **Creates a GitHub Release** with:
   - Tag name as the release title
   - Auto-generated release notes from commits
   - All binaries and their SHA256 checksums attached
3. **Publishes the release** (not as draft or prerelease)

## Usage

### Creating a New Release

1. **Ensure all changes are committed and pushed to main**:
   ```bash
   git add .
   git commit -m "Your commit message"
   git push origin main
   ```

2. **Create and push a version tag**:
   ```bash
   # For production releases
   git tag v0.29.9
   git push origin v0.29.9
   
   # For pre-releases (beta, rc, etc.)
   git tag v1.0.0-beta.1
   git push origin v1.0.0-beta.1
   ```

3. **Monitor the workflow**:
   - Go to https://github.com/chtugha/brassclaw/actions
   - Click on the "Release" workflow run
   - Watch the build progress for each platform
   - Typical build time: 15-30 minutes depending on cache hits

4. **Verify the release**:
   - Go to https://github.com/chtugha/brassclaw/releases
   - Check that the release was created with all artifacts
   - Verify checksums match the uploaded files

### Testing the Workflow

Before creating a production release, test with a test tag:

```bash
# Create test tag
git tag v0.29.9-test
git push origin v0.29.9-test

# Monitor the workflow at GitHub Actions

# If successful, clean up test release
gh release delete v0.29.9-test --yes
git tag -d v0.29.9-test
git push origin :refs/tags/v0.29.9-test
```

### Downloading Releases

Users can download pre-built binaries from:
- Latest release: https://github.com/chtugha/brassclaw/releases/latest
- All releases: https://github.com/chtugha/brassclaw/releases

Example download commands:

```bash
# Linux
curl -LO https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-linux-amd64
curl -LO https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-linux-amd64.sha256

# macOS ARM64
curl -LO https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64
curl -LO https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64.sha256

# macOS x86_64
curl -LO https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64
curl -LO https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64.sha256
```

### Verifying Checksums

```bash
# Linux/macOS
sha256sum -c brassclaw-linux-amd64.sha256
# or
shasum -a 256 -c brassclaw-macos-arm64.sha256
```

## Workflow Features

### Caching Strategy

The workflow uses GitHub Actions cache to speed up builds:
- **Cargo registry cache**: Stores downloaded crate metadata
- **Cargo index cache**: Stores the crates.io index
- **Build cache**: Stores compiled dependencies and incremental compilation data

Cache keys are based on:
- Operating system
- Target triple
- `Cargo.lock` hash (invalidates when dependencies change)

### Security

- **Minimal permissions**: The workflow only requests `contents: write` permission for the release job
- **Submodule support**: Recursively checks out submodules
- **Checksum verification**: SHA256 checksums provided for all binaries
- **Static linking**: Linux binaries are statically linked with musl for security and portability

### Error Handling

- **Fail-fast disabled**: If one platform build fails, others continue
- **Artifact preservation**: Build artifacts are preserved even if release creation fails
- **Automatic retry**: GitHub Actions automatically retries transient failures

## Maintenance

### Updating Rust Version

The workflow uses `dtolnay/rust-toolchain@stable`, which automatically uses the latest stable Rust. To pin to a specific version:

```yaml
- name: Install Rust
  uses: dtolnay/rust-toolchain@stable
  with:
    toolchain: 1.92.0  # Specify version
    targets: ${{ matrix.target }}
```

### Adding New Platforms

To add support for additional platforms, extend the build matrix:

```yaml
matrix:
  include:
    # ... existing platforms ...
    - os: windows-latest
      target: x86_64-pc-windows-msvc
      artifact_name: brassclaw-windows-amd64.exe
```

Note: Windows builds may require additional steps for handling `.exe` extension.

### Modifying Build Flags

To add custom build flags or features:

```yaml
- name: Build
  run: cargo build --release --target ${{ matrix.target }} --features "your-feature"
```

## Troubleshooting

### Build Failures

1. **Check the Actions tab**: https://github.com/chtugha/brassclaw/actions
2. **Review build logs**: Click on the failed job to see detailed logs
3. **Common issues**:
   - Missing dependencies: Update the "Install dependencies" step
   - Compilation errors: Fix in the codebase and push
   - Cache corruption: Clear cache by changing cache key or manually deleting

### Release Not Created

If builds succeed but release isn't created:
1. Check the "release" job logs
2. Verify `GITHUB_TOKEN` has sufficient permissions
3. Ensure tag format matches `v*` pattern

### Checksum Mismatches

If checksums don't match:
1. Re-download the binary
2. Verify you're using the correct checksum file
3. Check for network corruption during download

## Migration Notes

### Changes from Previous Workflow

The new workflow replaces the previous cargo-dist based workflow with a simpler approach:

**Removed:**
- cargo-dist dependency and complexity
- WASM extension building (can be re-added if needed)
- Docker image building (handled separately)
- Multiple manifest files

**Added:**
- Simpler, more maintainable workflow
- Direct control over build process
- Faster builds with better caching
- Clearer artifact naming

**Preserved:**
- Multi-platform support
- Automatic release creation
- Checksum generation
- GitHub Actions integration

## Future Enhancements

Potential improvements for future iterations:

1. **Windows support**: Add Windows builds to the matrix
2. **ARM Linux**: Add `aarch64-unknown-linux-musl` target
3. **Homebrew formula**: Auto-update Homebrew formula on release
4. **Docker images**: Integrate Docker image building into release workflow
5. **Release notes**: Generate more detailed release notes from changelog
6. **Signing**: Add code signing for macOS and Windows binaries
7. **Notarization**: Notarize macOS binaries for Gatekeeper

## Support

For issues with the CI/CD workflow:
1. Check this documentation
2. Review GitHub Actions logs
3. Open an issue at https://github.com/chtugha/brassclaw/issues

---

**Last Updated**: 2026-06-21  
**Workflow Version**: 1.0  
**Maintainer**: BrassClaw Team