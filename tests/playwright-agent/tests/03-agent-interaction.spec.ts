import { test, expect } from '@playwright/test';
import { setupTestEnvironment } from './helpers';

test.describe('Agent Interaction Tests', () => {
  test('should send message to agent and receive response', async ({ page }) => {
    const helper = await setupTestEnvironment(page);
    
    // Navigate to chat
    await page.goto('/');
    
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
    const helper = await setupTestEnvironment(page);
    
    await page.goto('/');
    
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
    const helper = await setupTestEnvironment(page);
    
    await page.goto('/');
    
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
    const helper = await setupTestEnvironment(page);
    
    await page.goto('/');
    
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
    const helper = await setupTestEnvironment(page);
    
    await page.goto('/');
    
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