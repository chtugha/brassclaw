const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  
  // Navigate to settings
  await page.goto('http://localhost:3000');
  await page.waitForTimeout(2000);
  
  // Click settings
  await page.click('a[href="/settings"]');
  await page.waitForTimeout(1000);
  
  // Capture Inference tab (reference)
  await page.click('button:has-text("Inference")');
  await page.waitForTimeout(1000);
  await page.screenshot({ path: 'screenshots/inference-tab-reference.png', fullPage: true });
  console.log('✓ Captured Inference tab');
  
  // Capture Tools tab
  await page.click('button:has-text("Tools")');
  await page.waitForTimeout(1000);
  await page.screenshot({ path: 'screenshots/tools-tab-issue.png', fullPage: true });
  console.log('✓ Captured Tools tab');
  
  // Capture Safety tab
  await page.click('button:has-text("Safety")');
  await page.waitForTimeout(1000);
  await page.screenshot({ path: 'screenshots/safety-tab-issue.png', fullPage: true });
  console.log('✓ Captured Safety tab');
  
  await browser.close();
  console.log('\n✓ All screenshots captured');
})();
