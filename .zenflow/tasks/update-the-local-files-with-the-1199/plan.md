# BrassClaw Migration to IronClaw Reborn Architecture

## Approach
Fresh start from upstream ironclaw/main (1274 commits ahead), adopting the full IronClaw Reborn
architecture. Re-apply brassclaw branding and local-LLM optimizations. Convert 4 local tools
to v2 Skills. Deploy to test machine with vLLM + Qwen.

## Key Decisions
- **Architecture**: Full IronClaw Reborn (ironclaw-reborn binary, new config system, drivers, WebUI v2)
- **Extensions**: Convert caldav, local_notes, local_search, web-browse to v2 Skills (markdown + YAML)
- **LLM Focus**: Preserve 8192-token budget, compact prompts, local LLM auto-detection
- **Database**: Keep libSQL support for local profiles
- **Deployment**: DietPi + vLLM (replaces Ollama) + systemd

## Brassclaw Customizations to Re-apply
1. Token budget: 8192 total, 2048 skill context
2. Compact Tier 0 system prompt (<=800 tokens)
3. Profile-based setup (local, local-sandbox, server, server-multitenant)
4. Token Guard priority dropping
5. Skills with budget caps (128-384 tokens each)
6. Deploy scripts for DietPi/systemd
7. Branding (name, logo, URLs, env vars)

### [x] Step: Investigation
- Studied upstream ironclaw v2 design (Engine v2 + Python orchestrator + Monty VM)
- Studied IronClaw Reborn architecture (new binary, drivers, runners, subagents)
- Identified 4 broken extensions and root causes
- Confirmed 1274 commits, 735K+ lines divergence makes merge impossible

### [x] Step: Fresh start from upstream and rename
- Reset repo to upstream/main
- Systematic rename: ironclaw -> brassclaw (crate names, binary names, env vars, config dirs)
- Rename ironclaw-reborn -> brassclaw-reborn
- Rename IRONCLAW_* env vars -> BRASSCLAW_*
- Rename .ironclaw/ config dir -> .brassclaw/
- Verify workspace compiles after rename

### [x] Step: Re-apply local-LLM optimizations
- Port token budget settings (8192 total, 2048 skill context)
- Port compact Tier 0 system prompt (<=800 tokens)
- Port local LLM auto-detection (loopback address detection)
- Port profile configurations (local.toml, local-sandbox.toml, server.toml, server-multitenant.toml)
- Ensure Token Guard priority dropping works with v2 orchestrator
- Add vLLM as first-class provider alongside Ollama

### [x] Step: Convert tools to v2 Skills
- Create caldav SKILL.md with CalDAV API knowledge + credential spec
- Create local_notes SKILL.md for note management
- Create local_search SKILL.md for filesystem search guidance
- Create web-browse SKILL.md for Playwright MCP browser usage
- Set appropriate token budgets (256-384 tokens each)
- Remove old broken registry entries for these tools

### [x] Step: Update deployment for DietPi + vLLM
- Create brassclaw.service systemd unit for DietPi
- Create vllm.service systemd unit for Qwen/Qwen2.5-7B-Instruct-AWQ
- Create setup script for DietPi deployment
- Configure dietpi-services integration

### [x] Step: Documentation
- Document v2 engine architecture (Python orchestrator, Monty VM, host functions)
- Document Reborn architecture (drivers, runners, model routes)
- Document Skills system (activation, scoring, budgets, CodeAct)
- Document deployment guide (DietPi, vLLM, systemd)
- Document configuration reference (profiles, env vars, config.toml)
- Document security model (WASM sandbox, credential injection, policy engine)

### [x] Step: Update README
- Rewrite README for brassclaw + Reborn architecture
- Update quick-start for local LLM setup with vLLM
- Update model recommendations table
- Update architecture diagram
- Update configuration reference

### [x] Step: Commit and push
- Committed all changes with descriptive message
- Pushed to main on github.com/chtugha/brassclaw (force push, new PAT)

### [x] Step: Setup test machine (192.168.10.219)
- Removed old ironclaw remnants and Ollama on .219 (CPU-only)
- Removed Ollama on .223 (GPU machine with RTX 5060 Ti)
- vLLM already running on .223 with Qwen/Qwen2.5-7B-Instruct-AWQ
- Built brassclaw-reborn on .219 with webui-v2-beta feature
- Configured brassclaw to connect to remote vLLM on .223
- Created systemd service with proper env vars (BRASSCLAW_REBORN_LOG, WEBUI_TOKEN, etc.)
- Registered in dietpi-services

### [x] Step: Test and debug on test machine
- Fixed lib.rs null byte corruption
- Fixed CalDAV SKILL.md invalid YAML
- Fixed tracing env var (BRASSCLAW_REBORN_LOG, not RUST_LOG)
- Fixed workspace overlap issue (WorkingDirectory=/root/brassclaw-workspace)
- E2E verified: thread creation, message submission, LLM inference, response retrieval
- Multi-turn conversation works (context retained across turns)
- Tool usage works (builtin.echo, builtin.skill_list confirmed)
- Skills listing works (all 30+ skills visible)
- Service stable: 23MB memory, auto-restart on failure
