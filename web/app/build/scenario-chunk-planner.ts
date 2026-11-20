import type { Plugin } from 'vite';
import { assertSignInVisualBoundary } from './sign-in-visual-boundary';
import { collectScenarioDemandManifest } from './scenario-demand/manifest';

import { PRODUCT_ANT_DESIGN_ICON_RESOLVED_IDS } from './native-ant-design-icons-modules';
import {
  PRODUCT_ANTD_RESOLVED_CHUNKS,
  productAntDesignChunk
} from './native-antd-es-modules';

const SCENARIO_CHUNK_NAMES = new Set([
  'react-runtime',
  'auth-surface',
  'auth-route',
  'auth-discovery',
  'auth-visual',
  'auth-form',
  'antd-core',
  'antd-overlay',
  'antd-data',
  'antd-feedback',
  'antd-navigation',
  'antd-input',
  'antd-display',
  'antd-icons',
  'workflow-canvas',
  'assistant-core',
  'assistant-activity',
  'assistant-markdown',
  'monaco-runtime'
]);

/**
 * Stable semantic partitions for dependencies that strongly co-occur in real
 * product scenarios. Application feature modules remain route/demand chunks.
 */
interface ScenarioChunkContext {
  getModuleInfo: (id: string) => { importers: readonly string[] } | null;
}

function installedAntDesignChunk(id: string): string | undefined {
  if (
    id.includes('/node_modules/@ant-design/cssinjs/') ||
    id.includes('/node_modules/@ant-design/colors/') ||
    id.includes('/node_modules/@ant-design/fast-color/')
  ) {
    return 'antd-core';
  }
  const marker = '/node_modules/antd/';
  const index = id.lastIndexOf(marker);
  if (index < 0) return undefined;
  const modulePath = id.slice(index + marker.length);
  if (/^(?:es|lib)\/index\.js$/u.test(modulePath)) return undefined;
  const component = /^(?:es|lib)\/([^/]+)/u.exec(modulePath)?.[1];
  return component
    ? productAntDesignChunk(`antd/es/${component}`)
    : 'antd-core';
}

export function planScenarioChunk(
  id: string,
  _context?: ScenarioChunkContext
): string | undefined {
  const normalized = id.replaceAll('\\', '/');
  if (PRODUCT_ANT_DESIGN_ICON_RESOLVED_IDS.has(id)) {
    return 'antd-icons';
  }
  const antChunk =
    PRODUCT_ANTD_RESOLVED_CHUNKS.get(id) ?? installedAntDesignChunk(normalized);
  if (antChunk) return antChunk;
  if (
    normalized.includes('/node_modules/react/') ||
    normalized.includes('/node_modules/react-dom/') ||
    normalized.includes('/node_modules/scheduler/')
  ) {
    return 'react-runtime';
  }
  return undefined;
}

// Rolldown assigns a static dependency closure to its highest-priority consumer.
// Keep React shared; never traverse dynamic form/route imports for the visual group.
export const scenarioCodeSplitting = {
  groups: [
    {
      name: 'react-runtime',
      test: /[\\/]node_modules[\\/](?:react|react-dom|scheduler)[\\/]/u,
      priority: 30
    },
    {
      name: 'auth-visual',
      test: /[\\/]antd[\\/]es[\\/](?:app|config-provider|skeleton|spin|alert)[\\/]index\.js$|[\\/]icons[\\/]es[\\/]icons[\\/]LoadingOutlined\.js$/u,
      priority: 20
    },
    {
      name: 'auth-form',
      test: /[\\/]antd[\\/]es[\\/](?:button|input|space)[\\/]index\.js$|[\\/]icons[\\/]es[\\/]icons[\\/]ArrowLeftOutlined\.js$/u,
      priority: 10
    },
    // Keep the tiny session request runnable before the route graph arrives.
    {
      name: 'auth-discovery',
      test: /[\\/]features[\\/]auth[\\/]api[\\/](?:auth-session-discovery|login-entry-discovery)\.ts$|[\\/]hero[\\/]document-motion\.ts$|vite[\\/]preload-helper(?:\.js)?$/u,
      priority: 25
    },
    // antd-style's ShadowRoot provider uses exactly these Ant Design roots.
    // Preserve that provider (including transitive style hooks), without bundling
    // its imports together with unrelated tables, displays and overlays.
    // A Babel destructuring helper is shared by antd-style and display widgets.
    // Isolate this helper, not the antd-style barrel, before generic widget partitions claim it.
    {
      name: 'shared-object-rest',
      test: /[\\/]@babel[\\/]runtime[\\/]helpers[\\/](?:esm[\\/])?objectWithoutProperties\.js$/u,
      priority: 7
    },
    {
      name: 'auth-surface',
      test: /[\\/]antd[\\/]es[\\/](?:theme|grid|modal|message|notification|version)[\\/]index\.js$/u,
      priority: 8
    },
    // Coalesce the route's eager imports; its dynamic form and MCP edges stay lazy.
    {
      name: 'auth-route',
      test: /[\\/]app[\\/]AnonymousAppRuntime\.tsx$/u,
      priority: 5
    },
    { name: (id: string) => planScenarioChunk(id) ?? null }
  ]
};

export function scenarioChunkManifestPlugin(): Plugin {
  return {
    name: '1flowbase-scenario-chunk-manifest',
    enforce: 'post',
    // Vite removes pure-CSS placeholder chunks and transfers their CSS edges
    // in generateBundle. Record the final graph, not the pre-transformation one.
    generateBundle: {
      order: 'post',
      handler(_options, bundle) {
        assertSignInVisualBoundary(bundle);
        const manifest = collectScenarioDemandManifest(bundle);
        const partitions = manifest.chunks.filter((chunk) =>
          [...SCENARIO_CHUNK_NAMES].some(
            (name) => chunk.name === name || chunk.name.startsWith(`${name}~`)
          )
        );
        this.emitFile({
          type: 'asset',
          fileName: 'scenario-chunk-manifest.json',
          source: `${JSON.stringify({ ...manifest, partitions }, null, 2)}\n`
        });
      }
    }
  };
}
