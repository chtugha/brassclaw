import { test, expect } from '@playwright/test';
import { setupTestEnvironment, SELECTORS } from './helpers';

test.describe('LLM Configuration Tests', () => {
  test('should configure OpenAI-compatible LLM provider', async ({ page }) => {
    const helper = await setupTestEnvironment(page);
    
    // Navigate to settings/providers
    await helper.navigateToSettings();
    await page.click(SELECTORS.providersTab);
    
    // Add LLM provider
    await page.click(SELECTORS.addProviderButton);
    
    // Fill in provider details
    await page.fill('input[name="name"]', 'Qwen-Test');
    await page.selectOption('select[name="type"]', 'openai-compatible');
    await page.fill('input[name="base_url"]', 'http://192.168.10.223:8000/v1');
    await page.fill('input[name="model"]', 'Qwen/Qwen2.5-7B-Instruct-AWQ');
    
    // No API key needed - leave empty or skip
    // await page.fill('input[name="api_key"]', ''); // Optional: explicitly clear
    
    await page.click('button:has-text("Save")');
    
    // Verify provider was added
    await expect(page.locator('text=Qwen-Test')).toBeVisible({ timeout: 10000 });
    
    await helper.takeScreenshot('02-llm-configured');
  });

  test('should test LLM connection', async ({ page }) => {
    const helper = await setupTestEnvironment(page);
    
    await helper.navigateToSettings();
    await page.click(SELECTORS.providersTab);
    
    // Find the Qwen provider and test connection
    const testButton = page.locator('button:has-text("Test Connection")').first();
    await testButton.click();
    
    // Wait for connection test result
    await page.waitForTimeout(3000);
    
    // Should show success message
    await expect(page.locator('text=/success|connected|ok/i')).toBeVisible({ timeout: 10000 });
    
    await helper.takeScreenshot('02-llm-connection-test');
  });

  test('should display LLM provider in list', async ({ page }) => {
    const helper = await setupTestEnvironment(page);
    
    await helper.navigateToSettings();
    await page.click(SELECTORS.providersTab);
    
    // Verify provider details are visible
    await expect(page.locator('text=Qwen-Test')).toBeVisible();
    await expect(page.locator('text=/Qwen.*2.5.*7B/i')).toBeVisible();
    
    await helper.takeScreenshot('02-llm-provider-list');
  });
});

// Made with Bob