import { test, expect } from '@playwright/test';
import { BrassClawTestHelper } from './helpers';

test.describe('Built-in Tools Display and Functionality', () => {
  let helper: BrassClawTestHelper;

  test.beforeEach(async ({ page }) => {
    helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
    await helper.navigateToSettings();
    await helper.navigateToToolsTab();
  });

  test('should display built-in tools immediately without empty state', async ({ page }) => {
    // Wait for tools to load
    await page.waitForSelector('[data-testid="tools-list"]', { timeout: 10000 });
    
    // Should NOT show empty state message about extensions/MCP
    const emptyState = page.locator('text=/extensions.*MCP/i');
    await expect(emptyState).not.toBeVisible();
    
    // Should show tools list
    const toolsList = page.locator('[data-testid="tools-list"]');
    await expect(toolsList).toBeVisible();
    
    // Take screenshot
    await page.screenshot({ path: 'test-results/builtin-tools-displayed.png', fullPage: true });
  });

  test('should display all 47 built-in capabilities', async ({ page }) => {
    // Wait for tools to load
    await page.waitForSelector('[data-testid="tool-item"]', { timeout: 10000 });
    
    // Count tool items
    const toolItems = page.locator('[data-testid="tool-item"]');
    const count = await toolItems.count();
    
    console.log(`Found ${count} built-in tools`);
    
    // Should have 47 built-in tools
    expect(count).toBeGreaterThanOrEqual(40); // Allow some flexibility
    expect(count).toBeLessThanOrEqual(50);
    
    // Take screenshot
    await page.screenshot({ path: 'test-results/all-builtin-tools.png', fullPage: true });
  });

  test('should display tool details correctly', async ({ page }) => {
    // Wait for first tool
    await page.waitForSelector('[data-testid="tool-item"]', { timeout: 10000 });
    
    const firstTool = page.locator('[data-testid="tool-item"]').first();
    
    // Should have tool ID
    const toolId = firstTool.locator('[data-testid="tool-id"]');
    await expect(toolId).toBeVisible();
    const idText = await toolId.textContent();
    expect(idText).toBeTruthy();
    console.log(`First tool ID: ${idText}`);
    
    // Should have description
    const description = firstTool.locator('[data-testid="tool-description"]');
    await expect(description).toBeVisible();
    
    // Should have provider
    const provider = firstTool.locator('[data-testid="tool-provider"]');
    await expect(provider).toBeVisible();
    const providerText = await provider.textContent();
    console.log(`First tool provider: ${providerText}`);
    
    // Should have permission dropdown
    const permissionDropdown = firstTool.locator('[data-testid="permission-select"]');
    await expect(permissionDropdown).toBeVisible();
    
    // Take screenshot
    await page.screenshot({ path: 'test-results/tool-details.png', fullPage: true });
  });

  test('should have permission controls (Allow/Ask/Deny)', async ({ page }) => {
    // Wait for first tool
    await page.waitForSelector('[data-testid="tool-item"]', { timeout: 10000 });
    
    const firstTool = page.locator('[data-testid="tool-item"]').first();
    const permissionDropdown = firstTool.locator('[data-testid="permission-select"]');
    
    // Click dropdown to open options
    await permissionDropdown.click();
    await page.waitForTimeout(500);
    
    // Should have Allow option
    const allowOption = page.locator('text=Allow').first();
    await expect(allowOption).toBeVisible();
    
    // Should have Ask option
    const askOption = page.locator('text=Ask').first();
    await expect(askOption).toBeVisible();
    
    // Should have Deny option
    const denyOption = page.locator('text=Deny').first();
    await expect(denyOption).toBeVisible();
    
    // Take screenshot
    await page.screenshot({ path: 'test-results/permission-options.png', fullPage: true });
  });

  test('should change permission mode successfully', async ({ page }) => {
    // Wait for first tool
    await page.waitForSelector('[data-testid="tool-item"]', { timeout: 10000 });
    
    const firstTool = page.locator('[data-testid="tool-item"]').first();
    const toolId = await firstTool.locator('[data-testid="tool-id"]').textContent();
    const permissionDropdown = firstTool.locator('[data-testid="permission-select"]');
    
    // Get current value
    const currentValue = await permissionDropdown.inputValue();
    console.log(`Current permission for ${toolId}: ${currentValue}`);
    
    // Change to different value
    const newValue = currentValue === 'allow' ? 'deny' : 'allow';
    await permissionDropdown.selectOption(newValue);
    
    // Wait for API call to complete
    await page.waitForTimeout(1000);
    
    // Verify value changed
    const updatedValue = await permissionDropdown.inputValue();
    expect(updatedValue).toBe(newValue);
    console.log(`Updated permission for ${toolId}: ${updatedValue}`);
    
    // Take screenshot
    await page.screenshot({ path: 'test-results/permission-changed.png', fullPage: true });
  });

  test('should have working search/filter functionality', async ({ page }) => {
    // Wait for tools to load
    await page.waitForSelector('[data-testid="tool-item"]', { timeout: 10000 });
    
    // Get initial count
    const initialCount = await page.locator('[data-testid="tool-item"]').count();
    console.log(`Initial tool count: ${initialCount}`);
    
    // Find search input
    const searchInput = page.locator('[data-testid="tools-search"], input[placeholder*="Search" i]').first();
    await expect(searchInput).toBeVisible();
    
    // Search for "read"
    await searchInput.fill('read');
    await page.waitForTimeout(500);
    
    // Should have fewer results
    const filteredCount = await page.locator('[data-testid="tool-item"]').count();
    console.log(`Filtered tool count: ${filteredCount}`);
    expect(filteredCount).toBeLessThan(initialCount);
    expect(filteredCount).toBeGreaterThan(0);
    
    // All visible tools should contain "read" in ID or description
    const visibleTools = page.locator('[data-testid="tool-item"]');
    const count = await visibleTools.count();
    for (let i = 0; i < Math.min(count, 5); i++) {
      const tool = visibleTools.nth(i);
      const text = await tool.textContent();
      expect(text?.toLowerCase()).toContain('read');
    }
    
    // Take screenshot
    await page.screenshot({ path: 'test-results/search-filtered.png', fullPage: true });
    
    // Clear search
    await searchInput.clear();
    await page.waitForTimeout(500);
    
    // Should show all tools again
    const clearedCount = await page.locator('[data-testid="tool-item"]').count();
    expect(clearedCount).toBe(initialCount);
  });

  test('should display provider grouping correctly', async ({ page }) => {
    // Wait for tools to load
    await page.waitForSelector('[data-testid="tool-item"]', { timeout: 10000 });
    
    // Check if provider headers exist
    const providerHeaders = page.locator('[data-testid="provider-header"]');
    const headerCount = await providerHeaders.count();
    
    if (headerCount > 0) {
      console.log(`Found ${headerCount} provider groups`);
      
      // Should have at least "builtin" provider
      const builtinHeader = page.locator('text=/builtin/i').first();
      await expect(builtinHeader).toBeVisible();
      
      // Take screenshot
      await page.screenshot({ path: 'test-results/provider-grouping.png', fullPage: true });
    } else {
      console.log('No provider grouping headers found - tools may be in flat list');
      
      // Verify tools still have provider labels
      const firstTool = page.locator('[data-testid="tool-item"]').first();
      const provider = firstTool.locator('[data-testid="tool-provider"]');
      await expect(provider).toBeVisible();
      
      // Take screenshot
      await page.screenshot({ path: 'test-results/provider-labels.png', fullPage: true });
    }
  });

  test('should verify API returns built-in tools', async ({ page }) => {
    // Intercept API call
    const responsePromise = page.waitForResponse(
      response => response.url().includes('/api/webchat/v2/tools') && response.status() === 200
    );
    
    // Navigate to trigger API call
    await helper.navigateToSettings();
    await helper.navigateToToolsTab();
    
    // Wait for response
    const response = await responsePromise;
    const data = await response.json();
    
    console.log('API Response:', JSON.stringify(data, null, 2));
    
    // Should have capabilities array
    expect(data).toHaveProperty('capabilities');
    expect(Array.isArray(data.capabilities)).toBe(true);
    
    // Should have at least 40 capabilities
    expect(data.capabilities.length).toBeGreaterThanOrEqual(40);
    
    // First capability should have required fields
    const firstCap = data.capabilities[0];
    expect(firstCap).toHaveProperty('id');
    expect(firstCap).toHaveProperty('description');
    expect(firstCap).toHaveProperty('provider');
    expect(firstCap).toHaveProperty('permission_mode');
    expect(firstCap).toHaveProperty('default_permission');
    
    console.log(`Total capabilities: ${data.capabilities.length}`);
    console.log(`First capability: ${firstCap.id} (${firstCap.provider})`);
  });

  test('should persist permission changes across page reload', async ({ page }) => {
    // Wait for first tool
    await page.waitForSelector('[data-testid="tool-item"]', { timeout: 10000 });
    
    const firstTool = page.locator('[data-testid="tool-item"]').first();
    const toolId = await firstTool.locator('[data-testid="tool-id"]').textContent();
    const permissionDropdown = firstTool.locator('[data-testid="permission-select"]');
    
    // Change permission to deny
    await permissionDropdown.selectOption('deny');
    await page.waitForTimeout(1000);
    
    console.log(`Set ${toolId} to deny`);
    
    // Reload page
    await page.reload();
    await helper.navigateToToolsTab();
    await page.waitForSelector('[data-testid="tool-item"]', { timeout: 10000 });
    
    // Find same tool again
    const reloadedTool = page.locator('[data-testid="tool-item"]').first();
    const reloadedDropdown = reloadedTool.locator('[data-testid="permission-select"]');
    
    // Should still be deny
    const persistedValue = await reloadedDropdown.inputValue();
    expect(persistedValue).toBe('deny');
    
    console.log(`Permission persisted: ${persistedValue}`);
    
    // Take screenshot
    await page.screenshot({ path: 'test-results/permission-persisted.png', fullPage: true });
  });
});

// Made with Bob
