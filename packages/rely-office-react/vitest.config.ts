import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'jsdom',
    include: ['src/**/*.test.{ts,tsx}', 'test/**/*.test.{ts,tsx}'],
    setupFiles: ['test/setup-dom.ts'],
    // The grid draws hundreds of cells in jsdom; a slow runner needs more than the default 5 s.
    testTimeout: 30_000,
  },
});
