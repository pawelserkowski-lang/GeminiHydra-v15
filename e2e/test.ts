import { test as base, expect } from '@playwright/test';

// Extend base test to mock authentication globally for E2E views
export const test = base.extend({
  page: async ({ page }, use) => {
    // Intercept auth status requests and return authenticated: true
    await page.route('**/api/auth/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ authenticated: true, method: 'env', oauth_available: true }),
      });
    });

    await page.route('**/api/auth/google/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ authenticated: true, method: 'env', oauth_available: true }),
      });
    });
    
    // Mock for ClaudeHydra specific endpoints
    await page.route('**/api/auth/github/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ authenticated: true }),
      });
    });
    await page.route('**/api/auth/vercel/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ authenticated: true }),
      });
    });

    // Inject local storage state to skip initial animations/login redirects
    await page.addInitScript(() => {
      window.localStorage.setItem('jaskier_auth_dismissed', 'true');
      // For GeminiHydra it might have a different key, assuming similar to ClaudeHydra
      window.localStorage.setItem('gemini-hydra-v15-view', JSON.stringify({
        state: { currentView: 'home' },
        version: 2
      }));
    });

    await use(page);
  },
});

export { expect };
