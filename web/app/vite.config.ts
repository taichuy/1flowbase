import { fileURLToPath, URL } from 'node:url';

import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv, searchForWorkspaceRoot } from 'vite';

function manualChunks(id: string) {
  if (!id.includes('/node_modules/')) {
    return;
  }

  if (id.includes('/monaco-editor/') || id.includes('/@monaco-editor/')) {
    return 'monaco-vendor';
  }

  if (id.includes('/@xyflow/')) {
    return 'flow-vendor';
  }

  if (id.includes('/antd/') || id.includes('/@ant-design/') || id.includes('/rc-')) {
    return 'antd-vendor';
  }

  if (
    id.includes('/react/') ||
    id.includes('/react-dom/') ||
    id.includes('/scheduler/') ||
    id.includes('/@tanstack/')
  ) {
    return 'react-vendor';
  }
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const apiProxyTarget = (
    env.VITE_API_PROXY_TARGET ||
    env.VITE_API_BASE_URL ||
    'http://127.0.0.1:7800'
  ).replace(/\/$/, '');

  return {
    plugins: [react()],
    build: {
      chunkSizeWarningLimit: 3500,
      rollupOptions: {
        output: {
          manualChunks
        }
      }
    },
    server: {
      host: '0.0.0.0',
      port: 3100,
      strictPort: true,
      fs: {
        allow: [
          searchForWorkspaceRoot(process.cwd()),
          fileURLToPath(new URL('../../scripts', import.meta.url))
        ]
      },
      proxy: {
        '/api': {
          target: apiProxyTarget,
          changeOrigin: true
        },
        '/health': {
          target: apiProxyTarget,
          changeOrigin: true
        },
        '/openapi.json': {
          target: apiProxyTarget,
          changeOrigin: true
        }
      }
    },
    resolve: {
      alias: {
        '@1flowbase/shared-types': fileURLToPath(
          new URL('../packages/shared-types/src/index.ts', import.meta.url)
        ),
        '@1flowbase/api-client': fileURLToPath(
          new URL('../packages/api-client/src/index.ts', import.meta.url)
        ),
        '@1flowbase/antd-facade': fileURLToPath(
          new URL('../packages/antd-facade/src/index.ts', import.meta.url)
        ),
        '@1flowbase/block-renderer/antd-facade': fileURLToPath(
          new URL('../packages/block-renderer/src/antd-facade.ts', import.meta.url)
        ),
        '@1flowbase/block-renderer': fileURLToPath(
          new URL('../packages/block-renderer/src/index.tsx', import.meta.url)
        ),
        '@1flowbase/model-provider-contracts': fileURLToPath(
          new URL('../../scripts/node/testing/contracts/model-providers', import.meta.url)
        ),
        '@1flowbase/ui': fileURLToPath(
          new URL('../packages/ui/src/index.tsx', import.meta.url)
        ),
        '@1flowbase/flow-schema': fileURLToPath(
          new URL('../packages/flow-schema/src/index.ts', import.meta.url)
        ),
        '@1flowbase/page-protocol': fileURLToPath(
          new URL('../packages/page-protocol/src/index.ts', import.meta.url)
        ),
        '@1flowbase/page-runtime': fileURLToPath(
          new URL('../packages/page-runtime/src/index.ts', import.meta.url)
        ),
        '@1flowbase/embed-sdk': fileURLToPath(
          new URL('../packages/embed-sdk/src/index.ts', import.meta.url)
        )
      }
    },
    test: {
      environment: 'jsdom',
      globals: true,
      setupFiles: './src/test/setup.ts',
      include: ['src/**/*.{test,spec}.?(c|m)[jt]s?(x)', '../packages/page-runtime/src/_tests/**/*.test.ts'],
      coverage: {
        provider: 'v8',
        reporter: ['text-summary', 'json-summary', 'html'],
        reportsDirectory: '../../tmp/test-governance/coverage/frontend',
        allowExternal: true,
        include: ['src/**/*.{ts,tsx}', '../packages/page-runtime/src/**/*.ts'],
      },
    }
  };
});
