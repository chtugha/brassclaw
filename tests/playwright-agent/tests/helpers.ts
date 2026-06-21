import { Page, expect } from '@playwright/test';

export const SELECTORS = {
  // Navigation
  homeLink: 'a[href="/"]',
  settingsLink: 'a[href="/settings"]',
  agentsLink: 'a[href="/agents"]',
  
  // Chat interface
  chatInput: 'textarea[placeholder*="message"], input[placeholder*="message"]',
  sendButton: 'button[type="submit"], button:has-text("Send")',
  messageList: '.messages, .chat-messages, [role="log"]',
  
  // Settings
  settingsTab: '[data-tab="settings"], a:has-text("Settings")',
  providersTab: '[data-tab="providers"], a:has-text("Providers")',
  
  // LLM Providers
  addProviderButton: 'button:has-text("Add Provider")',
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
    await this.page.fill(SELECTORS.chatInput, message);
    await this.page.click(SELECTORS.sendButton);
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
