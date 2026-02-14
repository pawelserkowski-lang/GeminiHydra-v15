import { test, expect } from '@playwright/test';

test.describe('Sidebar', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
  });

  test('should collapse sidebar and hide navigation text', async ({ page }) => {
    // Before collapse — nav labels should be visible
    const sidebar = page.locator('aside').first();
    await expect(sidebar.getByText('Home')).toBeVisible();

    // Click collapse button
    await page.getByTestId('btn-sidebar-collapse').click();

    // After collapse — nav labels should be hidden
    await expect(sidebar.getByText('Home')).toBeHidden();
  });

  test('should expand sidebar and show navigation text', async ({ page }) => {
    const sidebar = page.locator('aside').first();

    // Collapse first
    await page.getByTestId('btn-sidebar-collapse').click();
    await expect(sidebar.getByText('Home')).toBeHidden();

    // Expand
    await page.getByTestId('btn-sidebar-collapse').click();
    await expect(sidebar.getByText('Home')).toBeVisible();
  });

  test('should show new session in sidebar after creating chat', async ({ page }) => {
    // Initially no sessions (fresh state)
    const sidebar = page.locator('aside').first();

    // Create a new chat
    await page.getByTestId('btn-new-chat').click();

    // Navigate back to home to see sidebar sessions section
    await page.getByTestId('nav-home').click();

    // The session "New Chat" should appear in sidebar
    await expect(sidebar.getByText('New Chat')).toBeVisible();
  });

  test('should delete session from sidebar list', async ({ page }) => {
    // Create two sessions so delete button appears (need >1)
    await page.getByTestId('btn-new-chat').click();
    await page.getByTestId('nav-home').click();
    await page.getByTestId('btn-new-chat').click();
    await page.getByTestId('nav-home').click();

    const sidebar = page.locator('aside').first();

    // Find delete buttons (X icons) — they appear on hover
    const sessionButtons = sidebar.locator('button').filter({ hasText: 'New Chat' });
    const count = await sessionButtons.count();
    expect(count).toBeGreaterThanOrEqual(2);

    // Hover the first session to reveal delete button, then click it
    await sessionButtons.first().hover();
    const deleteBtn = sessionButtons.first().locator('button');
    await deleteBtn.click();

    // Should have one fewer session
    const newCount = await sidebar.locator('button').filter({ hasText: 'New Chat' }).count();
    expect(newCount).toBe(count - 1);
  });
});
