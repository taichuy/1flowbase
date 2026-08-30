import { fileURLToPath, URL } from 'node:url';

import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv, searchForWorkspaceRoot } from 'vite';

import { nativeModuleDeclarationsPlugin } from './build/native-module-declarations';
import {
  collectAntDesignEsModuleSources,
  nativeAntDesignEsModulesPlugin
} from './build/native-antd-es-modules';
import {
  collectAntDesignIconModuleSources,
  nativeAntDesignIconsModulesPlugin
} from './build/native-ant-design-icons-modules';
import {
  collectDndKitModuleSources,
  nativeDndKitModulesPlugin
} from './build/native-dnd-kit-modules';
import {
  collectDayjsModuleSources,
  nativeDayjsModulesPlugin
} from './build/native-dayjs-modules';
import { FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES } from './src/features/frontstage/lib/native-modules/resolved-dependency-sources';
import { oneFlowbaseDevRuntimePlugin } from './vite/dev-runtime';

const reactDraggableBrowserDefines = {
  'process.env.DRAGGABLE_DEBUG': 'false'
};
const appRoot = fileURLToPath(new URL('.', import.meta.url));
const nativeAntDesignEsModuleSources = collectAntDesignEsModuleSources().map(
  ({ moduleSource }) => moduleSource
);
const devCriticalAntDesignModules = [
  'antd/es/alert',
  'antd/es/app',
  'antd/es/button',
  'antd/es/config-provider',
  'antd/es/input',
  'antd/es/skeleton',
  'antd/es/space',
  'antd/es/spin',
  'antd/es/theme',
  'antd/es/theme/themes/default',
  'antd/es/typography'
] as const;
const deferredNativeAntDesignEsModuleSources =
  nativeAntDesignEsModuleSources.filter(
    (moduleSource) =>
      !devCriticalAntDesignModules.includes(
        moduleSource as (typeof devCriticalAntDesignModules)[number]
      )
  );
const nativeAntDesignIconsModuleInventory = collectAntDesignIconModuleSources({
  projectRoot: appRoot
});
const nativeDndKitModuleInventory = collectDndKitModuleSources({
  projectRoot: appRoot
});
const nativeDndKitPackageRoots = [
  ...new Set(nativeDndKitModuleInventory.map(({ packageName }) => packageName))
];
const nativeDayjsModuleInventory = collectDayjsModuleSources({
  projectRoot: appRoot
});
const nativeDayjsDeclarationSources = nativeDayjsModuleInventory
  .filter(({ hasDeclaration }) => hasDeclaration)
  .map(({ moduleSource }) => moduleSource);

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

  if (isLazyAntDesignIconModule(id)) {
    return;
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

function isLazyAntDesignIconModule(id: string): boolean {
  return (
    /\/node_modules\/@ant-design\/icons\/es\/(?:index\.js|icons\/(?:index\.js|[^/]+\.js))$/u.test(
      id
    ) ||
    /\/node_modules\/@ant-design\/icons-svg\/(?:es|lib)\/asn\/[^/]+\.js$/u.test(
      id
    )
  );
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

export default defineConfig(({ command, mode }) => {
  const env = { ...loadEnv(mode, process.cwd(), ''), ...process.env };
  const isRemoteDebug = mode === 'remote-debug';
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
  const externalNpmProxyTarget = (
    env.VITE_EXTERNAL_NPM_PROXY_TARGET || 'http://127.0.0.1:4174'
  ).replace(/\/$/, '');

  return {
    ...(env.VITE_DEV_CACHE_DIR ? { cacheDir: env.VITE_DEV_CACHE_DIR } : {}),
    plugins: [
      oneFlowbaseDevRuntimePlugin({ root: process.cwd(), mode, command }),
      nativeAntDesignEsModulesPlugin(),
      nativeAntDesignIconsModulesPlugin({
        inventory: nativeAntDesignIconsModuleInventory
      }),
      nativeDndKitModulesPlugin({ inventory: nativeDndKitModuleInventory }),
      nativeDayjsModulesPlugin({ inventory: nativeDayjsModuleInventory }),
      nativeModuleDeclarationsPlugin({
        moduleSources: [
          ...FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES,
          ...nativeAntDesignEsModuleSources,
          ...nativeDndKitPackageRoots,
          ...nativeDayjsDeclarationSources
        ],
        projectRoot: appRoot
      }),
      react()
    ],
    define: reactDraggableBrowserDefines,
    optimizeDeps: {
      rolldownOptions: {
        output: {
          minify: true
        }
      },
      exclude: [
        '@ant-design/icons-svg',
        'dayjs',
        ...nativeDndKitPackageRoots,
        ...deferredNativeAntDesignEsModuleSources
      ],
      include: [
        '@1flowbase/api-client/auth',
        '@ant-design/icons',
        ...devCriticalAntDesignModules,
        '@ant-design/x-markdown',
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
        'antd',
        'copy-to-clipboard',
        'echarts',
        'lexical',
        'monaco-editor',
        'vditor'
      ]
    },
    build: {
      chunkSizeWarningLimit: 3500,
      sourcemap: isRemoteDebug,
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
      warmup: {
        clientFiles: [
          './src/bootstrap.ts',
          './src/main.tsx',
          './src/app/router.tsx',
          './src/features/frontstage/pages/FrontStagePage.tsx',
          './src/features/settings/pages/SettingsPage.tsx'
        ]
      },
      fs: {
        allow: [
          searchForWorkspaceRoot(process.cwd()),
          fileURLToPath(new URL('../../scripts', import.meta.url))
        ]
      },
      proxy: {
        '/external-npm': {
          target: externalNpmProxyTarget,
          changeOrigin: true
        },
        '/api': {
          target: apiProxyTarget,
          changeOrigin: true,
          ws: true
        },
        '/v1': {
          target: apiProxyTarget,
          changeOrigin: true,
          ws: true
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
        ...(command === 'serve'
          ? {
              '@ant-design/icons-svg/lib/asn': '@ant-design/icons-svg/es/asn'
            }
          : {}),
        '@1flowbase/shared-types': fileURLToPath(
          new URL('../packages/shared-types/src/index.ts', import.meta.url)
        ),
        '@1flowbase/api-client/auth': fileURLToPath(
          new URL('../packages/api-client/src/auth/index.ts', import.meta.url)
        ),
        '@1flowbase/api-client': fileURLToPath(
          new URL('../packages/api-client/src/index.ts', import.meta.url)
        ),
        '@1flowbase/block-renderer/loading-shell': fileURLToPath(
          new URL(
            '../packages/block-renderer/src/BlockUiLoadingShell.tsx',
            import.meta.url
          )
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
        '@1flowbase/ui/app-theme-provider': fileURLToPath(
          new URL('../packages/ui/src/app-theme-provider.tsx', import.meta.url)
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
        '@1flowbase/page-runtime/module-registry': fileURLToPath(
          new URL(
            '../packages/page-runtime/src/native-react-compiler/module-registry/contracts.ts',
            import.meta.url
          )
        ),
        '@1flowbase/page-runtime/source-contract': fileURLToPath(
          new URL(
            '../packages/page-runtime/src/native-react-compiler/source-contract.ts',
            import.meta.url
          )
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
