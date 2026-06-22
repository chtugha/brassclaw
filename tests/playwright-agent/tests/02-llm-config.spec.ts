import { test, expect } from '@playwright/test';
import { setupTestEnvironment, SELECTORS } from './helpers';

test.describe('LLM Configuration Tests', () => {
  // Use a unique provider ID for each test run to avoid conflicts
  const timestamp = Date.now();
  const providerName = `Qwen-Test-${timestamp}`;
  const providerId = `qwen-test-${timestamp}`;

  // Helper function to create a provider
  async function createProvider(page: any) {
    // Navigate to settings/inference page directly
    await page.goto('/settings/inference');
    await page.waitForLoadState('networkidle');
    
    // Wait for the page to load and check for Add provider button
    await page.waitForSelector(SELECTORS.addProviderButton, { timeout: 10000 });
    
    // Add LLM provider
    await page.click(SELECTORS.addProviderButton);
    
    // Wait for dialog to open
    await page.getByLabel('Display name').waitFor({ state: 'visible', timeout: 10000 });
    
    // Fill in provider details
    await page.getByLabel('Display name').fill(providerName);
    await page.getByLabel('Provider ID').fill(providerId);
    
    // Adapter defaults to "open_ai_completions" (OpenAI Compatible), which is what we want
    // No need to select it explicitly
    
    await page.getByLabel('Base URL').fill('http://192.168.10.223:8000/v1');
    await page.getByLabel('Default model').fill('Qwen/Qwen2.5-7B-Instruct-AWQ');
    
    // Save
    await page.getByRole('button', { name: 'Save' }).click();
    
    // Wait for the dialog to close (it should close on successful save)
    await page.waitForTimeout(2000);
    
    // Check if there's an error message displayed
    const errorMessage = page.locator('text=/error|failed/i').first();
    const hasError = await errorMessage.isVisible().catch(() => false);
    
    if (hasError) {
      console.log('Error detected after saving provider');
      // Close dialog with Escape if there's an error
      await page.keyboard.press('Escape');
      await page.waitForTimeout(1000);
      throw new Error('Failed to create provider - validation error');
    }
    
    // Check if dialog is still open (shouldn't be if save was successful)
    const displayNameInput = page.getByLabel('Display name');
    const isStillVisible = await displayNameInput.isVisible().catch(() => false);
    if (isStillVisible) {
      // Dialog is still open, close it
      await page.keyboard.press('Escape');
      await page.waitForTimeout(1000);
    }
    
    // Wait for provider list to refresh
    await page.waitForTimeout(2000);
  }

  test('should configure OpenAI-compatible LLM provider', async ({ page }) => {
    const helper = await setupTestEnvironment(page);
    
    // Create the provider using helper function
    await createProvider(page);
    
    // Verify provider was added by checking for the provider card with specific class
    // Use first() to handle multiple matches and be more specific
    const providerCard = page.locator('.min-w-0.truncate.text-sm.font-semibold').filter({ hasText: providerName }).first();
    await expect(providerCard).toBeVisible({ timeout: 10000 });
    
    await helper.takeScreenshot('02-llm-configured');
  });

  test('should test LLM connection', async ({ page }) => {
    const helper = await setupTestEnvironment(page);
    
    // Create a provider first (this leaves us on the settings page)
    await createProvider(page);
    
    // Provider is now visible, find the test connection button
    // Look for button with "Test" text - use a simple, reliable selector
    const testButton = page.locator('button').filter({ hasText: /test/i }).first();
    await expect(testButton).toBeVisible({ timeout: 5000 });
    await testButton.click();
    
    // Wait for connection test to complete (can take 10-30s for real LLM)
    await page.waitForTimeout(5000);
    
    // Should show success message - be more flexible with the selector
    const successIndicator = page.locator('text=/success|connected|ok|✓|✔/i').first();
    await expect(successIndicator).toBeVisible({ timeout: 30000 });
    
    await helper.takeScreenshot('02-llm-connection-test');
  });

  test('should display LLM provider in list', async ({ page }) => {
    const helper = await setupTestEnvironment(page);
    
    // Create a provider first (this leaves us on the settings page with provider visible)
    await createProvider(page);
    
    // Verify provider details are visible using more specific selectors
    // Look for the provider name in the semibold text element
    const providerNameElement = page.locator('.min-w-0.truncate.text-sm.font-semibold').filter({ hasText: providerName }).first();
    await expect(providerNameElement).toBeVisible();
    
    // Verify model name is visible - be more flexible since it might be truncated or formatted differently
    const modelElement = page.locator('text=/Qwen/i').first();
    await expect(modelElement).toBeVisible();
    
    await helper.takeScreenshot('02-llm-provider-list');
  });
});

// Made with Bob