/** @type {import('@stryker-mutator/api/core').PartialStrykerOptions} */
export default {
  testRunner: 'vitest',
  vitest: { configFile: 'vite.config.ts' },
  mutate: ['src/**/*.ts', 'src/**/*.tsx', '!src/**/*.test.*', '!src/**/*.spec.*'],
  reporters: ['html', 'clear-text', 'progress'],
  coverageAnalysis: 'perTest',
  thresholds: { high: 80, low: 60, break: 50 },
};
