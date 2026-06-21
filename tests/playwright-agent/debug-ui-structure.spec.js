const { test } = require('@playwright/test');

test('debug UI structure', async ({ page }) => {
  // Set auth token
  process.env.BRASSCLAW_REBORN_WEBUI_TOKEN = 'test-playwright-token';
  
  // Navigate to the app
  await page.goto('http://127.0.0.1:3000');
  
  // Wait for page to load
  await page.waitForTimeout(3000);
  
  // Check if we need to authenticate
  const tokenInput = page.locator('input[placeholder*="token"], input[placeholder*="auth"]');
  const isLoginPage = await tokenInput.isVisible().catch(() => false);
  
  if (isLoginPage) {
    await tokenInput.fill('test-playwright-token');
    const connectButton = page.locator('button:has-text("Connect")');
    await connectButton.click();
    await page.waitForTimeout(3000);
  }
  
  // Navigate to settings
  await page.click('a[href*="/settings"]');
  await page.waitForTimeout(2000);
  
  // Get all buttons on the page
  const buttons = await page.$$eval('button', btns => 
    btns.map(b => ({
      text: b.textContent?.trim(),
      class: b.className,
      visible: b.offsetParent !== null
    }))
  );
  
  console.log('=== ALL BUTTONS ===');
  console.log(JSON.stringify(buttons, null, 2));
  
  // Get page HTML
  const html = await page.content();
  console.log('\n=== PAGE HTML (first 5000 chars) ===');
  console.log(html.substring(0, 5000));
  
  // Take screenshot
  await page.screenshot({ path: 'debug-ui-structure.png', fullPage: true });
  
  console.log('\n=== Screenshot saved to debug-ui-structure.png ===');
});

// Made with Bob
