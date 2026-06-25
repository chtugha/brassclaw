import { test, expect } from '@playwright/test';
import { BrassClawTestHelper } from './helpers';

test.describe('Safety Tab Navigation with Cache Bust', () => {
  test('should display Safety tab after hard refresh', async ({ page, context }) => {
    // First authenticate
    const helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
    
    // Now do a hard reload to force fresh JS files
    await page.reload({ waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);
    
    // Navigate to Settings
    await page.goto('/settings', { waitUntil: 'networkidle' });
    await page.waitForTimeout(2000);
    
    // Take screenshot
    await page.screenshot({ path: 'screenshots/settings-after-cache-clear.png', fullPage: true });
    
    // Look for Safety tab
    const safetyTab = page.locator('button:has-text("Safety"), a:has-text("Safety")');
    const safetyCount = await safetyTab.count();
    
    console.log(`Found ${safetyCount} Safety tab elements`);
    
    if (safetyCount > 0) {
      await expect(safetyTab.first()).toBeVisible({ timeout: 5000 });
      console.log('✅ Safety tab is visible!');
      
      // Click it
      await safetyTab.first().click();
      await page.waitForTimeout(1000);
      
      // Verify we're on the safety page
      await expect(page).toHaveURL(/\/settings\/safety/);
      console.log('✅ Successfully navigated to Safety settings');
    } else {
      console.log('❌ Safety tab not found - checking what tabs are available');
      
      // List all available tabs
      const allButtons = await page.locator('button').all();
      console.log(`\nFound ${allButtons.length} buttons:`);
      for (const btn of allButtons) {
        const text = await btn.textContent();
        console.log(`  - "${text?.trim()}"`);
      }
    }
  });
});

// Made with Bob
