import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    // React Testing Library registers automatic cleanup through global hooks.
    globals: true
  }
});
