import { test, expect } from '@playwright/test';

test.describe('Agents View', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
    // Navigate to agents view
    await page.getByTestId('nav-agents').click();
  });

  test('should display all 12 agent cards', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Hydra Agents' })).toBeVisible();
    // Header shows "12 agents"
    await expect(page.getByText('12 agents')).toBeVisible();
    // Verify individual agent names are visible
    const agentNames = [
      'Geralt', 'Yennefer', 'Triss', 'Jaskier', 'Vesemir', 'Ciri',
      'Dijkstra', 'Lambert', 'Eskel', 'Regis', 'Zoltan', 'Philippa',
    ];
    for (const name of agentNames) {
      await expect(page.getByText(name, { exact: true }).first()).toBeVisible();
    }
  });

  test('should filter agents by tier', async ({ page }) => {
    // Click "Commander" filter
    await page.getByRole('button', { name: /Commander/i }).click();
    // Only Geralt should be visible (the only commander)
    await expect(page.getByText('Geralt', { exact: true })).toBeVisible();
    // Other agents should not be visible
    await expect(page.getByText('Yennefer', { exact: true })).toBeHidden();
    await expect(page.getByText('Lambert', { exact: true })).toBeHidden();

    // Click "All Agents" to reset
    await page.getByRole('button', { name: 'All Agents' }).click();
    await expect(page.getByText('Yennefer', { exact: true })).toBeVisible();
  });

  test('should show agent name and status on each card', async ({ page }) => {
    // Check that Geralt has name, role, and status displayed
    await expect(page.getByText('Geralt', { exact: true })).toBeVisible();
    await expect(page.getByText('Commander', { exact: true }).first()).toBeVisible();
    await expect(page.getByText('Active').first()).toBeVisible();
  });
});
