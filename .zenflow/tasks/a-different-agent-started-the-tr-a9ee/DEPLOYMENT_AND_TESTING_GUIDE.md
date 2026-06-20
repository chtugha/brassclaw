# BrassClaw P0.2/P0.3 Deployment and Testing Guide

## Architecture Overview

### V1 vs V2
- **V1 (Disabled)**: Main binary (`brassclaw`) is a stub that exits with error
- **V2 (Active)**: Full functionality via `cargo run` from source
- **P0.2/P0.3 Changes**: Implemented in V2 agent loop

### Binary Status
- ✅ Built on test machine: `/root/brassclaw-build/target/release/brassclaw`
- ✅ Installed to: `/root/.local/bin/brassclaw`
- ⚠️ Binary is V1 stub - must use `cargo run` for V2 functionality

## Test Machine Configuration

### Machine Details
- **IP**: 192.168.10.219
- **Build Directory**: `/root/brassclaw-build`
- **Binary Location**: `/root/.local/bin/brassclaw`
- **Build Time**: 17m 30s
- **Binary Size**: 335KB (stub)

### LLM Target
- **Endpoint**: http://192.168.10.223
- **Model**: Qwen/Qwen2.5-7B-Instruct-AWQ
- **Type**: OpenAI-compatible API

## Testing Steps

### Step 1: Configure LLM Connection

On test machine (192.168.10.219):

```bash
cd /root/brassclaw-build

# Set environment variables
export LLM_BACKEND="provider"
export PROVIDER_BASE_URL="http://192.168.10.223"
export PROVIDER_MODEL="Qwen/Qwen2.5-7B-Instruct-AWQ"
export PROVIDER_API_KEY="not-needed"  # If no auth required

# Or create .env file
cat > .env << 'EOF'
LLM_BACKEND=provider
PROVIDER_BASE_URL=http://192.168.10.223
PROVIDER_MODEL=Qwen/Qwen2.5-7B-Instruct-AWQ
PROVIDER_API_KEY=not-needed
EOF
```

### Step 2: Initialize BrassClaw

```bash
# Run onboarding wizard
cargo run --release -- onboard --quick

# Or use doctor to check configuration
cargo run --release -- doctor
```

### Step 3: Test LLM Connection

```bash
# Check models status
cargo run --release -- models status

# Test with REPL
cargo run --release -- repl
```

### Step 4: Test Tool Execution with Auto-Approval

```bash
# Run with auto-approval enabled
cargo run --release -- run --auto-approve

# Or start in CLI-only mode
cargo run --release -- run --cli-only --auto-approve
```

### Step 5: Monitor Auto-Approval Logs

Look for these log messages indicating P0.3 is working:

```
Auto-approving tool execution (P0.3 stub)
thread_id=<uuid> action_name=<action>
```

### Step 6: Test Specific Tools

#### File Operations
```bash
# In REPL or via message
cargo run --release -- -m "List files in the current directory"
cargo run --release -- -m "Read the contents of Cargo.toml"
cargo run --release -- -m "Create a test file called hello.txt with 'Hello World'"
```

#### Network Operations
```bash
cargo run --release -- -m "Make an HTTP GET request to http://httpbin.org/get"
```

#### Skills
```bash
# List available skills
cargo run --release -- skills list

# Test a skill
cargo run --release -- -m "Use a skill to help me with something"
```

## Verification Checklist

### P0.2 Enhancements (ThreadExecutionContext)
- [ ] project_id extracted from job context
- [ ] thread_type determined correctly (Foreground/Research/Mission)
- [ ] step_id tracked properly
- [ ] user_timezone set (default UTC)
- [ ] source_channel extracted from metadata
- [ ] conversation_id linked to originating conversation

### P0.3 Auto-Approval
- [ ] AutoApprovingGateController logs approval requests
- [ ] Tool execution proceeds without manual approval
- [ ] All tool types work (file, network, shell, etc.)
- [ ] No blocking on approval gates

### Action Inventory
- [ ] V1 action inventory populated
- [ ] V2 action inventory populated
- [ ] Actions available to agent

### Error Handling
- [ ] EngineError variants mapped correctly
- [ ] Detailed error messages provided
- [ ] Errors logged with appropriate severity

## Expected Behavior

### Successful Tool Execution Flow
1. Agent receives task
2. Agent decides to use a tool
3. ThreadExecutionContext created with real values
4. Action inventory checked
5. AutoApprovingGateController auto-approves
6. Tool executes
7. Result returned to agent
8. Agent continues

### Log Messages to Look For

```
INFO Auto-approving tool execution (P0.3 stub) thread_id=... action_name=...
INFO Executing tool: <tool_name> with parameters: {...}
INFO Tool execution successful: <result>
```

### Error Messages to Watch For

```
ERROR EffectExecutor failed: <error details>
ERROR Failed to extract project_id from job context
ERROR Action not found in inventory: <action_name>
```

## Troubleshooting

### LLM Connection Issues
```bash
# Test endpoint directly
curl http://192.168.10.223/v1/models

# Check environment variables
env | grep -E '(LLM|PROVIDER)'

# Run doctor
cargo run --release -- doctor
```

### Tool Execution Issues
```bash
# Check logs
cargo run --release -- logs --follow

# Verify auto-approval is enabled
cargo run --release -- run --auto-approve --cli-only

# Check action inventory
cargo run --release -- skills list
```

### Build Issues
```bash
# Clean and rebuild
cargo clean
cargo build --release

# Check for compilation errors
cargo check
```

## Performance Notes

- Build time: ~17 minutes on test machine
- Binary size: 335KB (stub), full binary would be larger
- V2 via cargo run has longer startup time
- Consider building with `--release` for production use

## Next Steps After Testing

1. ✅ Verify all tools work with auto-approval
2. ✅ Confirm ThreadExecutionContext values are correct
3. ✅ Check action inventory is populated
4. ✅ Monitor logs for errors
5. ✅ Test various tool types (file, network, shell)
6. ✅ Document any issues found
7. ✅ Create final completion report

## Known Limitations

- Main binary is V1 stub - must use `cargo run`
- 272 pre-existing warnings from V1 code (documented in WARNINGS_ANALYSIS.md)
- Auto-approval is global (P0.3 stub) - no per-action control yet
- No approval UI integration yet (future work)

## Success Criteria

- [x] P0.2 implementation complete
- [x] P0.3 implementation complete
- [x] Zero compilation errors
- [x] Zero new warnings introduced
- [x] Code committed and pushed to GitHub
- [x] Binary built and deployed to test machine
- [ ] LLM configured and connected
- [ ] Tool execution tested and working
- [ ] Auto-approval verified in logs
- [ ] Final completion report created

## Contact

For issues or questions, refer to:
- P0.2_COMPLETION_REPORT.md
- P0.3_COMPLETION_REPORT.md
- WARNINGS_ANALYSIS.md
- LLM_CONFIGURATION_GUIDE.md