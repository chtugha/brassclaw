const { test, expect } = require('@playwright/test');

test.describe('BrassClaw v0.29.2 Remote Server Tests', () => {
  const baseURL = 'http://192.168.10.219:3000';

  test('health endpoint responds with HTML', async ({ request }) => {
    const response = await request.get(`${baseURL}/health`);
    expect(response.ok()).toBeTruthy();
    const text = await response.text();
    expect(text).toContain('<!DOCTYPE html>');
    expect(text).toContain('BrassClaw');
    console.log('✅ Health endpoint responds correctly');
  });

  test('frontend loads successfully', async ({ page }) => {
    await page.goto(baseURL);
    await expect(page.locator('body')).toBeVisible();
    
    // Take screenshot
    await page.screenshot({ path: 'remote-server-v0.29.2-homepage.png' });
    console.log('✅ Frontend loaded and screenshot saved');
  });

  test('no JavaScript errors on page load', async ({ page }) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    
    await page.goto(baseURL);
    await page.waitForLoadState('networkidle');
    
    if (errors.length > 0) {
      console.log('JavaScript errors found:', errors);
    }
    expect(errors).toHaveLength(0);
    console.log('✅ No JavaScript errors detected');
  });

  test('page title is correct', async ({ page }) => {
    await page.goto(baseURL);
    await expect(page).toHaveTitle(/BrassClaw/);
    console.log('✅ Page title is correct');
  });

  test('service is accessible from network', async ({ request }) => {
    const response = await request.get(baseURL);
    expect(response.status()).toBe(200);
    console.log('✅ Service is accessible from network');
  });
});

// Made with Bob
