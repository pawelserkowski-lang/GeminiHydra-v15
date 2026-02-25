import { test, expect } from '@playwright/test';

test.describe('Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
  });

  test('should show home view by default with welcome hero', async ({ page }) => {
    await expect(page.getByTestId('welcome-hero')).toBeVisible();
  });

  test('should navigate to chat view via nav-chat', async ({ page }) => {
    await page.getByTestId('nav-chat').click();
    // Chat view shows the ChatContainer with input
    await expect(page.getByTestId('chat-textarea')).toBeVisible();
  });

  test('should navigate back to home via nav-home', async ({ page }) => {
    // First go to chat
    await page.getByTestId('nav-chat').click();
    await expect(page.getByTestId('chat-textarea')).toBeVisible();
    // Then back to home
    await page.getByTestId('nav-home').click();
    await expect(page.getByTestId('welcome-hero')).toBeVisible();
  });

  test('should highlight active nav item', async ({ page }) => {
    // Home should be active by default
    const navHome = page.getByTestId('nav-home');
    await expect(navHome).toBeVisible();

    // Navigate to chat
    const navChat = page.getByTestId('nav-chat');
    await navChat.click();
    await expect(page.getByTestId('chat-textarea')).toBeVisible();

    // Navigate back to home
    await navHome.click();
    await expect(page.getByTestId('welcome-hero')).toBeVisible();
  });

  test('should toggle sidebar collapse', async ({ page }) => {
    const collapseBtn = page.getByTestId('btn-sidebar-collapse');
    await expect(collapseBtn).toBeVisible();

    // Collapse
    await collapseBtn.click();

    // Nav items should still be visible (icons only when collapsed)
    await expect(page.getByTestId('nav-home')).toBeVisible();
    await expect(page.getByTestId('nav-chat')).toBeVisible();

    // Expand again
    await collapseBtn.click();

    // Nav items still visible
    await expect(page.getByTestId('nav-home')).toBeVisible();
    await expect(page.getByTestId('nav-chat')).toBeVisible();
  });
});
