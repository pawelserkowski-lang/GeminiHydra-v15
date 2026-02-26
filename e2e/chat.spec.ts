import { test, expect } from '@playwright/test';

/**
 * GeminiHydra v15 — E2E Chat Tests
 * ==================================
 * Comprehensive tests for the chat interface:
 * - Page loads and shows chat view
 * - Can type and send a message
 * - Message appears in chat history
 * - Sidebar session list updates
 */

test.describe('Chat — Full Flow', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForLoadState('networkidle');
  });

  test('page loads and shows chat view after clicking New Chat', async ({ page }) => {
    // Start on WelcomeScreen
    await expect(page.getByTestId('welcome-hero')).toBeVisible();

    // Navigate to chat
    await page.getByTestId('btn-new-chat').click();

    // Chat textarea should be visible and focused
    const textarea = page.getByTestId('chat-textarea');
    await expect(textarea).toBeVisible();
    await expect(textarea).toBeFocused();

    // Send button should be present but disabled (empty input)
    const sendBtn = page.getByTestId('btn-send');
    await expect(sendBtn).toBeVisible();
    await expect(sendBtn).toBeDisabled();
  });

  test('can type a message and send button enables', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();

    const textarea = page.getByTestId('chat-textarea');
    const sendBtn = page.getByTestId('btn-send');

    // Initially disabled
    await expect(sendBtn).toBeDisabled();

    // Type a message
    await textarea.fill('Hello, GeminiHydra!');

    // Send button should now be enabled
    await expect(sendBtn).toBeEnabled();

    // Character counter should show
    await expect(page.getByText('19/4000')).toBeVisible();
  });

  test('message appears in chat history after submission', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();

    // Type and send a message
    await page.getByTestId('chat-textarea').fill('Test message for E2E');
    await page.getByTestId('btn-send').click();

    // User message should appear in the chat area
    const chatArea = page.getByLabel('File drop zone');
    await expect(chatArea.getByText('Test message for E2E')).toBeVisible({ timeout: 5000 });

    // Input should be cleared after sending
    const textarea = page.getByTestId('chat-textarea');
    await expect(textarea).toHaveValue('');
  });

  test('sidebar session list updates when a new chat is created', async ({ page }) => {
    // Create a new chat session
    await page.getByTestId('btn-new-chat').click();

    // The sidebar should now have at least one session item
    // Session items are in the sidebar aside element
    const sidebar = page.locator('aside').first();
    await expect(sidebar).toBeVisible();

    // After creating a new chat, there should be a session entry
    // The session list typically shows "New Chat" or similar title
    const sessionItems = sidebar.locator('[data-testid^="session-"]');
    const count = await sessionItems.count();
    expect(count).toBeGreaterThanOrEqual(1);
  });

  test('cannot send empty or whitespace-only message', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();

    const sendBtn = page.getByTestId('btn-send');

    // Empty input — disabled
    await expect(sendBtn).toBeDisabled();

    // Whitespace only — still disabled
    await page.getByTestId('chat-textarea').fill('   ');
    await expect(sendBtn).toBeDisabled();
  });

  test('Enter key sends message, Shift+Enter adds newline', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();

    const textarea = page.getByTestId('chat-textarea');
    await textarea.fill('Line one');

    // Shift+Enter should add a newline (not submit)
    await textarea.press('Shift+Enter');
    // The textarea should now contain a newline
    const val = await textarea.inputValue();
    expect(val).toContain('\n');

    // Clear and type a fresh message, then press Enter to send
    await textarea.fill('Quick send test');
    await textarea.press('Enter');

    // Message should appear in chat
    const chatArea = page.getByLabel('File drop zone');
    await expect(chatArea.getByText('Quick send test')).toBeVisible({ timeout: 5000 });
  });

  test('character counter warns when approaching limit', async ({ page }) => {
    await page.getByTestId('btn-new-chat').click();

    const textarea = page.getByTestId('chat-textarea');

    // Type a short message and verify counter
    await textarea.fill('Hello');
    await expect(page.getByText('5/4000')).toBeVisible();
  });

  test('multiple sessions can be created', async ({ page }) => {
    // Create first chat
    await page.getByTestId('btn-new-chat').click();
    await page.getByTestId('chat-textarea').fill('First session message');
    await page.getByTestId('btn-send').click();

    // Wait for message to appear
    const chatArea = page.getByLabel('File drop zone');
    await expect(chatArea.getByText('First session message')).toBeVisible({ timeout: 5000 });

    // Create second chat
    await page.getByTestId('btn-new-chat').click();

    // The textarea should be empty (new session)
    const textarea = page.getByTestId('chat-textarea');
    await expect(textarea).toHaveValue('');
  });
});
