import { test, expect } from '@playwright/test';

test.describe('Debug Safety Store', () => {
  test('check if safety endpoints exist', async ({ request }) => {
    // Test if the endpoint exists (should return 401 without auth, not 404)
    const response = await request.get('http://127.0.0.1:3000/api/webchat/v2/safety/sensitive-paths');
    
    console.log('Response status:', response.status());
    console.log('Response headers:', await response.headers());
    
    // 401 = endpoint exists but needs auth
    // 404 = endpoint doesn't exist
    // 500 = endpoint exists but has internal error
    expect([401, 500]).toContain(response.status());
  });

  test('check with auth token', async ({ request }) => {
    const response = await request.get('http://127.0.0.1:3000/api/webchat/v2/safety/sensitive-paths', {
      headers: {
        'Authorization': 'Bearer test-playwright-token',
      },
    });
    
    console.log('With auth - Status:', response.status());
    console.log('With auth - Body:', await response.text());
    
    if (response.status() === 500) {
      console.log('ERROR: Got 500 error - safety_config_store is likely None');
    } else if (response.status() === 200) {
      console.log('SUCCESS: Got 200 - safety_config_store is wired correctly');
    }
  });
});

// Made with Bob
