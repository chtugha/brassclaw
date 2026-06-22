import { test } from '@playwright/test';

test.describe('Debug Server Status', () => {
  test('check if server is running and LLM endpoint responds', async ({ request }) => {
    console.log('\n=== Testing Server Connectivity ===');
    
    // Test 1: Check if server is up
    try {
      const homeResponse = await request.get('http://localhost:3000/');
      console.log(`Homepage status: ${homeResponse.status()}`);
    } catch (error) {
      console.log(`Homepage error: ${error}`);
    }
    
    // Test 2: Check LLM providers endpoint without auth
    try {
      const llmResponse = await request.get('http://localhost:3000/api/webchat/v2/llm/providers');
      console.log(`\nLLM endpoint (no auth) status: ${llmResponse.status()}`);
      const body = await llmResponse.text();
      console.log(`Response body: ${body}`);
    } catch (error) {
      console.log(`LLM endpoint (no auth) error: ${error}`);
    }
    
    // Test 3: Check LLM providers endpoint with auth token
    try {
      const token = process.env.BRASSCLAW_REBORN_WEBUI_TOKEN || 'test-playwright-token';
      const llmAuthResponse = await request.get('http://localhost:3000/api/webchat/v2/llm/providers', {
        headers: {
          'Authorization': `Bearer ${token}`
        }
      });
      console.log(`\nLLM endpoint (with auth) status: ${llmAuthResponse.status()}`);
      const body = await llmAuthResponse.text();
      console.log(`Response body: ${body}`);
    } catch (error) {
      console.log(`LLM endpoint (with auth) error: ${error}`);
    }
  });
});

// Made with Bob
