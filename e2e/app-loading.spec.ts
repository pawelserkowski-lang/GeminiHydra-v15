import { test, expect } from '@playwright/test';

test.describe('App Loading', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
  });

  test('should render the app without errors', async ({ page }) => {
    await expect(page.locator('body')).toBeVisible();
    // No uncaught errors — console error check
    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));
    await page.waitForTimeout(1000);
    expect(errors).toHaveLength(0);
  });

  test('should show WelcomeScreen by default', async ({ page }) => {
    await expect(page.getByTestId('welcome-hero')).toBeVisible();
    await expect(page.getByRole('heading', { name: 'GeminiHydra' })).toBeVisible();
  });

  test('should show Sidebar on desktop', async ({ page }) => {
    // Desktop viewport — sidebar should be visible
    await expect(page.locator('aside').first()).toBeVisible();
  });

  test('should show StatusFooter', async ({ page }) => {
    await expect(page.getByTestId('status-footer')).toBeVisible();
    await expect(page.getByTestId('status-footer')).toContainText('v15.0.0');
  });

  test('should display feature badges', async ({ page }) => {
    await expect(page.getByText('12 Agents')).toBeVisible();
    await expect(page.getByText('5-Phase Pipeline')).toBeVisible();
    await expect(page.getByText('Multi-Provider')).toBeVisible();
    await expect(page.getByText('Swarm Architecture')).toBeVisible();
  });
});
