# BrassClaw LLM Configuration Guide for Test Machine

## Current Status
- ✅ BrassClaw binary installed at `/root/.local/bin/brassclaw`
- ✅ Binary includes P0.2/P0.3 enhancements
- ⏳ LLM configuration needed

## Target LLM
- **Endpoint**: `http://192.168.10.223`
- **Model**: `Qwen/Qwen2.5-7B-Instruct-AWQ`
- **Type**: OpenAI-compatible API

## Configuration Options

### Option 1: Environment Variables (Recommended for Testing)
Create a `.env` file or export variables:

```bash
# On test machine (192.168.10.219)
export LLM_BACKEND="provider"
export PROVIDER_BASE_URL="http://192.168.10.223"
export PROVIDER_MODEL="Qwen/Qwen2.5-7B-Instruct-AWQ"
export PROVIDER_API_KEY="not-needed"  # If API doesn't require auth
```

### Option 2: BrassClaw Settings Database
BrassClaw stores settings in a database. Configuration can be set via:
1. Web UI (if available)
2. CLI commands
3. Direct database manipulation

### Option 3: Configuration File
Check for config files in:
- `~/.brassclaw/config.toml`
- `~/.config/brassclaw/config.toml`

## Next Steps

### 1. Initialize BrassClaw
```bash
# On test machine
cd /root/brassclaw-build
cargo run --release -- init
# or
~/.local/bin/brassclaw init
```

### 2. Configure LLM
```bash
# Set environment variables
export LLM_BACKEND="provider"
export PROVIDER_BASE_URL="http://192.168.10.223"
export PROVIDER_MODEL="Qwen/Qwen2.5-7B-Instruct-AWQ"
```

### 3. Test LLM Connection
```bash
# Test with a simple query
cargo run --release -- test-llm
# or use REPL
cargo run --release -- repl
```

### 4. Run BrassClaw
```bash
# Start BrassClaw service
cargo run --release -- serve
# or
~/.local/bin/brassclaw serve
```

## Testing Tool Execution

Once LLM is configured, test:

### File Operations
- Read files
- Write files
- List directories

### Network Operations
- HTTP requests
- API calls

### Skills
- Check available skills
- Execute skill actions

### Monitor Auto-Approval Logs
Look for log messages:
```
Auto-approving tool execution (P0.3 stub)
thread_id=<uuid> action_name=<action>
```

## Troubleshooting

### If LLM Connection Fails
1. Check endpoint is reachable: `curl http://192.168.10.223/v1/models`
2. Verify model name is correct
3. Check BrassClaw logs for errors

### If Tool Execution Fails
1. Check auto-approval logs
2. Verify ThreadExecutionContext values
3. Check action inventory population

## Architecture Notes

- Main binary is a stub (V1 disabled)
- V2 functionality via `cargo run`
- P0.2/P0.3 enhancements are in V2 agent loop
- Auto-approval is active (P0.3)

## Status
**Ready for LLM configuration and testing**