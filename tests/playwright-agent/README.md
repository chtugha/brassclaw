# BrassClaw Agent Playwright Tests

Comprehensive end-to-end tests for BrassClaw agent functionality via webfrontend-ui.

## Prerequisites

- Node.js 18+
- npm or yarn
- BrassClaw built with `cargo build --release`

## Installation

```bash
npm install
npx playwright install chromium
```

## Running Tests

```bash
# Run all tests
npm test

# Run with browser visible
npm run test:headed

# Run in debug mode
npm run test:debug

# Run with UI mode
npm run test:ui

# Run specific test file
npx playwright test tests/01-connection.spec.ts
```

## Test Structure

- `tests/01-connection.spec.ts` - Connection and authentication tests
- `tests/02-llm-config.spec.ts` - LLM configuration tests
- `tests/03-agent-interaction.spec.ts` - Agent interaction and conversation tests
- `tests/04-tool-execution.spec.ts` - Tool execution tests (planned)
- `tests/05-memory-context.spec.ts` - Memory and context management (planned)
- `tests/06-session-management.spec.ts` - Session handling (planned)

## LLM Configuration

The tests are configured to use the following LLM:

- **Endpoint:** http://192.168.10.223:8000/v1
- **Model:** Qwen/Qwen2.5-7B-Instruct-AWQ
- **API Key:** None required
- **Gateway Token:** Set via environment variable

### Environment Variables

The following environment variables must be set before running tests:

```bash
export BRASSCLAW_GATEWAY_TOKEN=your-token-here
```

The token is read from the environment variable in `playwright.config.ts`.

### Running Tests with LLM

```bash
# Run all tests including LLM interaction
npm test

# Run only LLM configuration tests
npx playwright test tests/02-llm-config.spec.ts

# Run only agent interaction tests
npx playwright test tests/03-agent-interaction.spec.ts

# Run with visible browser to see LLM responses
npm run test:headed
```

### Manual LLM Configuration

If you need to manually configure the LLM provider in the UI:

1. Start the server with gateway token:
   ```bash
   export BRASSCLAW_GATEWAY_TOKEN=your-token-here
   cd ../.. && cargo run --release -p brassclaw_reborn_cli --bin brassclaw-reborn -- serve --host 127.0.0.1 --port 3000
   ```

2. Open browser to http://127.0.0.1:3000

3. Configure LLM provider:
   - Go to Settings → Providers
   - Click "Add Provider"
   - Name: Qwen-Test (or any name)
   - Type: openai-compatible
   - Base URL: http://192.168.10.223:8000/v1
   - Model: Qwen/Qwen2.5-7B-Instruct-AWQ
   - API Key: (leave empty)
   - Click "Save"
   - Click "Test Connection" to verify

4. Start chatting with the agent

## Test Reports

After running tests, view the HTML report:

```bash
npx playwright show-report
```

## Configuration

Edit `playwright.config.ts` to customize:
- Base URL
- Timeout values
- Browser settings
- Screenshot/video capture

## Helper Utilities

The `tests/helpers.ts` file provides:
- `BrassClawTestHelper` class with common operations
- `SELECTORS` object with UI element selectors
- `setupTestEnvironment()` function for test initialization

## Screenshots

Screenshots are automatically captured on test failures and saved to the `screenshots/` directory.

## Troubleshooting

### Server doesn't start

If the webServer fails to start, ensure:
1. BrassClaw is built: `cd ../.. && cargo build --release -p brassclaw_reborn_cli --bin brassclaw-reborn`
2. Port 3000 is available
3. Check server logs in test output

### Tests timeout

Increase timeout values in `playwright.config.ts`:
- `timeout`: Per-test timeout
- `actionTimeout`: Per-action timeout
- `navigationTimeout`: Page navigation timeout

### Browser issues

Reinstall browsers:
```bash
npx playwright install --force chromium
```

## Development

### Adding New Tests

1. Create a new `.spec.ts` file in `tests/` directory
2. Import helpers: `import { setupTestEnvironment } from './helpers';`
3. Use the helper class for common operations
4. Follow existing test patterns

### Updating Selectors

If UI changes, update selectors in `tests/helpers.ts` in the `SELECTORS` object.

## CI/CD Integration

Set `CI=true` environment variable to enable CI mode:
- Disables `test.only`
- Increases retries
- Prevents server reuse

```bash
CI=true npm test