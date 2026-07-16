import { test, expect } from '@playwright/test';
import { setupTestEnvironment, SELECTORS } from './helpers';

test('Debug provider creation', async ({ page }) => {
  const helper = await setupTestEnvironment(page);
  
  // Listen for all network requests
  page.on('response', async (response) => {
    const url = response.url();
    if (url.includes('/api/llm') || url.includes('/api/inference')) {
      console.log(`\n=== API Response ===`);
      console.log(`URL: ${url}`);
      console.log(`Status: ${response.status()}`);
      try {
        const body = await response.text();
        console.log(`Body: ${body}`);
      } catch (e) {
        console.log(`Could not read body: ${e.message}`);
      }
      console.log(`===================\n`);
    }
  });
  
  page.on('request', (request) => {
    const url = request.url();
    if (url.includes('/api/llm') || url.includes('/api/inference')) {
      console.log(`\n=== API Request ===`);
      console.log(`URL: ${url}`);
      console.log(`Method: ${request.method()}`);
      console.log(`Headers: ${JSON.stringify(request.headers(), null, 2)}`);
      const postData = request.postData();
      if (postData) {
        console.log(`Body: ${postData}`);
      }
      console.log(`===================\n`);
    }
  });
  
  // Navigate to settings/inference
  await page.goto('/settings/inference');
  await page.waitForLoadState('networkidle');
  
  // Wait for Add provider button
  await page.waitForSelector(SELECTORS.addProviderButton, { timeout: 10000 });
  
  // Click Add provider
  await page.click(SELECTORS.addProviderButton);
  
  // Wait for dialog
  await page.getByLabel('Display name').waitFor({ state: 'visible', timeout: 10000 });
  
  // Fill in provider details
  const timestamp = Date.now();
  await page.getByLabel('Display name').fill(`Test-${timestamp}`);
  await page.getByLabel('Provider ID').fill(`test-${timestamp}`);
  await page.getByLabel('Base URL').fill('http://192.168.10.171:8000/v1');
  await page.getByLabel('Default model').fill('Qwen/Qwen2.5-7B-Instruct-AWQ');
  
  // Take screenshot before save
  await page.screenshot({ path: 'test-results/debug-before-save.png', fullPage: true });
  
  // Click Save
  await page.getByRole('button', { name: 'Save' }).click();
  
  // Wait to see what happens
  await page.waitForTimeout(5000);
  
  // Take screenshot after save
  await page.screenshot({ path: 'test-results/debug-after-save.png', fullPage: true });
  
  // Check for any error messages
  const pageText = await page.textContent('body');
  console.log('\n=== Page Content (first 1000 chars) ===');
  console.log(pageText?.substring(0, 1000));
  console.log('=======================================\n');
});

// Made with Bob
