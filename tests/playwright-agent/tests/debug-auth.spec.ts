import { test, expect } from '@playwright/test';

test('debug auth token flow', async ({ page }) => {
  // Enable console logging
  page.on('console', msg => console.log('BROWSER:', msg.text()));
  
  // Step 1: Navigate to homepage
  console.log('Step 1: Navigate to homepage');
  await page.goto('http://127.0.0.1:3000/');
  await page.waitForTimeout(2000);
  
  // Step 2: Check sessionStorage before injection
  console.log('Step 2: Check sessionStorage before injection');
  const tokenBefore = await page.evaluate(() => {
    return sessionStorage.getItem('brassclaw_token');
  });
  console.log('Token before injection:', tokenBefore);
  
  // Step 3: Inject token
  console.log('Step 3: Inject token');
  await page.evaluate(() => {
    sessionStorage.setItem('brassclaw_token', 'test-playwright-token');
    console.log('Token injected into sessionStorage');
  });
  
  // Step 4: Verify token was set
  console.log('Step 4: Verify token was set');
  const tokenAfter = await page.evaluate(() => {
    return sessionStorage.getItem('brassclaw_token');
  });
  console.log('Token after injection:', tokenAfter);
  
  // Step 5: Reload page
  console.log('Step 5: Reload page');
  await page.reload();
  await page.waitForTimeout(2000);
  
  // Step 6: Check if token persists after reload
  console.log('Step 6: Check if token persists after reload');
  const tokenAfterReload = await page.evaluate(() => {
    return sessionStorage.getItem('brassclaw_token');
  });
  console.log('Token after reload:', tokenAfterReload);
  
  // Step 7: Check if we're logged in
  console.log('Step 7: Check if we are logged in');
  const settingsLink = page.locator('a[href*="/settings"]');
  const isVisible = await settingsLink.isVisible().catch(() => false);
  console.log('Settings link visible:', isVisible);
  
  if (!isVisible) {
    // Take screenshot for debugging
    await page.screenshot({ path: 'debug-auth-failure.png', fullPage: true });
    console.log('Screenshot saved to debug-auth-failure.png');
    
    // Check what's on the page
    const pageContent = await page.content();
    console.log('Page title:', await page.title());
    console.log('Current URL:', page.url());
  }
  
  expect(isVisible).toBe(true);
});

// Made with Bob
