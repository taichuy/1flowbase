import { fileURLToPath, URL } from 'node:url';

import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv, searchForWorkspaceRoot } from 'vite';

const reactDraggableBrowserDefines = {
  'process.env.DRAGGABLE_DEBUG': 'false'
};

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

  if (
    id.includes('/antd/') ||
    id.includes('/@ant-design/') ||
    id.includes('/rc-')
  ) {
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

function parseAllowedHosts(value?: string) {
  const hosts = String(value || '')
    .split(',')
    .map((host) => host.trim())
    .filter(Boolean);

  return hosts.length > 0 ? hosts : undefined;
}

function parseAllowedOrigins(value?: string) {
  const origins = String(value || '')
    .split(',')
    .map((origin) => origin.trim())
    .filter(Boolean);

  if (origins.includes('*')) {
    return true;
  }

  return origins.length > 0 ? origins : undefined;
}

export default defineConfig(({ mode }) => {
  const env = { ...loadEnv(mode, process.cwd(), ''), ...process.env };
  const devServerPort = Number.parseInt(env.VITE_DEV_SERVER_PORT || '3100', 10);
  const devAllowedHosts = parseAllowedHosts(env.VITE_DEV_ALLOWED_HOSTS);
  const devCorsAllowedOrigins = parseAllowedOrigins(
    env.VITE_DEV_CORS_ALLOWED_ORIGINS
  );
  const apiProxyTarget = (
    env.VITE_API_PROXY_TARGET ||
    env.VITE_API_BASE_URL ||
    'http://127.0.0.1:7800'
  ).replace(/\/$/, '');

  return {
    plugins: [react()],
    define: reactDraggableBrowserDefines,
    optimizeDeps: {
      include: [
        '@lexical/react/LexicalComposer',
        '@lexical/react/LexicalComposerContext',
        '@lexical/react/LexicalContentEditable',
        '@lexical/react/LexicalErrorBoundary',
        '@lexical/react/LexicalHistoryPlugin',
        '@lexical/react/LexicalOnChangePlugin',
        '@lexical/react/LexicalRichTextPlugin',
        '@lexical/react/useLexicalNodeSelection',
        '@lexical/utils',
        '@monaco-editor/react',
        '@scalar/api-reference-react',
        '@xyflow/react',
        'copy-to-clipboard',
        'echarts',
        'lexical',
        'react-markdown',
        'remark-breaks',
        'remark-gfm',
        'vditor'
      ],
      esbuildOptions: {
        define: reactDraggableBrowserDefines
      }
    },
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
      ...(devAllowedHosts ? { allowedHosts: devAllowedHosts } : {}),
      ...(devCorsAllowedOrigins
        ? {
            cors: {
              origin: devCorsAllowedOrigins,
              credentials: true
            }
          }
        : {}),
      port:
        Number.isInteger(devServerPort) && devServerPort > 0
          ? devServerPort
          : 3100,
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
        '/v1': {
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
        '@1flowbase/block-renderer': fileURLToPath(
          new URL('../packages/block-renderer/src/index.tsx', import.meta.url)
        ),
        '@1flowbase/model-provider-contracts': fileURLToPath(
          new URL(
            '../../scripts/node/testing/contracts/model-providers',
            import.meta.url
          )
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
      testTimeout: 15_000,
      setupFiles: './src/test/setup.ts',
      coverage: {
        provider: 'v8',
        reporter: ['text-summary', 'json-summary', 'html'],
        reportsDirectory: '../../tmp/test-governance/coverage/frontend'
      }
    }
  };
});
