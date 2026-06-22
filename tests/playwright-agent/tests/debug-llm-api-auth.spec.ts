import { test } from '@playwright/test';
import { setupTestEnvironment } from './helpers';

test('Debug LLM API authentication', async ({ page }) => {
  const helper = await setupTestEnvironment(page);
  
  // Listen for API requests
  page.on('request', (request) => {
    const url = request.url();
    if (url.includes('/api/')) {
      console.log(`\n=== API Request ===`);
      console.log(`URL: ${url}`);
      console.log(`Method: ${request.method()}`);
      const authHeader = request.headers()['authorization'];
      console.log(`Authorization header: ${authHeader || 'MISSING!'}`);
      console.log(`===================\n`);
    }
  });
  
  page.on('response', async (response) => {
    const url = response.url();
    if (url.includes('/api/llm') || url.includes('/api/webchat/v2/llm')) {
      console.log(`\n=== API Response ===`);
      console.log(`URL: ${url}`);
      console.log(`Status: ${response.status()}`);
      try {
        const body = await response.text();
        console.log(`Body: ${body.substring(0, 500)}`);
      } catch (e) {
        console.log(`Could not read body`);
      }
      console.log(`====================\n`);
    }
  });
  
  // Navigate to settings/inference
  console.log('\n>>> Navigating to /settings/inference');
  await page.goto('/settings/inference');
  await page.waitForLoadState('networkidle');
  
  // Wait a bit to see all requests
  await page.waitForTimeout(3000);
  
  // Check sessionStorage
  const tokenInStorage = await page.evaluate(() => {
    return sessionStorage.getItem('brassclaw_token');
  });
  
  console.log(`\n>>> Token in sessionStorage: ${tokenInStorage || 'MISSING!'}`);
  
  // Take screenshot
  await page.screenshot({ path: 'test-results/debug-llm-api-auth.png', fullPage: true });
});

// Made with Bob
