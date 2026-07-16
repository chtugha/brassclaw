import { test, expect } from '@playwright/test';
import { setupTestEnvironment } from './helpers';

test.describe('Debug Error Message', () => {
  test('capture exact error message when creating provider', async ({ page }) => {
    // Setup test environment with auth
    await setupTestEnvironment(page);
    
    // Navigate to settings/inference page
    await page.goto('http://localhost:3000/settings/inference');
    await page.waitForLoadState('networkidle');
    
    // Wait for page to be ready
    await page.waitForTimeout(2000);
    
    // Click "Add Provider" button
    await page.getByRole('button', { name: /add provider/i }).click();
    
    // Wait for dialog
    await page.waitForSelector('input[name="display_name"]', { state: 'visible', timeout: 5000 });
    
    // Fill in provider details
    await page.fill('input[name="display_name"]', 'Test OpenAI Provider');
    await page.selectOption('select[name="adapter"]', 'open_ai_completions');
    await page.fill('input[name="base_url"]', 'http://192.168.10.171:8000/v1');
    await page.fill('input[name="default_model"]', 'Qwen/Qwen2.5-7B-Instruct-AWQ');
    
    // Take screenshot before save
    await page.screenshot({ path: 'test-results/before-save.png', fullPage: true });
    
    // Click Save
    await page.getByRole('button', { name: 'Save' }).click();
    
    // Wait a bit for any error to appear
    await page.waitForTimeout(3000);
    
    // Take screenshot after save
    await page.screenshot({ path: 'test-results/after-save.png', fullPage: true });
    
    // Get all text content from the page
    const pageText = await page.textContent('body');
    console.log('=== FULL PAGE TEXT ===');
    console.log(pageText);
    
    // Look for any error elements
    const errorElements = await page.locator('[class*="error"], [class*="Error"], [role="alert"]').all();
    console.log(`\n=== FOUND ${errorElements.length} ERROR ELEMENTS ===`);
    for (let i = 0; i < errorElements.length; i++) {
      const text = await errorElements[i].textContent();
      const html = await errorElements[i].innerHTML();
      console.log(`\nError Element ${i + 1}:`);
      console.log('Text:', text);
      console.log('HTML:', html);
    }
    
    // Look for elements containing "error", "failed", "invalid"
    const errorTexts = await page.locator('text=/error|failed|invalid/i').all();
    console.log(`\n=== FOUND ${errorTexts.length} ELEMENTS WITH ERROR TEXT ===`);
    for (let i = 0; i < errorTexts.length; i++) {
      const text = await errorTexts[i].textContent();
      console.log(`Element ${i + 1}:`, text);
    }
    
    // Check network responses
    console.log('\n=== CHECKING NETWORK RESPONSES ===');
    page.on('response', async (response) => {
      if (response.url().includes('/llm/providers')) {
        console.log('LLM Provider API Response:');
        console.log('Status:', response.status());
        console.log('URL:', response.url());
        try {
          const body = await response.text();
          console.log('Body:', body);
        } catch (e) {
          console.log('Could not read response body');
        }
      }
    });
  });
});

// Made with Bob
