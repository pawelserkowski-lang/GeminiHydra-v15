import { test, expect } from '@playwright/test';

test.describe('History View', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
  });

  test('should display sessions in history view', async ({ page }) => {
    // Create a session first
    await page.getByTestId('btn-new-chat').click();
    // Navigate to history
    await page.getByTestId('nav-history').click();

    const main = page.getByTestId('main-content');
    await expect(main.getByRole('heading', { name: 'Session History' })).toBeVisible();
    // Should show 1 session
    await expect(main.getByText('1 session')).toBeVisible();
    await expect(main.getByText('New Chat').first()).toBeVisible();
  });

  test('should filter sessions by search query', async ({ page }) => {
    const main = page.getByTestId('main-content');

    // Seed two sessions with different titles directly in the Zustand store
    // This avoids API mutation issues (no backend)
    await page.evaluate(() => {
      const storeData = {
        state: {
          currentView: 'home',
          sidebarCollapsed: false,
          sessions: [
            { id: 'sess-alpha', title: 'Alpha conversation topic', createdAt: Date.now() - 10000 },
            { id: 'sess-beta', title: 'Beta discussion item', createdAt: Date.now() },
          ],
          currentSessionId: 'sess-beta',
          chatHistory: {
            'sess-alpha': [{ role: 'user', content: 'Alpha conversation topic', timestamp: Date.now() - 10000 }],
            'sess-beta': [{ role: 'user', content: 'Beta discussion item', timestamp: Date.now() }],
          },
          tabs: [],
          activeTabId: null,
        },
        version: 0,
      };
      localStorage.setItem('geminihydra-v15-state', JSON.stringify(storeData));
    });
    await page.reload();
    await page.waitForLoadState('networkidle');

    // Navigate to history
    await page.getByTestId('nav-history').click();
    await expect(main.getByText('2 sessions')).toBeVisible();

    // Search for "Alpha" using pressSequentially for reliable React onChange
    const searchInput = main.getByPlaceholder('Search sessions...');
    await searchInput.click();
    await searchInput.pressSequentially('Alpha', { delay: 50 });
    // Wait for filter to take effect
    await expect(main.getByText('Alpha conversation topic').first()).toBeVisible();
    // Beta session should not be visible
    await expect(main.getByText('Beta discussion item').first()).toBeHidden();
  });

  test('should toggle sort order', async ({ page }) => {
    // Create two sessions
    await page.getByTestId('btn-new-chat').click();
    await page.getByTestId('nav-home').click();
    await page.getByTestId('btn-new-chat').click();
    await page.getByTestId('nav-history').click();

    // Default is "Newest first"
    await expect(page.getByRole('button', { name: /Newest first/i })).toBeVisible();

    // Click to toggle to "Oldest first"
    await page.getByRole('button', { name: /Newest first/i }).click();
    await expect(page.getByRole('button', { name: /Oldest first/i })).toBeVisible();

    // Toggle back
    await page.getByRole('button', { name: /Oldest first/i }).click();
    await expect(page.getByRole('button', { name: /Newest first/i })).toBeVisible();
  });
});
