import { test, expect } from '@playwright/test';
import { setupTestEnvironment } from './helpers';

test.describe('BrassClaw Connection Tests', () => {
  test('should connect to brassclaw webfrontend-ui', async ({ page }) => {
    const helper = await setupTestEnvironment(page);
    
    // Verify page loaded
    await expect(page).toHaveTitle(/BrassClaw|Brass Claw/i);
    
    // Take screenshot for documentation
    await helper.takeScreenshot('01-homepage');
  });

  test('should have navigation elements', async ({ page }) => {
    await setupTestEnvironment(page);
    
    // Check for main navigation elements
    const homeLink = page.locator('a[href="/"]');
    const settingsLink = page.locator('a[href="/settings"]');
    
    await expect(homeLink).toBeVisible();
    await expect(settingsLink).toBeVisible();
  });

  test('should load without console errors', async ({ page }) => {
    const errors: string[] = [];
    
    page.on('console', msg => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });
    
    await setupTestEnvironment(page);
    
    // Allow some time for any async errors
    await page.waitForTimeout(2000);
    
    // Should have no critical errors
    expect(errors.length).toBe(0);
  });
});

// Made with Bob
