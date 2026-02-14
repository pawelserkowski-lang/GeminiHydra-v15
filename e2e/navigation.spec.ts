import { test, expect } from '@playwright/test';

test.describe('Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
  });

  test('should navigate to chat view via nav-chat', async ({ page }) => {
    await page.getByTestId('nav-chat').click();
    // Chat view shows the ChatContainer with input
    await expect(page.getByTestId('chat-textarea')).toBeVisible();
  });

  test('should navigate to agents view via nav-agents', async ({ page }) => {
    await page.getByTestId('nav-agents').click();
    await expect(page.getByRole('heading', { name: 'Hydra Agents' })).toBeVisible();
  });

  test('should navigate to history view via nav-history', async ({ page }) => {
    await page.getByTestId('nav-history').click();
    await expect(page.getByRole('heading', { name: 'Session History' })).toBeVisible();
  });

  test('should navigate back to home via nav-home', async ({ page }) => {
    // First go to agents
    await page.getByTestId('nav-agents').click();
    await expect(page.getByRole('heading', { name: 'Hydra Agents' })).toBeVisible();
    // Then back to home
    await page.getByTestId('nav-home').click();
    await expect(page.getByTestId('welcome-hero')).toBeVisible();
  });

  test('should show Coming Soon for settings', async ({ page }) => {
    await page.getByTestId('nav-settings').click();
    await expect(page.getByText('Coming Soon')).toBeVisible();
  });

  test('should show Coming Soon for status', async ({ page }) => {
    await page.getByTestId('nav-status').click();
    await expect(page.getByText('Coming Soon')).toBeVisible();
  });
});
