import { test as base, expect } from '@playwright/test';

// Do not use the custom fixture here, we want to test the actual auth gate
const test = base;

test.describe('Authentication Flow', () => {
  test.beforeEach(async ({ page }) => {
    // Mock the auth status to be UNauthenticated for this specific test
    await page.route('**/api/auth/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ authenticated: false, oauth_available: true }),
      });
    });

    await page.route('**/api/auth/google/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ authenticated: false, oauth_available: true }),
      });
    });

    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.waitForLoadState('networkidle');
  });

  test('should render LoginView when not authenticated', async ({ page }) => {
    // Verify AuthGate blocked access and showed LoginView
    await expect(page.locator('text=Authentication Required')).toBeVisible();
    await expect(page.getByRole('button', { name: /Continue with Google/i })).toBeVisible();
    
    // Test entering API Key
    const input = page.locator('input[placeholder="Enter API Key..."]');
    await expect(input).toBeVisible();
    
    // Type fake key
    await input.fill('sk-test-fake-api-key-123');
    
    // Mock the api key save endpoint
    await page.route('**/api/auth/apikey', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ status: 'ok', authenticated: true }),
      });
    });
    
    // Submit
    await page.getByRole('button', { name: /Save Key/i }).click();
    
    // Now mock the auth status to return true so the app navigates away
    await page.route('**/api/auth/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ authenticated: true, method: 'api_key', oauth_available: true }),
      });
    });
  });
});
