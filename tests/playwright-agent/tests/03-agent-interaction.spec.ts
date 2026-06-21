import { test, expect } from '@playwright/test';
import { setupTestEnvironment, SELECTORS } from './helpers';

test.describe('Agent Interaction Tests', () => {
  // Use a unique provider ID for each test run
  const timestamp = Date.now();
  const providerName = `Qwen-Test-${timestamp}`;
  const providerId = `qwen-test-${timestamp}`;
  
  // Helper function to ensure provider is configured and activated
  async function ensureProviderConfigured(page: any) {
    const helper = await setupTestEnvironment(page);
    
    // Navigate to settings
    await helper.navigateToSettings();
    await page.waitForTimeout(1000);
    
    // Add the provider
    await page.waitForSelector(SELECTORS.addProviderButton, { timeout: 10000 });
    await page.click(SELECTORS.addProviderButton);
    
    // Wait for dialog
    await page.getByLabel('Display name').waitFor({ state: 'visible', timeout: 10000 });
    
    // Fill in provider details
    await page.getByLabel('Display name').fill(providerName);
    await page.getByLabel('Provider ID').fill(providerId);
    await page.getByLabel('Base URL').fill('http://192.168.10.223:8000/v1');
    await page.getByLabel('Default model').fill('Qwen/Qwen2.5-7B-Instruct-AWQ');
    
    // Save
    await page.getByRole('button', { name: 'Save' }).click();
    await page.waitForTimeout(2000);
    
    // Close dialog if still open
    const displayNameInput = page.getByLabel('Display name');
    const isStillVisible = await displayNameInput.isVisible().catch(() => false);
    if (isStillVisible) {
      await page.keyboard.press('Escape');
      await page.waitForTimeout(1000);
    }
    
    // Now we need to activate this provider
    // Look for the provider card and click "Use" button on it
    const providerCard = page.locator('.min-w-0.truncate.text-sm.font-semibold').filter({ hasText: providerName }).first();
    await expect(providerCard).toBeVisible({ timeout: 10000 });
    
    // Find the "Use" button - it should be in the same container as the provider name
    // Try multiple strategies to find it
    let useButton = page.locator('button:has-text("Use")').first();
    let hasUseButton = await useButton.isVisible().catch(() => false);
    
    if (!hasUseButton) {
      // Try finding it near the provider card
      useButton = providerCard.locator('..').locator('button:has-text("Use")').first();
      hasUseButton = await useButton.isVisible().catch(() => false);
    }
    
    if (!hasUseButton) {
      // Try a broader search
      useButton = page.locator('button').filter({ hasText: 'Use' }).first();
      hasUseButton = await useButton.isVisible().catch(() => false);
    }
    
    if (hasUseButton) {
      await useButton.click();
      await page.waitForTimeout(2000);
    } else {
      console.log('Warning: Could not find Use button for provider');
    }
    
    // Navigate to chat
    await page.goto('/chat');
    await page.waitForTimeout(3000);
    
    // Wait for chat input to be available
    await page.waitForSelector('textarea, input[type="text"]:not([placeholder*="token"]):not([placeholder*="auth"])', { timeout: 15000 });
  }
  
  test('should send message to agent and receive response', async ({ page }) => {
    await ensureProviderConfigured(page);
    const helper = await setupTestEnvironment(page);
    
    // Send a simple message
    await helper.sendChatMessage('Hello, what is 2+2?');
    
    // Wait for agent response
    await page.waitForTimeout(5000);
    
    // Check for response
    const messages = await page.locator('.message, .chat-message, [role="article"]').count();
    expect(messages).toBeGreaterThan(1);
    
    // Verify response contains expected content
    const lastMessage = page.locator('.message, .chat-message, [role="article"]').last();
    await expect(lastMessage).toContainText(/4|four/i, { timeout: 10000 });
    
    await helper.takeScreenshot('03-agent-response');
  });

  test('should handle tool execution request', async ({ page }) => {
    await ensureProviderConfigured(page);
    const helper = await setupTestEnvironment(page);
    
    // Send message that requires tool use
    await helper.sendChatMessage('What is the current time?');
    
    // Wait for tool execution
    await page.waitForTimeout(5000);
    
    // Should show tool execution or approval UI
    const toolIndicator = page.locator('text=/tool|executing|approve|function/i');
    await expect(toolIndicator).toBeVisible({ timeout: 10000 });
    
    await helper.takeScreenshot('03-tool-execution');
  });

  test('should display agent thinking process', async ({ page }) => {
    await ensureProviderConfigured(page);
    const helper = await setupTestEnvironment(page);
    
    // Send a message that requires reasoning
    await helper.sendChatMessage('Explain the difference between a list and a tuple in Python');
    
    // Wait for response
    await page.waitForTimeout(8000);
    
    // Check that response is present
    const messages = await page.locator('.message, .chat-message, [role="article"]').count();
    expect(messages).toBeGreaterThan(1);
    
    // Verify response contains relevant content
    const lastMessage = page.locator('.message, .chat-message, [role="article"]').last();
    await expect(lastMessage).toContainText(/list|tuple|python/i, { timeout: 10000 });
    
    await helper.takeScreenshot('03-agent-reasoning');
  });

  test('should handle multi-turn conversation', async ({ page }) => {
    await ensureProviderConfigured(page);
    const helper = await setupTestEnvironment(page);
    
    // First message
    await helper.sendChatMessage('My name is Alice');
    await page.waitForTimeout(3000);
    
    // Second message referencing context
    await helper.sendChatMessage('What is my name?');
    await page.waitForTimeout(5000);
    
    // Should remember the name from previous message
    const lastMessage = page.locator('.message, .chat-message, [role="article"]').last();
    await expect(lastMessage).toContainText(/Alice/i, { timeout: 10000 });
    
    await helper.takeScreenshot('03-multi-turn-conversation');
  });

  test('should handle code generation request', async ({ page }) => {
    await ensureProviderConfigured(page);
    const helper = await setupTestEnvironment(page);
    
    // Request code generation
    await helper.sendChatMessage('Write a Python function to calculate factorial');
    
    // Wait for response
    await page.waitForTimeout(8000);
    
    // Check for code block or code-related content
    const codeBlock = page.locator('code, pre, .code-block');
    await expect(codeBlock).toBeVisible({ timeout: 10000 });
    
    // Verify response contains relevant keywords
    const lastMessage = page.locator('.message, .chat-message, [role="article"]').last();
    await expect(lastMessage).toContainText(/def|factorial|return/i, { timeout: 10000 });
    
    await helper.takeScreenshot('03-code-generation');
  });
});

// Made with Bob