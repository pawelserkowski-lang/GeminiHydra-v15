import { test, expect } from '@playwright/test';

test.describe('Chat Flow', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
  });

  test('should navigate to chat with empty state when clicking New Chat', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();
    // Should be in chat view with empty state message
    await expect(page.getByText('Type a message to start a conversation...')).toBeVisible();
  });

  test('should show textarea that is visible and focused', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();
    const textarea = page.getByTestId('chat-textarea');
    await expect(textarea).toBeVisible();
    await expect(textarea).toBeFocused();
  });

  test('should enable Send button when text is typed', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();
    const sendBtn = page.getByTestId('btn-send');
    // Initially disabled (empty input)
    await expect(sendBtn).toBeDisabled();
    // Type something
    await page.getByTestId('chat-textarea').fill('Hello World');
    // Now enabled
    await expect(sendBtn).toBeEnabled();
  });

  test('should add user message to chat on submit', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();
    await page.getByTestId('chat-textarea').fill('Hello, Gemini!');
    await page.getByTestId('btn-send').click();
    // User message should appear in the chat message area (inside the drop zone)
    const chatArea = page.getByLabel('File drop zone');
    await expect(chatArea.getByText('Hello, Gemini!')).toBeVisible({ timeout: 5000 });
  });

  test('should show character counter when typing', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();
    await page.getByTestId('chat-textarea').fill('Test message');
    // Character counter format: "12/4000"
    await expect(page.getByText('12/4000')).toBeVisible();
  });

  test('should not allow sending empty message', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();
    const sendBtn = page.getByTestId('btn-send');
    await expect(sendBtn).toBeDisabled();
    // Type spaces only
    await page.getByTestId('chat-textarea').fill('   ');
    await expect(sendBtn).toBeDisabled();
  });
});
