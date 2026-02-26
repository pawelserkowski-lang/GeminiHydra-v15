import { test, expect } from '@playwright/test';

test('home view visual regression', async ({ page }) => {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await expect(page).toHaveScreenshot('home.png', { maxDiffPixelRatio: 0.02 });
});

test('dark theme visual', async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => document.documentElement.setAttribute('data-theme', 'dark'));
  await page.waitForLoadState('networkidle');
  await expect(page).toHaveScreenshot('home-dark.png', { maxDiffPixelRatio: 0.02 });
});

test('light theme visual', async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => document.documentElement.setAttribute('data-theme', 'light'));
  await page.waitForLoadState('networkidle');
  await expect(page).toHaveScreenshot('home-light.png', { maxDiffPixelRatio: 0.02 });
});
