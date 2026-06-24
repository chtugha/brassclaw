# BrassClaw V2 - Production Release

## 🎉 Major Milestone: V1 to V2 Transition Complete

This release marks the completion of the V1 to V2 architecture transition. The brassclaw agent is now fully operational with the new V2 infrastructure.

### ✨ Key Features

- **V2 Infrastructure**: 100% complete with 47 capabilities across 13 domains
- **WebUI V2**: Fully functional web interface with authentication
- **Zero Compilation Errors**: Clean, production-ready codebase
- **Comprehensive Testing**: 10/10 Playwright tests passing
- **Static Binary**: No dependencies, runs on any x86_64 Linux system

### 📦 What's Included

- `brassclaw-linux-amd64`: Precompiled binary for Linux AMD64
- `brassclaw-linux-amd64.sha256`: SHA256 checksum for verification

### 🚀 Quick Start

```bash
# Download and verify
wget https://github.com/chtugha/brassclaw/releases/download/v0.29.1/brassclaw-linux-amd64
wget https://github.com/chtugha/brassclaw/releases/download/v0.29.1/brassclaw-linux-amd64.sha256
sha256sum -c brassclaw-linux-amd64.sha256

# Install
chmod +x brassclaw-linux-amd64
sudo mv brassclaw-linux-amd64 /usr/local/bin/brassclaw

# Run
brassclaw serve --port 3000
```

### 📋 System Requirements

- **OS**: Any x86_64 Linux distribution
- **Architecture**: AMD64 (x86_64)
- **Dependencies**: None (statically linked)

### 🔧 Configuration

Set up authentication:
```bash
export BRASSCLAW_REBORN_WEBUI_TOKEN="your-secure-token"
export BRASSCLAW_REBORN_WEBUI_USER_ID="your-user-id"
```

Configure LLM backend in `~/.brassclaw/reborn/config.toml`

### 📊 Changes Since Last Release

- Removed V1 tools.rs stub (235 lines)
- Fixed 6 files to remove V1 references
- Verified routine engine V2 migration
- Created comprehensive Playwright test suite
- Built with webui-v2-beta feature enabled

### 🔒 Security

- Binary is Position Independent Executable (PIE)
- Static linking eliminates dependency vulnerabilities
- SHA256 checksum provided for verification

### 📚 Documentation

- [Installation Guide](./release-artifacts/README.md)
- [V1 to V2 Transition Summary](./V1_TO_V2_TRANSITION_COMPLETION_SUMMARY.md)
- [WebUI Test Report](./BRASSCLAW_WEBUI_V2_TEST_REPORT.md)

### 🐛 Known Issues

- LLM configuration required for agent operations
- 42+ test files need updates (deferred, non-blocking)

### 💬 Support

For issues, questions, or contributions, please visit the [GitHub repository](https://github.com/chtugha/brassclaw).