import { test, expect } from '@playwright/test';

test.describe('Theme Switching', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
  });

  test('should have dark theme by default', async ({ page }) => {
    await expect(page.locator('html')).toHaveClass(/dark/);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  });

  test('should switch to light theme on toggle click', async ({ page }) => {
    await page.getByTestId('btn-theme-toggle').click();
    await expect(page.locator('html')).toHaveClass(/light/);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  });

  test('should switch back to dark on second toggle click', async ({ page }) => {
    await page.getByTestId('btn-theme-toggle').click();
    await expect(page.locator('html')).toHaveClass(/light/);
    await page.getByTestId('btn-theme-toggle').click();
    await expect(page.locator('html')).toHaveClass(/dark/);
  });

  test('should persist theme after reload', async ({ page }) => {
    // Switch to light
    await page.getByTestId('btn-theme-toggle').click();
    await expect(page.locator('html')).toHaveClass(/light/);
    // Reload
    await page.reload();
    await page.waitForLoadState('networkidle');
    // Theme should still be light (persisted in localStorage)
    await expect(page.locator('html')).toHaveClass(/light/);
  });
});
