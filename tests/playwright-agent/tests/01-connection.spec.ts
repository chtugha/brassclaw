import { test, expect } from '@playwright/test';
import { setupTestEnvironment, SELECTORS } from './helpers';

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
    
    // Check for main navigation elements using SELECTORS
    const settingsLink = page.locator(SELECTORS.settingsLink);
    
    // Verify settings link is visible
    await expect(settingsLink).toBeVisible({ timeout: 10000 });
  });

  test('should load without console errors', async ({ page }) => {
    const errors: string[] = [];
    const criticalErrors: string[] = [];
    
    page.on('console', msg => {
      if (msg.type() === 'error') {
        const text = msg.text();
        errors.push(text);
        
        // Only track critical errors (not warnings about missing features)
        if (!text.includes('Failed to load') &&
            !text.includes('Invalid or missing auth token') &&
            !text.includes('404') &&
            !text.includes('favicon')) {
          criticalErrors.push(text);
        }
      }
    });
    
    await setupTestEnvironment(page);
    
    // Allow some time for any async errors
    await page.waitForTimeout(2000);
    
    // Log all errors for debugging
    if (errors.length > 0) {
      console.log('Console errors detected:', errors);
    }
    
    // Should have no critical errors (allow non-critical ones)
    expect(criticalErrors.length).toBe(0);
  });
});

// Made with Bob
