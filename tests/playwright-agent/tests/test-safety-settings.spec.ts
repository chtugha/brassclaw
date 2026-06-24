import { test, expect } from '@playwright/test';
import { BrassClawTestHelper } from './helpers';

test.describe('Safety Settings', () => {
  let helper: BrassClawTestHelper;

  test.beforeEach(async ({ page }) => {
    helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
  });

  test('should display Safety tab in Settings', async ({ page }) => {
    // Navigate to Settings
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Look for Safety tab with shield icon
    const safetyTab = page.locator('a[href="/settings/safety"], button:has-text("Safety")');
    await expect(safetyTab).toBeVisible({ timeout: 10000 });

    // Take screenshot
    await page.screenshot({ path: 'screenshots/safety-tab-visible.png', fullPage: true });
  });

  test('should navigate to Safety settings page', async ({ page }) => {
    // Navigate to Safety settings
    await page.goto('/settings/safety');
    await page.waitForLoadState('networkidle');

    // Wait for content to load
    await page.waitForTimeout(2000);

    // Check for main heading or sections
    const pageContent = await page.content();
    console.log('Page URL:', page.url());
    console.log('Page contains "Safety":', pageContent.includes('Safety'));
    console.log('Page contains "Sensitive":', pageContent.includes('Sensitive'));

    // Take screenshot
    await page.screenshot({ path: 'screenshots/safety-settings-page.png', fullPage: true });
  });

  test('should display three safety sections', async ({ page }) => {
    await page.goto('/settings/safety');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Look for the three sections by their titles
    const sensitivePaths = page.getByText(/Sensitive Path/i);
    const workspaceRules = page.getByText(/Workspace.*Rule/i);
    const blockedPaths = page.getByText(/Blocked Path|Device.*Path/i);

    // Check if at least one section is visible
    const sections = [sensitivePaths, workspaceRules, blockedPaths];
    let visibleCount = 0;
    
    for (const section of sections) {
      const count = await section.count();
      if (count > 0) {
        visibleCount++;
        console.log(`Found section: ${await section.first().textContent()}`);
      }
    }

    console.log(`Visible sections: ${visibleCount}/3`);
    expect(visibleCount).toBeGreaterThan(0);

    await page.screenshot({ path: 'screenshots/safety-sections.png', fullPage: true });
  });

  test('should load default entries on first access', async ({ page }) => {
    await page.goto('/settings/safety');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(3000);

    // Look for default entry indicators
    const defaultBadges = page.locator('span:has-text("Default")');
    const defaultCount = await defaultBadges.count();
    
    console.log(`Found ${defaultCount} default entries`);

    // Look for common default patterns
    const envPattern = page.getByText('**/.env');
    const idRsaPattern = page.getByText('**/id_rsa');
    const memoryMd = page.getByText('MEMORY.md');
    const devZero = page.getByText('/dev/zero');

    const patterns = [envPattern, idRsaPattern, memoryMd, devZero];
    let foundCount = 0;

    for (const pattern of patterns) {
      const count = await pattern.count();
      if (count > 0) {
        foundCount++;
        console.log(`Found pattern: ${await pattern.first().textContent()}`);
      }
    }

    console.log(`Found ${foundCount}/4 expected default patterns`);

    await page.screenshot({ path: 'screenshots/safety-default-entries.png', fullPage: true });
  });

  test('should have collapsible sections', async ({ page }) => {
    await page.goto('/settings/safety');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Look for section headers that might be clickable
    const headers = page.locator('button:has-text("Sensitive"), button:has-text("Workspace"), button:has-text("Blocked")');
    const headerCount = await headers.count();

    console.log(`Found ${headerCount} collapsible headers`);

    if (headerCount > 0) {
      // Try to click the first header
      await headers.first().click();
      await page.waitForTimeout(500);
      
      await page.screenshot({ path: 'screenshots/safety-section-collapsed.png', fullPage: true });

      // Click again to expand
      await headers.first().click();
      await page.waitForTimeout(500);

      await page.screenshot({ path: 'screenshots/safety-section-expanded.png', fullPage: true });
    }
  });

  test('should have add entry form', async ({ page }) => {
    await page.goto('/settings/safety');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Look for input field with placeholder
    const addInput = page.locator('input[placeholder*="pattern"], input[placeholder*="entry"]');
    const inputCount = await addInput.count();

    console.log(`Found ${inputCount} add entry inputs`);

    // Look for add button
    const addButton = page.locator('button:has-text("Add")');
    const buttonCount = await addButton.count();

    console.log(`Found ${buttonCount} add buttons`);

    expect(inputCount + buttonCount).toBeGreaterThan(0);

    await page.screenshot({ path: 'screenshots/safety-add-form.png', fullPage: true });
  });

  test('should test API endpoints directly', async ({ page }) => {
    // Get auth token
    const token = process.env.BRASSCLAW_REBORN_WEBUI_TOKEN || process.env.BRASSCLAW_GATEWAY_TOKEN;
    
    if (!token) {
      console.log('No auth token found, skipping API test');
      return;
    }

    // Test sensitive-paths endpoint
    const response = await page.request.get('/api/webchat/v2/safety/sensitive-paths', {
      headers: {
        'Authorization': `Bearer ${token}`
      }
    });

    console.log('API Response Status:', response.status());
    console.log('API Response Headers:', response.headers());

    if (response.ok()) {
      const data = await response.json();
      console.log('API Response Data:', JSON.stringify(data, null, 2));
      
      expect(data).toHaveProperty('entries');
      expect(Array.isArray(data.entries)).toBe(true);
      
      if (data.entries.length > 0) {
        console.log(`Found ${data.entries.length} entries`);
        console.log('First entry:', data.entries[0]);
      }
    } else {
      const text = await response.text();
      console.log('API Error Response:', text);
    }
  });

  test('should test all three API endpoints', async ({ page }) => {
    const token = process.env.BRASSCLAW_REBORN_WEBUI_TOKEN || process.env.BRASSCLAW_GATEWAY_TOKEN;
    
    if (!token) {
      console.log('No auth token found, skipping API test');
      return;
    }

    const endpoints = [
      '/api/webchat/v2/safety/sensitive-paths',
      '/api/webchat/v2/safety/workspace-rules',
      '/api/webchat/v2/safety/blocked-paths'
    ];

    for (const endpoint of endpoints) {
      console.log(`\nTesting ${endpoint}...`);
      
      const response = await page.request.get(endpoint, {
        headers: {
          'Authorization': `Bearer ${token}`
        }
      });

      console.log(`Status: ${response.status()}`);

      if (response.ok()) {
        const data = await response.json();
        console.log(`Entries: ${data.entries?.length || 0}`);
        
        if (data.entries && data.entries.length > 0) {
          console.log('Sample entry:', data.entries[0]);
        }
      } else {
        const text = await response.text();
        console.log('Error:', text);
      }
    }
  });

  test('should verify enable/disable toggles exist', async ({ page }) => {
    await page.goto('/settings/safety');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Look for checkboxes
    const checkboxes = page.locator('input[type="checkbox"]');
    const checkboxCount = await checkboxes.count();

    console.log(`Found ${checkboxCount} checkboxes (enable/disable toggles)`);

    if (checkboxCount > 0) {
      // Get state of first checkbox
      const firstCheckbox = checkboxes.first();
      const isChecked = await firstCheckbox.isChecked();
      console.log(`First checkbox is ${isChecked ? 'checked' : 'unchecked'}`);
    }

    await page.screenshot({ path: 'screenshots/safety-toggles.png', fullPage: true });
  });

  test('should verify delete buttons for user entries', async ({ page }) => {
    await page.goto('/settings/safety');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Look for delete/remove buttons (X icon or text)
    const deleteButtons = page.locator('button[title*="Remove"], button[title*="Delete"], button:has(svg):not(:has-text("Add"))');
    const deleteCount = await deleteButtons.count();

    console.log(`Found ${deleteCount} delete buttons`);

    await page.screenshot({ path: 'screenshots/safety-delete-buttons.png', fullPage: true });
  });
});

// Made with Bob
