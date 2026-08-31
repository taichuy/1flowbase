import type { Plugin } from 'vite';

import { PRODUCT_ANT_DESIGN_ICON_RESOLVED_IDS } from './native-ant-design-icons-modules';
import {
  PRODUCT_ANTD_RESOLVED_CHUNKS,
  productAntDesignChunk
} from './native-antd-es-modules';

const SCENARIO_CHUNK_NAMES = new Set([
  'react-runtime',
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

export function scenarioChunkManifestPlugin(): Plugin {
  return {
    name: '1flowbase-scenario-chunk-manifest',
    generateBundle(_options, bundle) {
      const chunks = Object.values(bundle)
        .filter((output) => output.type === 'chunk')
        .filter((chunk) =>
          [...SCENARIO_CHUNK_NAMES].some(
            (name) => chunk.name === name || chunk.name.startsWith(`${name}~`)
          )
        )
        .map((chunk) => ({
          file: chunk.fileName,
          name: chunk.name,
          dynamicImports: [...chunk.dynamicImports].sort(),
          imports: [...chunk.imports].sort(),
          moduleCount: Object.keys(chunk.modules).length
        }))
        .sort((left, right) => left.file.localeCompare(right.file));
      this.emitFile({
        type: 'asset',
        fileName: 'scenario-chunk-manifest.json',
        source: `${JSON.stringify({ version: 1, partitions: chunks }, null, 2)}\n`
      });
    }
  };
}
