import { test } from '@playwright/test';
import { setupTestEnvironment, SELECTORS } from './helpers';

test.describe('Debug Save Response', () => {
  test('capture page state after clicking save', async ({ page }) => {
    // Setup test environment with auth
    await setupTestEnvironment(page);
    
    // Navigate to settings/inference page directly
    await page.goto('/settings/inference');
    await page.waitForLoadState('networkidle');
    
    // Wait for the page to load
    await page.waitForSelector(SELECTORS.addProviderButton, { timeout: 10000 });
    
    // Add LLM provider
    await page.click(SELECTORS.addProviderButton);
    
    // Wait for dialog to open
    await page.getByLabel('Display name').waitFor({ state: 'visible', timeout: 10000 });
    
    // Fill in provider details
    await page.getByLabel('Display name').fill('Debug Test Provider');
    await page.getByLabel('Provider ID').fill('debug-test-provider');
    await page.getByLabel('Base URL').fill('http://192.168.10.171:8000/v1');
    await page.getByLabel('Default model').fill('Qwen/Qwen2.5-7B-Instruct-AWQ');
    
    console.log('\n=== BEFORE CLICKING SAVE ===');
    await page.screenshot({ path: 'test-results/before-save.png', fullPage: true });
    
    // Click Save
    await page.getByRole('button', { name: 'Save' }).click();
    
    // Wait for response
    await page.waitForTimeout(3000);
    
    console.log('\n=== AFTER CLICKING SAVE ===');
    await page.screenshot({ path: 'test-results/after-save.png', fullPage: true });
    
    // Get all visible text on the page
    const bodyText = await page.locator('body').textContent();
    console.log('\n=== FULL PAGE TEXT ===');
    console.log(bodyText);
    
    // Look for specific error-related elements
    const errorLocators = [
      'text=/error/i',
      'text=/failed/i',
      'text=/invalid/i',
      '[role="alert"]',
      '.error',
      '.alert-error'
    ];
    
    console.log('\n=== CHECKING ERROR ELEMENTS ===');
    for (const selector of errorLocators) {
      const elements = await page.locator(selector).all();
      if (elements.length > 0) {
        console.log(`\nFound ${elements.length} elements matching "${selector}":`);
        for (let i = 0; i < elements.length; i++) {
          const text = await elements[i].textContent();
          const isVisible = await elements[i].isVisible();
          console.log(`  ${i + 1}. Visible: ${isVisible}, Text: "${text}"`);
        }
      }
    }
    
    // Check if dialog is still open
    const displayNameInput = page.getByLabel('Display name');
    const dialogStillOpen = await displayNameInput.isVisible().catch(() => false);
    console.log(`\n=== DIALOG STATUS ===`);
    console.log(`Dialog still open: ${dialogStillOpen}`);
  });
});

// Made with Bob
