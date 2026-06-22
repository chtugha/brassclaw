import { test } from '@playwright/test';
import { setupTestEnvironment, SELECTORS } from './helpers';

test('Debug adapter dropdown options', async ({ page }) => {
  const helper = await setupTestEnvironment(page);
  
  // Navigate to settings/inference
  await page.goto('/settings/inference');
  await page.waitForLoadState('networkidle');
  
  // Wait for Add provider button
  await page.waitForSelector(SELECTORS.addProviderButton, { timeout: 10000 });
  
  // Click Add provider
  await page.click(SELECTORS.addProviderButton);
  
  // Wait for dialog
  await page.getByLabel('Display name').waitFor({ state: 'visible', timeout: 10000 });
  
  // Get all options from the Adapter dropdown
  const options = await page.evaluate(() => {
    const select = document.querySelector('select') as HTMLSelectElement;
    if (!select) return [];
    
    const opts = Array.from(select.options).map(opt => ({
      value: opt.value,
      text: opt.textContent?.trim() || '',
      selected: opt.selected
    }));
    
    console.log('Adapter options:', JSON.stringify(opts, null, 2));
    return opts;
  });
  
  console.log('\n=== Adapter Dropdown Options ===');
  options.forEach(opt => {
    console.log(`Value: "${opt.value}" | Text: "${opt.text}" | Selected: ${opt.selected}`);
  });
  console.log('================================\n');
  
  // Take screenshot
  await page.screenshot({ path: 'test-results/debug-adapter-dropdown.png', fullPage: true });
});

// Made with Bob
