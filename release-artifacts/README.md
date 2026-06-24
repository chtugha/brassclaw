# Brassclaw Reborn - Linux AMD64 Release

This directory contains the precompiled Linux AMD64 binary for brassclaw.

## Files

- `brassclaw-linux-amd64` - The main executable binary (80 MB)
- `brassclaw-linux-amd64.sha256` - SHA256 checksum for verification

## System Requirements

- **Architecture:** x86_64 (AMD64)
- **Operating System:** Linux (any distribution)
- **Kernel:** 2.6.32 or later
- **Dependencies:** None (statically linked)

## Installation

### 1. Download the Binary

Download both files:
- `brassclaw-linux-amd64`
- `brassclaw-linux-amd64.sha256`

### 2. Verify Checksum (Recommended)

```bash
sha256sum -c brassclaw-linux-amd64.sha256
```

Expected output:
```
brassclaw-linux-amd64: OK
```

### 3. Make Executable

```bash
chmod +x brassclaw-linux-amd64
```

### 4. Run

```bash
./brassclaw-linux-amd64 --help
```

Or move to a directory in your PATH:

```bash
sudo mv brassclaw-linux-amd64 /usr/local/bin/brassclaw
brassclaw --help
```

## Binary Details

- **Build Date:** June 20, 2026
- **Build Method:** Cross-compiled from macOS using musl toolchain
- **Linking:** Static (no external dependencies required)
- **Features:** webui-v2-beta enabled
- **SHA256:** `902a0c2f4a61e123fbf158876051ab1b79f976271dd016d8a772897f2ee7f9de`

## Compatibility

This binary is statically linked with musl libc and should run on any x86_64 Linux distribution, including:

- Ubuntu / Debian
- CentOS / RHEL / Fedora
- Alpine Linux
- Arch Linux
- openSUSE
- And any other x86_64 Linux distribution

No additional runtime dependencies are required.

## Verification

To verify the binary type:

```bash
file brassclaw-linux-amd64
```

Expected output:
```
brassclaw-linux-amd64: ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), static-pie linked, stripped
```

To check for dependencies (should show "statically linked"):

```bash
ldd brassclaw-linux-amd64
```

Expected output:
```
statically linked
```

## Support

For issues, questions, or contributions, please visit the project repository.

## License

See the main project repository for license information.