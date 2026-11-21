import { defineConfig } from 'vitest/config';

// Run from web/app. This fixture deliberately excludes src/test/setup.ts:
// its eager registration of every locale would conceal cold-start regressions.
export default defineConfig({
  test: {
    environment: 'node',
    setupFiles: [],
    include: [
      'src/shared/i18n/_tests/application-resource-loader.test.ts',
      'src/app/bootstrap/_tests/anonymous-startup-boundary.test.ts'
    ],
    maxWorkers: 1
  }
});
