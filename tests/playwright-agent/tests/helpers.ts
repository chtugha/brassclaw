import { Page, expect } from '@playwright/test';

export const SELECTORS = {
  // Navigation
  homeLink: 'a[href="/chat"], a[href="/"]',
  settingsLink: 'a[href*="/settings"]',
  agentsLink: 'a[href="/agents"]',
  
  // Chat interface
  chatInput: 'textarea, input[type="text"]:not([placeholder*="token"]):not([placeholder*="auth"])',
  sendButton: 'button[type="submit"], button:has-text("Send"), button[aria-label*="Send"]',
  messageList: '.messages, .chat-messages, [role="log"]',
  
  // Settings
  settingsTab: '[data-tab="settings"], a:has-text("Settings")',
  // Note: There is no separate Providers tab - providers are on the Inference settings page
  providersTab: 'a[href="/settings/inference"]',
  
  // LLM Providers
  addProviderButton: 'button:has-text("Add provider")',
  providerNameInput: 'input[name="name"], input[placeholder*="name"]',
  providerTypeSelect: 'select[name="type"], select[name="provider_type"]',
  providerApiKeyInput: 'input[name="api_key"], input[type="password"]',
  saveProviderButton: 'button:has-text("Save"), button[type="submit"]',
  
  // Agent management
  createAgentButton: 'button:has-text("Create Agent"), button:has-text("New Agent")',
  agentNameInput: 'input[name="agent_name"], input[placeholder*="agent"]',
  agentDescriptionInput: 'textarea[name="description"], textarea[placeholder*="description"]',
};

export class BrassClawTestHelper {
  constructor(private page: Page) {}

  async waitForServer() {
    // Wait for server to be ready
    await this.page.waitForTimeout(2000);
    await this.page.goto('/');
    await expect(this.page).toHaveTitle(/BrassClaw|Brass Claw/i);
    
    // Check if we need to authenticate
    const tokenInput = this.page.locator('input[placeholder*="token"], input[placeholder*="auth"]');
    const isLoginPage = await tokenInput.isVisible().catch(() => false);
    
    if (isLoginPage) {
      // Get token from environment variable
      const token = process.env.BRASSCLAW_REBORN_WEBUI_TOKEN || process.env.BRASSCLAW_GATEWAY_TOKEN;
      if (!token) {
        throw new Error('Authentication token not found. Please set BRASSCLAW_REBORN_WEBUI_TOKEN or BRASSCLAW_GATEWAY_TOKEN environment variable.');
      }
      await tokenInput.fill(token);
      
      // Click connect button
      const connectButton = this.page.locator('button:has-text("Connect")');
      await connectButton.click();
      
      // Wait for navigation after login
      await this.page.waitForTimeout(3000);
    }
    
    // Verify we're logged in by checking for main app elements
    // Look for Settings link (could be /settings or /settings/inference)
    await this.page.waitForSelector('a[href*="/settings"], button:has-text("Settings")', { timeout: 10000 });
  }

  async navigateToSettings() {
    await this.page.click(SELECTORS.settingsLink);
    await this.page.waitForURL('**/settings**');
  }

  async navigateToAgents() {
    await this.page.click(SELECTORS.agentsLink);
    await this.page.waitForURL('**/agents**');
  }

  async sendChatMessage(message: string) {
    // Fill the input
    const input = this.page.locator(SELECTORS.chatInput).first();
    await input.fill(message);
    
    // Trigger input event to enable the send button
    await input.dispatchEvent('input');
    await input.dispatchEvent('change');
    
    // Wait a moment for the button to become enabled
    await this.page.waitForTimeout(500);
    
    // Wait for send button to be enabled
    const sendButton = this.page.locator(SELECTORS.sendButton).first();
    await sendButton.waitFor({ state: 'visible', timeout: 5000 });
    
    // Click when enabled
    await sendButton.click({ force: false });
  }

  async waitForResponse(timeout = 30000) {
    // Wait for agent response to appear
    await this.page.waitForTimeout(1000);
    // Look for new message in chat
    const messages = await this.page.locator(SELECTORS.messageList + ' > *').count();
    return messages > 0;
  }

  async addLLMProvider(config: {
    name: string;
    type: string;
    apiKey?: string;
    baseUrl?: string;
  }) {
    await this.navigateToSettings();
    await this.page.click(SELECTORS.providersTab);
    await this.page.click(SELECTORS.addProviderButton);
    
    await this.page.fill(SELECTORS.providerNameInput, config.name);
    await this.page.selectOption(SELECTORS.providerTypeSelect, config.type);
    
    if (config.apiKey) {
      await this.page.fill(SELECTORS.providerApiKeyInput, config.apiKey);
    }
    
    await this.page.click(SELECTORS.saveProviderButton);
    await this.page.waitForTimeout(1000);
  }

  async createAgent(config: {
    name: string;
    description?: string;
    provider?: string;
  }) {
    await this.navigateToAgents();
    await this.page.click(SELECTORS.createAgentButton);
    
    await this.page.fill(SELECTORS.agentNameInput, config.name);
    
    if (config.description) {
      await this.page.fill(SELECTORS.agentDescriptionInput, config.description);
    }
    
    await this.page.click(SELECTORS.saveProviderButton);
    await this.page.waitForTimeout(1000);
  }

  async takeScreenshot(name: string) {
    await this.page.screenshot({ 
      path: `screenshots/${name}.png`,
      fullPage: true 
    });
  }
}

export async function setupTestEnvironment(page: Page) {
  const helper = new BrassClawTestHelper(page);
  await helper.waitForServer();
  return helper;
}

// Made with Bob
