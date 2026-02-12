import '@testing-library/jest-dom/vitest';

// ============================================================================
// Polyfill localStorage for jsdom (broken in jsdom 28 + vitest 4)
// ============================================================================
// jsdom 28 exposes window.localStorage as an object without methods.
// We replace it with a simple Map-backed implementation.

const localStorageMap = new Map<string, string>();

const localStorageMock: Storage = {
  get length() {
    return localStorageMap.size;
  },
  clear() {
    localStorageMap.clear();
  },
  getItem(key: string) {
    return localStorageMap.get(key) ?? null;
  },
  key(index: number) {
    const keys = [...localStorageMap.keys()];
    return keys[index] ?? null;
  },
  removeItem(key: string) {
    localStorageMap.delete(key);
  },
  setItem(key: string, value: string) {
    localStorageMap.set(key, String(value));
  },
};

Object.defineProperty(globalThis, 'localStorage', {
  value: localStorageMock,
  writable: true,
  configurable: true,
});

Object.defineProperty(window, 'localStorage', {
  value: localStorageMock,
  writable: true,
  configurable: true,
});

// ============================================================================
// Ensure crypto.randomUUID is available in jsdom
// ============================================================================

if (typeof globalThis.crypto?.randomUUID !== 'function') {
  Object.defineProperty(globalThis, 'crypto', {
    value: {
      ...globalThis.crypto,
      randomUUID: () => {
        const bytes = new Uint8Array(16);
        for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
        bytes[6] = (bytes[6]! & 0x0f) | 0x40;
        bytes[8] = (bytes[8]! & 0x3f) | 0x80;
        const hex = [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');
        return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
      },
    },
  });
}
