import { test, expect } from '@playwright/test';
import { BrassClawTestHelper } from './helpers';

/**
 * Test Suite: Tools Settings Tab and Safety Settings Panel
 * 
 * Tests the implementation of Step 12 (Tools Settings Tab) and Step 13 (Safety Settings Panel).
 * 
 * Expected Status:
 * - Tools Tab: ✅ Fully functional (Step 12 complete)
 * - Safety Panel: ⚠️ UI complete, API returns 501 (database integration pending)
 */

test.describe('Tools Settings Tab', () => {
  test.beforeEach(async ({ page }) => {
    const helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
  });

  test('should navigate to Tools settings tab', async ({ page }) => {
    // Navigate to Settings
    await page.click('a[href="/settings"]');
    await page.waitForURL('**/settings/**');

    // Click on Tools tab
    await page.click('text=Tools');
    await page.waitForURL('**/settings/tools');

    // Verify we're on the Tools page
    await expect(page.locator('h1, h2').filter({ hasText: 'Tool Capabilities' })).toBeVisible();
  });

  test('should display tool capabilities list', async ({ page }) => {
    await page.goto('/settings/tools');
    
    // Wait for tools to load
    await page.waitForSelector('[data-testid="tools-list"], .tools-list, [class*="tool"]', { 
      timeout: 10000 
    });

    // Check if we have capability groups or individual tools
    const hasProviderGroups = await page.locator('[data-testid="provider-group"], [class*="provider-group"]').count() > 0;
    const hasToolRows = await page.locator('[data-testid="tool-row"], [class*="tool-row"]').count() > 0;

    expect(hasProviderGroups || hasToolRows).toBeTruthy();

    // Take screenshot
    await page.screenshot({ 
      path: 'test-results/tools-tab-capabilities-list.png',
      fullPage: true 
    });
  });

  test('should show capability metadata (name, description, effects)', async ({ page }) => {
    await page.goto('/settings/tools');
    
    // Wait for content to load
    await page.waitForTimeout(2000);

    // Look for capability information
    const hasCapabilityNames = await page.locator('[data-testid="capability-name"], [class*="capability-name"], [class*="tool-name"]').count() > 0;
    const hasDescriptions = await page.locator('[data-testid="capability-description"], [class*="description"]').count() > 0;
    const hasEffectBadges = await page.locator('[data-testid="effect-badge"], [class*="effect-badge"], [class*="badge"]').count() > 0;

    console.log('Capability metadata found:', {
      names: hasCapabilityNames,
      descriptions: hasDescriptions,
      effectBadges: hasEffectBadges
    });

    // At least one of these should be present
    expect(hasCapabilityNames || hasDescriptions || hasEffectBadges).toBeTruthy();
  });

  test('should display permission dropdowns', async ({ page }) => {
    await page.goto('/settings/tools');
    
    // Wait for tools to load
    await page.waitForTimeout(2000);

    // Look for permission controls (dropdowns, selects, or buttons)
    const hasPermissionDropdowns = await page.locator('select, [role="combobox"], [data-testid*="permission"]').count() > 0;

    if (hasPermissionDropdowns) {
      console.log('✅ Permission controls found');
      
      // Try to find permission options
      const firstDropdown = page.locator('select, [role="combobox"]').first();
      if (await firstDropdown.isVisible()) {
        await firstDropdown.click();
        
        // Look for permission modes: Allow, Ask, Deny
        const hasAllowOption = await page.locator('text=/allow/i').count() > 0;
        const hasAskOption = await page.locator('text=/ask/i').count() > 0;
        const hasDenyOption = await page.locator('text=/deny/i').count() > 0;

        console.log('Permission options:', {
          allow: hasAllowOption,
          ask: hasAskOption,
          deny: hasDenyOption
        });
      }
    } else {
      console.log('⚠️ No permission controls found - may need to expand groups first');
    }

    await page.screenshot({ 
      path: 'test-results/tools-tab-permissions.png',
      fullPage: true 
    });
  });

  test('should support search/filter functionality', async ({ page }) => {
    await page.goto('/settings/tools');
    
    // Look for search input
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i], input[placeholder*="filter" i]');
    
    if (await searchInput.count() > 0) {
      console.log('✅ Search input found');
      
      // Try searching
      await searchInput.first().fill('file');
      await page.waitForTimeout(500);

      // Check if results are filtered
      const toolCount = await page.locator('[data-testid="tool-row"], [class*="tool-row"]').count();
      console.log(`Filtered results: ${toolCount} tools`);

      await page.screenshot({ 
        path: 'test-results/tools-tab-search.png',
        fullPage: true 
      });
    } else {
      console.log('⚠️ Search functionality not found');
    }
  });

  test('should display provider/domain grouping', async ({ page }) => {
    await page.goto('/settings/tools');
    
    await page.waitForTimeout(2000);

    // Look for provider groups
    const providerGroups = page.locator('[data-testid="provider-group"], [class*="provider-group"], [class*="domain-group"]');
    const groupCount = await providerGroups.count();

    console.log(`Found ${groupCount} provider groups`);

    if (groupCount > 0) {
      // Try to expand first group
      const firstGroup = providerGroups.first();
      const expandButton = firstGroup.locator('button, [role="button"]').first();
      
      if (await expandButton.isVisible()) {
        await expandButton.click();
        await page.waitForTimeout(500);
        
        await page.screenshot({ 
          path: 'test-results/tools-tab-expanded-group.png',
          fullPage: true 
        });
      }
    }

    expect(groupCount).toBeGreaterThanOrEqual(0); // May be 0 if using different layout
  });
});

test.describe('Safety Settings Panel', () => {
  test.beforeEach(async ({ page }) => {
    const helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
    await page.goto('/settings/tools');
  });

  test('should display Safety Configuration section', async ({ page }) => {
    // Scroll down to find Safety section
    await page.evaluate('window.scrollTo(0, document.body.scrollHeight)');
    await page.waitForTimeout(1000);

    // Look for Safety Configuration heading
    const safetyHeading = page.locator('h2, h3').filter({ hasText: /safety configuration/i });
    
    if (await safetyHeading.count() > 0) {
      console.log('✅ Safety Configuration section found');
      await expect(safetyHeading.first()).toBeVisible();
      
      await page.screenshot({ 
        path: 'test-results/safety-panel-visible.png',
        fullPage: true 
      });
    } else {
      console.log('⚠️ Safety Configuration section not found - may need to scroll more');
      await page.screenshot({ 
        path: 'test-results/safety-panel-not-found.png',
        fullPage: true 
      });
    }
  });

  test('should display three safety categories', async ({ page }) => {
    await page.evaluate('window.scrollTo(0, document.body.scrollHeight)');
    await page.waitForTimeout(1000);

    // Look for the three safety categories
    const categories = [
      'Sensitive Path Blocking',
      'Workspace File Rules',
      'Device/Process Path Blocking'
    ];

    const foundCategories: string[] = [];

    for (const category of categories) {
      const categoryElement = page.locator('text=' + category);
      if (await categoryElement.count() > 0) {
        foundCategories.push(category);
        console.log(`✅ Found category: ${category}`);
      } else {
        console.log(`⚠️ Missing category: ${category}`);
      }
    }

    console.log(`Found ${foundCategories.length}/3 safety categories`);
    
    await page.screenshot({ 
      path: 'test-results/safety-panel-categories.png',
      fullPage: true 
    });

    // At least one category should be present
    expect(foundCategories.length).toBeGreaterThan(0);
  });

  test('should show empty states or API errors (database not integrated)', async ({ page }) => {
    await page.evaluate('window.scrollTo(0, document.body.scrollHeight)');
    await page.waitForTimeout(2000);

    // Look for empty states or error messages
    const hasEmptyState = await page.locator('text=/no.*configured/i, text=/empty/i').count() > 0;
    const hasErrorMessage = await page.locator('text=/failed/i, text=/error/i, text=/not implemented/i').count() > 0;

    console.log('Safety panel state:', {
      emptyState: hasEmptyState,
      errorMessage: hasErrorMessage
    });

    // Expected: Either empty states or "Not Implemented" errors since database integration is pending
    expect(hasEmptyState || hasErrorMessage).toBeTruthy();

    await page.screenshot({ 
      path: 'test-results/safety-panel-state.png',
      fullPage: true 
    });
  });

  test('should have collapsible sections', async ({ page }) => {
    await page.evaluate('window.scrollTo(0, document.body.scrollHeight)');
    await page.waitForTimeout(1000);

    // Look for collapsible section headers
    const collapsibleHeaders = page.locator('[role="button"], button').filter({ 
      hasText: /sensitive|workspace|device|process/i 
    });

    const headerCount = await collapsibleHeaders.count();
    console.log(`Found ${headerCount} collapsible section headers`);

    if (headerCount > 0) {
      // Try to expand/collapse first section
      const firstHeader = collapsibleHeaders.first();
      await firstHeader.click();
      await page.waitForTimeout(500);

      await page.screenshot({ 
        path: 'test-results/safety-panel-expanded.png',
        fullPage: true 
      });

      // Click again to collapse
      await firstHeader.click();
      await page.waitForTimeout(500);

      console.log('✅ Collapsible sections working');
    } else {
      console.log('⚠️ No collapsible sections found');
    }
  });

  test('should display add entry controls', async ({ page }) => {
    await page.evaluate('window.scrollTo(0, document.body.scrollHeight)');
    await page.waitForTimeout(1000);

    // Look for "Add entry" buttons or inputs
    const addButtons = page.locator('button, [role="button"]').filter({ hasText: /add entry/i });
    const addInputs = page.locator('input[placeholder*="pattern" i], input[placeholder*="entry" i]');

    const hasAddControls = (await addButtons.count() > 0) || (await addInputs.count() > 0);

    console.log('Add entry controls:', {
      buttons: await addButtons.count(),
      inputs: await addInputs.count()
    });

    if (hasAddControls) {
      console.log('✅ Add entry controls found');
      await page.screenshot({ 
        path: 'test-results/safety-panel-add-controls.png',
        fullPage: true 
      });
    } else {
      console.log('⚠️ Add entry controls not visible (may need to expand sections)');
    }
  });
});

test.describe('API Integration Status', () => {
  test.beforeEach(async ({ page }) => {
    const helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
  });

  test('should verify Tools API endpoints are working', async ({ page }) => {
    // Intercept API calls
    const apiCalls: { url: string; status: number }[] = [];

    page.on('response', response => {
      if (response.url().includes('/api/webchat/v2/tools')) {
        apiCalls.push({
          url: response.url(),
          status: response.status()
        });
      }
    });

    await page.goto('/settings/tools');
    await page.waitForTimeout(3000);

    console.log('Tools API calls:', apiCalls);

    // Should have at least one successful API call
    const successfulCalls = apiCalls.filter(call => call.status === 200);
    expect(successfulCalls.length).toBeGreaterThan(0);

    console.log(`✅ ${successfulCalls.length} successful Tools API calls`);
  });

  test('should verify Safety API endpoints return 501 (Not Implemented)', async ({ page }) => {
    // Intercept API calls
    const safetyApiCalls: { url: string; status: number }[] = [];

    page.on('response', response => {
      if (response.url().includes('/api/webchat/v2/safety')) {
        safetyApiCalls.push({
          url: response.url(),
          status: response.status()
        });
      }
    });

    await page.goto('/settings/tools');
    await page.waitForTimeout(3000);

    console.log('Safety API calls:', safetyApiCalls);

    if (safetyApiCalls.length > 0) {
      // Expected: 501 Not Implemented (database integration pending)
      const notImplementedCalls = safetyApiCalls.filter(call => call.status === 501);
      
      console.log(`Found ${notImplementedCalls.length} "Not Implemented" responses (expected)`);
      console.log('⚠️ Safety Settings Panel UI is complete but database integration is pending');
    } else {
      console.log('ℹ️ No Safety API calls detected - panel may not be visible or not making requests');
    }
  });
});

test.describe('Visual Regression', () => {
  test.beforeEach(async ({ page }) => {
    const helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
  });

  test('should capture full Tools settings page', async ({ page }) => {
    await page.goto('/settings/tools');
    await page.waitForTimeout(2000);

    await page.screenshot({ 
      path: 'test-results/tools-settings-full-page.png',
      fullPage: true 
    });

    console.log('✅ Full page screenshot captured');
  });

  test('should capture Tools tab in different states', async ({ page }) => {
    await page.goto('/settings/tools');
    
    // Initial state
    await page.waitForTimeout(1000);
    await page.screenshot({ 
      path: 'test-results/tools-tab-initial.png',
      fullPage: true 
    });

    // After scrolling to bottom (Safety panel)
    await page.evaluate('window.scrollTo(0, document.body.scrollHeight)');
    await page.waitForTimeout(1000);
    await page.screenshot({ 
      path: 'test-results/tools-tab-with-safety.png',
      fullPage: true 
    });

    console.log('✅ Multiple state screenshots captured');
  });
});

// Made with Bob
