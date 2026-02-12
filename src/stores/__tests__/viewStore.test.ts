import { describe, it, expect, beforeEach } from 'vitest';
import { useViewStore } from '@/stores/viewStore';
import type { Message } from '@/stores/viewStore';

// Helper to get fresh state
const getState = () => useViewStore.getState();

// Helper to create a message
function makeMsg(
  role: Message['role'],
  content: string,
  model?: string,
): Message {
  return { role, content, timestamp: Date.now(), model };
}

// ============================================================================
// RESET STORE BETWEEN TESTS
// ============================================================================

const initialState = {
  currentView: 'home' as const,
  sidebarCollapsed: false,
  sessions: [],
  currentSessionId: null,
  chatHistory: {},
  tabs: [],
  activeTabId: null,
};

beforeEach(() => {
  // Use merge mode (not replace) to preserve action functions in the store
  useViewStore.setState(initialState);
  // Also clear localStorage to avoid persist rehydration interference
  localStorage.clear();
});

// ============================================================================
// INITIAL STATE
// ============================================================================

describe('viewStore - initial state', () => {
  it('should have correct initial values', () => {
    const state = getState();
    expect(state.currentView).toBe('home');
    expect(state.sidebarCollapsed).toBe(false);
    expect(state.sessions).toEqual([]);
    expect(state.currentSessionId).toBeNull();
    expect(state.chatHistory).toEqual({});
    expect(state.tabs).toEqual([]);
    expect(state.activeTabId).toBeNull();
  });
});

// ============================================================================
// VIEW ACTIONS
// ============================================================================

describe('viewStore - setCurrentView', () => {
  it('should change the current view', () => {
    getState().setCurrentView('chat');
    expect(getState().currentView).toBe('chat');
  });

  it('should accept all valid view values', () => {
    const views = ['home', 'chat', 'agents', 'history', 'settings', 'status'] as const;
    for (const view of views) {
      getState().setCurrentView(view);
      expect(getState().currentView).toBe(view);
    }
  });
});

describe('viewStore - sidebar toggle', () => {
  it('should toggle sidebar from false to true', () => {
    expect(getState().sidebarCollapsed).toBe(false);
    getState().toggleSidebar();
    expect(getState().sidebarCollapsed).toBe(true);
  });

  it('should toggle sidebar from true to false', () => {
    getState().setSidebarCollapsed(true);
    expect(getState().sidebarCollapsed).toBe(true);
    getState().toggleSidebar();
    expect(getState().sidebarCollapsed).toBe(false);
  });

  it('should set sidebar collapsed directly', () => {
    getState().setSidebarCollapsed(true);
    expect(getState().sidebarCollapsed).toBe(true);
    getState().setSidebarCollapsed(false);
    expect(getState().sidebarCollapsed).toBe(false);
  });
});

// ============================================================================
// SESSION CRUD
// ============================================================================

describe('viewStore - createSession', () => {
  it('should create a new session and set it as current', () => {
    getState().createSession();
    const state = getState();
    expect(state.sessions).toHaveLength(1);
    expect(state.currentSessionId).toBe(state.sessions[0]!.id);
    expect(state.sessions[0]!.title).toBe('New Chat');
  });

  it('should prepend new sessions (newest first)', () => {
    getState().createSession();
    const first = getState().sessions[0]!.id;
    getState().createSession();
    const state = getState();
    expect(state.sessions).toHaveLength(2);
    expect(state.sessions[0]!.id).not.toBe(first);
    expect(state.sessions[1]!.id).toBe(first);
  });

  it('should initialize empty chat history for new session', () => {
    getState().createSession();
    const id = getState().currentSessionId!;
    expect(getState().chatHistory[id]).toEqual([]);
  });
});

describe('viewStore - deleteSession', () => {
  it('should remove the session and its chat history', () => {
    getState().createSession();
    const id = getState().currentSessionId!;
    getState().deleteSession(id);
    expect(getState().sessions).toHaveLength(0);
    expect(getState().chatHistory[id]).toBeUndefined();
  });

  it('should select the first remaining session when deleting current', () => {
    getState().createSession();
    const firstId = getState().currentSessionId!;
    getState().createSession();
    const secondId = getState().currentSessionId!;

    getState().deleteSession(secondId);
    expect(getState().currentSessionId).toBe(firstId);
  });

  it('should set currentSessionId to null when deleting the last session', () => {
    getState().createSession();
    const id = getState().currentSessionId!;
    getState().deleteSession(id);
    expect(getState().currentSessionId).toBeNull();
  });

  it('should also close tabs linked to the deleted session', () => {
    getState().createSession();
    const id = getState().currentSessionId!;
    getState().openTab(id);
    expect(getState().tabs).toHaveLength(1);

    getState().deleteSession(id);
    expect(getState().tabs).toHaveLength(0);
  });
});

describe('viewStore - selectSession', () => {
  it('should select an existing session', () => {
    getState().createSession();
    const firstId = getState().currentSessionId!;
    getState().createSession();
    expect(getState().currentSessionId).not.toBe(firstId);

    getState().selectSession(firstId);
    expect(getState().currentSessionId).toBe(firstId);
  });

  it('should not change state when selecting non-existent session', () => {
    getState().createSession();
    const currentId = getState().currentSessionId!;
    getState().selectSession('non-existent-id');
    expect(getState().currentSessionId).toBe(currentId);
  });
});

describe('viewStore - updateSessionTitle', () => {
  it('should update the session title', () => {
    getState().createSession();
    const id = getState().currentSessionId!;
    getState().updateSessionTitle(id, 'My Custom Title');
    expect(getState().sessions[0]!.title).toBe('My Custom Title');
  });

  it('should truncate titles exceeding MAX_TITLE_LENGTH (100)', () => {
    getState().createSession();
    const id = getState().currentSessionId!;
    const longTitle = 'A'.repeat(150);
    getState().updateSessionTitle(id, longTitle);
    expect(getState().sessions[0]!.title).toHaveLength(100);
  });

  it('should default to "New Chat" for empty/whitespace-only titles', () => {
    getState().createSession();
    const id = getState().currentSessionId!;
    getState().updateSessionTitle(id, '   ');
    expect(getState().sessions[0]!.title).toBe('New Chat');
  });

  it('should also update matching tab titles', () => {
    getState().createSession();
    const id = getState().currentSessionId!;
    getState().openTab(id);
    getState().updateSessionTitle(id, 'Updated Title');
    expect(getState().tabs[0]!.title).toBe('Updated Title');
  });
});

// ============================================================================
// MAX_SESSIONS LIMIT
// ============================================================================

describe('viewStore - MAX_SESSIONS limit (50)', () => {
  it('should enforce max 50 sessions, removing oldest when exceeded', () => {
    for (let i = 0; i < 52; i++) {
      getState().createSession();
    }
    expect(getState().sessions).toHaveLength(50);
  });

  it('should clean up chat history for removed sessions', () => {
    // Create 50 sessions, record the oldest
    for (let i = 0; i < 50; i++) {
      getState().createSession();
    }
    const oldestId = getState().sessions[49]!.id;

    // Create one more to push the oldest out
    getState().createSession();
    expect(getState().chatHistory[oldestId]).toBeUndefined();
  });
});

// ============================================================================
// TAB MANAGEMENT
// ============================================================================

describe('viewStore - openTab', () => {
  it('should create a new tab for a session', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);

    const state = getState();
    expect(state.tabs).toHaveLength(1);
    expect(state.tabs[0]!.sessionId).toBe(sessionId);
    expect(state.activeTabId).toBe(state.tabs[0]!.id);
  });

  it('should reuse existing tab for same session', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);
    const tabId = getState().activeTabId;

    getState().openTab(sessionId);
    expect(getState().tabs).toHaveLength(1);
    expect(getState().activeTabId).toBe(tabId);
  });

  it('should set currentSessionId when opening a tab', () => {
    getState().createSession();
    const firstSession = getState().currentSessionId!;
    getState().createSession();
    const secondSession = getState().currentSessionId!;

    getState().openTab(firstSession);
    expect(getState().currentSessionId).toBe(firstSession);

    getState().openTab(secondSession);
    expect(getState().currentSessionId).toBe(secondSession);
  });
});

describe('viewStore - closeTab', () => {
  it('should close an unpinned tab', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);
    const tabId = getState().activeTabId!;

    getState().closeTab(tabId);
    expect(getState().tabs).toHaveLength(0);
  });

  it('should NOT close a pinned tab', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);
    const tabId = getState().activeTabId!;
    getState().togglePinTab(tabId);

    getState().closeTab(tabId);
    expect(getState().tabs).toHaveLength(1);
  });

  it('should activate the next tab when closing the active tab', () => {
    getState().createSession();
    const s1 = getState().currentSessionId!;
    getState().createSession();
    const s2 = getState().currentSessionId!;

    getState().openTab(s1);
    getState().openTab(s2);
    const tabToClose = getState().activeTabId!;

    getState().closeTab(tabToClose);
    expect(getState().activeTabId).toBe(getState().tabs[0]!.id);
  });

  it('should do nothing if tabId does not exist', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);
    const before = getState().tabs.length;

    getState().closeTab('non-existent');
    expect(getState().tabs).toHaveLength(before);
  });
});

describe('viewStore - switchTab', () => {
  it('should set activeTabId and currentSessionId', () => {
    getState().createSession();
    const s1 = getState().currentSessionId!;
    getState().createSession();
    const s2 = getState().currentSessionId!;

    getState().openTab(s1);
    const tab1Id = getState().tabs.find((t) => t.sessionId === s1)!.id;
    getState().openTab(s2);

    getState().switchTab(tab1Id);
    expect(getState().activeTabId).toBe(tab1Id);
    expect(getState().currentSessionId).toBe(s1);
  });

  it('should set currentView to chat', () => {
    getState().setCurrentView('home');
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);
    const tabId = getState().activeTabId!;

    getState().switchTab(tabId);
    expect(getState().currentView).toBe('chat');
  });

  it('should not change state for non-existent tab', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);
    const before = getState().activeTabId;

    getState().switchTab('non-existent');
    expect(getState().activeTabId).toBe(before);
  });
});

describe('viewStore - reorderTabs', () => {
  it('should swap tab positions', () => {
    getState().createSession();
    const s1 = getState().currentSessionId!;
    getState().createSession();
    const s2 = getState().currentSessionId!;
    getState().createSession();
    const s3 = getState().currentSessionId!;

    getState().openTab(s1);
    getState().openTab(s2);
    getState().openTab(s3);

    const originalFirst = getState().tabs[0]!.id;
    const originalLast = getState().tabs[2]!.id;

    getState().reorderTabs(0, 2);
    expect(getState().tabs[2]!.id).toBe(originalFirst);
    expect(getState().tabs[1]!.id).toBe(originalLast);
  });

  it('should ignore out-of-bounds indices', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);

    const before = [...getState().tabs];
    getState().reorderTabs(-1, 5);
    expect(getState().tabs).toEqual(before);
  });
});

describe('viewStore - togglePinTab', () => {
  it('should toggle pin state on a tab', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);
    const tabId = getState().tabs[0]!.id;

    expect(getState().tabs[0]!.isPinned).toBe(false);
    getState().togglePinTab(tabId);
    expect(getState().tabs[0]!.isPinned).toBe(true);
    getState().togglePinTab(tabId);
    expect(getState().tabs[0]!.isPinned).toBe(false);
  });
});

// ============================================================================
// MESSAGE ACTIONS
// ============================================================================

describe('viewStore - addMessage', () => {
  it('should add a message to the current session', () => {
    getState().createSession();
    getState().addMessage(makeMsg('user', 'Hello'));

    const id = getState().currentSessionId!;
    const messages = getState().chatHistory[id]!;
    expect(messages).toHaveLength(1);
    expect(messages[0]!.content).toBe('Hello');
    expect(messages[0]!.role).toBe('user');
  });

  it('should do nothing if no current session', () => {
    getState().addMessage(makeMsg('user', 'Hello'));
    expect(Object.keys(getState().chatHistory)).toHaveLength(0);
  });

  it('should preserve message model field', () => {
    getState().createSession();
    getState().addMessage(makeMsg('assistant', 'Hi!', 'gemini-2.0'));
    const id = getState().currentSessionId!;
    expect(getState().chatHistory[id]![0]!.model).toBe('gemini-2.0');
  });
});

describe('viewStore - updateLastMessage', () => {
  it('should append content to the last message', () => {
    getState().createSession();
    getState().addMessage(makeMsg('assistant', 'Hello'));
    getState().updateLastMessage(' World');

    const id = getState().currentSessionId!;
    expect(getState().chatHistory[id]![0]!.content).toBe('Hello World');
  });

  it('should do nothing if no messages exist', () => {
    getState().createSession();
    getState().updateLastMessage('test');
    const id = getState().currentSessionId!;
    expect(getState().chatHistory[id]).toEqual([]);
  });

  it('should do nothing if no current session', () => {
    getState().updateLastMessage('test');
    expect(getState().chatHistory).toEqual({});
  });
});

describe('viewStore - clearHistory', () => {
  it('should clear messages for the current session', () => {
    getState().createSession();
    getState().addMessage(makeMsg('user', 'Hello'));
    getState().addMessage(makeMsg('assistant', 'Hi!'));

    getState().clearHistory();
    const id = getState().currentSessionId!;
    expect(getState().chatHistory[id]).toEqual([]);
  });

  it('should not affect other sessions', () => {
    getState().createSession();
    const s1 = getState().currentSessionId!;
    getState().addMessage(makeMsg('user', 'Session 1'));

    getState().createSession();
    getState().addMessage(makeMsg('user', 'Session 2'));

    getState().clearHistory();
    // Current session (s2) is cleared
    const s2 = getState().currentSessionId!;
    expect(getState().chatHistory[s2]).toEqual([]);
    // s1 is untouched
    expect(getState().chatHistory[s1]).toHaveLength(1);
  });

  it('should do nothing if no current session', () => {
    getState().clearHistory();
    expect(getState().chatHistory).toEqual({});
  });
});

// ============================================================================
// AUTO-TITLING
// ============================================================================

describe('viewStore - auto-titling', () => {
  it('should set session title from first user message', () => {
    getState().createSession();
    getState().addMessage(makeMsg('user', 'What is quantum computing?'));
    const session = getState().sessions[0]!;
    // Message is <= 30 chars so no ellipsis is appended
    expect(session.title).toBe('What is quantum computing?');
  });

  it('should truncate long first messages to 30 chars + ellipsis', () => {
    getState().createSession();
    const longMsg = 'A'.repeat(50);
    getState().addMessage(makeMsg('user', longMsg));
    const session = getState().sessions[0]!;
    expect(session.title).toBe('A'.repeat(30) + '...');
  });

  it('should not change title on subsequent user messages', () => {
    getState().createSession();
    getState().addMessage(makeMsg('user', 'First message'));
    const titleAfterFirst = getState().sessions[0]!.title;
    getState().addMessage(makeMsg('user', 'Second message'));
    expect(getState().sessions[0]!.title).toBe(titleAfterFirst);
  });

  it('should not auto-title on assistant/system messages', () => {
    getState().createSession();
    getState().addMessage(makeMsg('system', 'System prompt'));
    // system message is first, so no auto-title since it is not 'user' role
    // But wait - the condition is msg.role === 'user' && currentMessages.length === 0
    // system message at index 0 won't trigger auto-title. After it, length is 1, so
    // next user message also won't trigger. Title stays 'New Chat'.
    expect(getState().sessions[0]!.title).toBe('New Chat');
  });

  it('should also update matching tab title on auto-title', () => {
    getState().createSession();
    const sessionId = getState().currentSessionId!;
    getState().openTab(sessionId);
    getState().addMessage(makeMsg('user', 'Hello world'));
    expect(getState().tabs[0]!.title).toBe('Hello world');
  });
});

// ============================================================================
// MESSAGE COUNT LIMIT
// ============================================================================

describe('viewStore - MAX_MESSAGES_PER_SESSION (500)', () => {
  it('should cap messages at 500, keeping the most recent', () => {
    getState().createSession();
    const id = getState().currentSessionId!;

    for (let i = 0; i < 510; i++) {
      getState().addMessage(makeMsg('user', `Message ${i}`));
    }

    const messages = getState().chatHistory[id]!;
    expect(messages).toHaveLength(500);
    // Oldest should be trimmed, most recent kept
    expect(messages[messages.length - 1]!.content).toBe('Message 509');
    expect(messages[0]!.content).toBe('Message 10');
  });
});
