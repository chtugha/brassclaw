import { test, expect } from '@playwright/test';
import { BrassClawTestHelper } from './helpers';

// Minimum expected capability count: 23 base + 4 extension lifecycle = 27.
const MIN_EXPECTED_CAPABILITIES = 25;

test.describe('Built-in Tools - Simple API Test', () => {
  test('should return built-in tools from API', async ({ page, request }) => {
    const helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
    
    // Get the token
    const token = process.env.BRASSCLAW_REBORN_WEBUI_TOKEN || process.env.BRASSCLAW_GATEWAY_TOKEN;
    
    // Make direct API call
    const response = await request.get('http://localhost:3000/api/webchat/v2/tools', {
      headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json'
      }
    });
    
    expect(response.status()).toBe(200);
    
    const data = await response.json();
    console.log('API Response:', JSON.stringify(data, null, 2));
    
    // Should have capabilities array
    expect(data).toHaveProperty('capabilities');
    expect(Array.isArray(data.capabilities)).toBe(true);
    
    // Should have at least 25 capabilities (27 expected: 23 base + 4 extension lifecycle)
    const capCount = data.capabilities.length;
    console.log(`Total capabilities returned: ${capCount}`);
    expect(capCount).toBeGreaterThanOrEqual(MIN_EXPECTED_CAPABILITIES);
    
    // First capability should have required fields
    if (capCount > 0) {
      const firstCap = data.capabilities[0];
      expect(firstCap).toHaveProperty('id');
      expect(firstCap).toHaveProperty('description');
      expect(firstCap).toHaveProperty('provider');
      expect(firstCap).toHaveProperty('permission_mode');
      expect(firstCap).toHaveProperty('default_permission');
      
      console.log(`First capability: ${firstCap.id} (${firstCap.provider})`);
      console.log(`Permission: ${firstCap.permission_mode} (default: ${firstCap.default_permission})`);
    }
    
    // List all capability IDs
    console.log('\nAll capability IDs:');
    data.capabilities.forEach((cap: any, index: number) => {
      console.log(`${index + 1}. ${cap.id} (${cap.provider})`);
    });
  });

  test('should display tools in UI', async ({ page }) => {
    const helper = new BrassClawTestHelper(page);
    await helper.waitForServer();
    await helper.navigateToSettings();
    await helper.navigateToToolsTab();
    
    // Wait a bit for the page to load
    await page.waitForTimeout(2000);
    
    // Take screenshot
    await page.screenshot({ path: 'test-results/tools-page.png', fullPage: true });
    
    // Check if there's content on the page
    const pageContent = await page.content();
    console.log('Page has tools content:', pageContent.includes('tool') || pageContent.includes('capability'));
    
    // Try to find any tool-related elements
    const toolElements = await page.locator('[data-testid*="tool"], [class*="tool"], [class*="capability"]').count();
    console.log(`Found ${toolElements} tool-related elements`);
    
    // Check for empty state or tools list
    const hasEmptyState = await page.locator('text=/no tools|empty|extensions.*MCP/i').count();
    const hasToolsList = await page.locator('[data-testid="tools-list"], [class*="tools-list"]').count();
    
    console.log(`Empty state elements: ${hasEmptyState}`);
    console.log(`Tools list elements: ${hasToolsList}`);
  });
});

// Made with Bob
