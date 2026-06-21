import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: false, // Run tests sequentially for agent testing
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 1,
  workers: 1, // Single worker for agent state consistency
  reporter: [
    ['html', { outputFolder: 'playwright-report' }],
    ['list'],
    ['json', { outputFile: 'test-results.json' }]
  ],
  
  use: {
    baseURL: 'http://127.0.0.1:3000',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    actionTimeout: 30000,
    navigationTimeout: 30000,
  },

  timeout: 60000, // 60 seconds per test

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  // Start brassclaw-reborn server before tests
  webServer: {
    command: 'cd ../.. && cargo run --release -p brassclaw_reborn_cli --bin brassclaw-reborn -- serve --host 127.0.0.1 --port 3000',
    url: 'http://127.0.0.1:3000',
    reuseExistingServer: !process.env.CI,
    timeout: 120000, // 2 minutes to start server
    stdout: 'pipe',
    stderr: 'pipe',
    env: {
      BRASSCLAW_REBORN_WEBUI_TOKEN: 'test-playwright-token',
      BRASSCLAW_REBORN_WEBUI_USER_ID: 'test-playwright-user',
      BRASSCLAW_GATEWAY_TOKEN: process.env.BRASSCLAW_GATEWAY_TOKEN || 'test-token',
    },
  },
});

// Made with Bob
