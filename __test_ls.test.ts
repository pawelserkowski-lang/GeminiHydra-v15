
import { test, expect } from 'vitest';
test('localStorage', () => {
  console.log('window.localStorage:', typeof window.localStorage);
  console.log('setItem:', typeof window.localStorage?.setItem);
  console.log('getItem:', typeof window.localStorage?.getItem);
  window.localStorage.setItem('test', 'hello');
  expect(window.localStorage.getItem('test')).toBe('hello');
});
