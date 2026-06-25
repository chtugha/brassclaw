import { test, expect } from '@playwright/test';
import { BrassClawTestHelper } from './helpers';

test.describe('Debug Settings Navigation', () => {
  test('inspect settings page structure', async ({ page }) => {
    const helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
    
    // Navigate to Settings
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);
    
    // Take screenshot
    await page.screenshot({ path: 'screenshots/settings-page-structure.png', fullPage: true });
    
    // Get all links on the page
    const allLinks = await page.locator('a').all();
    console.log(`\nFound ${allLinks.length} links on settings page:`);
    
    for (const link of allLinks) {
      const href = await link.getAttribute('href');
      const text = await link.textContent();
      console.log(`  - href="${href}" text="${text?.trim()}"`);
    }
    
    // Get all buttons
    const allButtons = await page.locator('button').all();
    console.log(`\nFound ${allButtons.length} buttons on settings page:`);
    
    for (const button of allButtons) {
      const text = await button.textContent();
      const classes = await button.getAttribute('class');
      console.log(`  - text="${text?.trim()}" class="${classes}"`);
    }
    
    // Check for any element containing "Safety"
    const safetyElements = await page.locator('*:has-text("Safety")').all();
    console.log(`\nFound ${safetyElements.length} elements containing "Safety":`);
    
    for (const el of safetyElements) {
      const tagName = await el.evaluate(e => e.tagName);
      const text = await el.textContent();
      const href = await el.getAttribute('href');
      console.log(`  - <${tagName}> href="${href}" text="${text?.trim()}"`);
    }
  });
});

// Made with Bob
