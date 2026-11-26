import type { Plugin } from 'vite';
import {
  assertSignInVisualBoundary,
  assertSignInBudget
} from './sign-in-visual-boundary';
import { collectScenarioDemandManifest } from './scenario-demand/manifest';

import { PRODUCT_ANT_DESIGN_ICON_RESOLVED_IDS } from './native-ant-design-icons-modules';
import {
  PRODUCT_ANTD_RESOLVED_CHUNKS,
  productAntDesignChunk
} from './native-antd-es-modules';

const SCENARIO_CHUNK_NAMES = new Set([
  'react-runtime',
  'app-runtime',
  'auth-anonymous',
  'auth-discovery',
  'auth-visual',
  'antd-core',
  'antd-form',
  'antd-data',
  'antd-picker',
  'antd-display',
  'shell-core',
  'frontstage-shell',
  'frontstage-canvas',
  'native-host',
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
  if (/^(?:es|lib)\/color-picker\/(?:util|color)\.js$/u.test(modulePath)) {
    return 'antd-overlay';
  }
  const normalizedSubpath = modulePath.replace(/^lib\//u, 'es/');
  return /^es\/[^/]+/u.test(normalizedSubpath)
    ? productAntDesignChunk(`antd/${normalizedSubpath}`)
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

const ANTD_SHELL_EQUIVALENCE_GROUPS = new Set([
  'antd-core',
  'antd-icons',
  'antd-overlay',
  'antd-feedback',
  'antd-navigation'
]);

const ANTD_DEMAND_PRIORITIES: ReadonlyArray<readonly [string, number]> = [
  ['antd-form', 3],
  ['antd-display', 2],
  ['antd-data', 1],
  ['antd-picker', 0]
];

// Rolldown assigns a static dependency closure to its highest-priority consumer.
// Keep React shared; never traverse dynamic form/route imports for the visual group.
// Coalesce modules by true scenario co-occurrence equivalence classes so production
// builds saturate parallel channels with coarse cache-stable chunks instead of
// queueing 20+ fragmented micro-chunks behind HTTP/1.1 connection limits.
export const scenarioCodeSplitting = {
  groups: [
    {
      name: 'react-runtime',
      test: /[\\/]node_modules[\\/](?:react|react-dom|scheduler)[\\/]/u,
      priority: 30
    },
    {
      name: 'auth-visual',
      test: /[\\/]antd[\\/]es[\\/](?:app|config-provider)[\\/]index\.js$/u,
      priority: 20
    },
    // Quotient equivalence class for pure node_modules authenticated runtimes
    // (TanStack Query/Router, i18next, Dayjs core/plugins without locales, active
    // zh_CN/en_US Antd string tables without antd-picker widget, and shared Babel helper).
    {
      name: 'app-runtime',
      test: /[\\/]@babel[\\/]runtime[\\/]helpers[\\/](?:esm[\\/])?objectWithoutProperties\.js$|[\\/]node_modules[\\/]dayjs[\\/](?!esm[\\/]locale[\\/]|locale[\\/])|[\\/]node_modules[\\/]@tanstack[\\/](?:react-query|query-core|react-router|router-core|history)[\\/]|[\\/]node_modules[\\/](?:i18next|react-i18next)[\\/]|[\\/](?:antd[\\/](?:es|lib)[\\/](?:locale|date-picker[\\/]locale|time-picker[\\/]locale|calendar[\\/]locale)|@rc-component[\\/](?:picker|pagination)[\\/](?:es|lib)[\\/]locale)[\\/].*(?:zh_CN|en_US|common)\.js$/u,
      priority: 7
    },
    // Coalesce core shell dictionaries per locale (quotient equivalence class on shell)
    // while keeping route-only settings/backstage dictionaries independently lazy.
    {
      name: 'i18n-shell-zh',
      test: /[\\/]src[\\/](?:app[\\/]i18n|app-shell[\\/]i18n|shared[\\/]i18n[\\/]resources|shared[\\/]ui[\\/]i18n)[\\/]zh_Hans\.json$/u,
      priority: 7
    },
    {
      name: 'i18n-shell-en',
      test: /[\\/]src[\\/](?:app[\\/]i18n|app-shell[\\/]i18n|shared[\\/]i18n[\\/]resources|shared[\\/]ui[\\/]i18n)[\\/]en_US\.json$/u,
      priority: 7
    },
    // Layer 1: Quotient equivalence class for authenticated startup + Topbar shell + route frame
    // (priority 5.5 < antd-core 8 so shell-core never swallows AntD widget groups).
    {
      name: 'shell-core',
      test: /[\\/]packages[\\/]ui[\\/]src[\\/]|[\\/]src[\\/](?:app[\\/](?:bootstrap[\\/]|AuthenticatedAppRuntime|AppProviders|AppI18nProvider|router)|app-shell[\\/](?!TopbarNavigationDesigner\.tsx$|_tests[\\/])|routes[\\/](?!LazyRouteResult\.tsx$|_tests[\\/])|state[\\/]navigation-query-scope\.ts$|features[\\/](?:auth[\\/](?:api[\\/]session-actions\.ts$|components[\\/]anonymous-loading[\\/]WorkspaceShellSkeleton\.tsx$)|frontstage[\\/](?:api[\\/]|(?:hooks|lib)[\\/]runtime-i18n[\\/])|settings[\\/]api[\\/]console-navigation\.ts$)|shared[\\/](?:api[\\/]system-metadata\.ts$|i18n[\\/](?:text|app-i18n|application-i18n-demand|application-i18n-resources|application-resource-loader)\.ts$|ui[\\/]window-workspace[\\/](?:WindowWorkspaceProvider|window-workspace-context)))/u,
      priority: 5.5
    },
    // Layer 2: Quotient equivalence class for Frontstage Left Sidebar + Page Tree shell
    // (renders immediately alongside Topbar; zero PageCanvas, react-grid-layout, or native-host!).
    {
      name: 'frontstage-shell',
      test: /[\\/]src[\\/]features[\\/]frontstage[\\/](?:pages[\\/](?:FrontstageWorkspacePage\.tsx$|frontstage-page[\\/](?:workspace-shell[\\/]|runtime-session[\\/]|use-page-tree-workspace\.ts$|page-tree-operations\.ts$|page-tree-form-modal\.tsx$|refresh-current-page\.ts$|page-constants\.ts$))|hooks[\\/]use-frontstage-page-tree-mutations\.ts$|components[\\/](?:FrontStagePageTreeSidebar\.tsx$|FrontstageNodeActionButton\.tsx$)|lib[\\/](?:page-tree\.ts$|page-tree-(?:icons[\\/]registry\.tsx$|state[\\/])|navigation-drag[\\/]|api-errors\.ts$|page-canvas[\\/](?:runtime-activity|runtime-assembly)\.ts$))/u,
      priority: 5
    },
    // Layer 3a: Quotient equivalence class for browser-side Native React host, cache,
    // lightweight host overlay/style facades, and virtual product/icon loader tables
    // (explicitly excluding main-thread Acorn/Sucrase compiler, Monaco Studio, editor declarations, charts, and sign-in sha256).
    {
      name: 'native-host',
      test: /^\0virtual:1flowbase-native-(?:antd-product-loaders|ant-design-icons-loaders)$|[\\/]packages[\\/](?:block-renderer[\\/]src[\\/]BlockUiLoadingShell\.tsx$|page-runtime[\\/]src[\\/](?:entrypoints[\\/]browser\.ts$|native-react-compiler[\\/](?:artifact-contract|artifact-evaluator|source-contract|module-registry[\\/])|native-block-context[\\/]|native-trusted-block-host|native-trusted-block-portal|native-trusted-block[\\/](?:runtime-error|source-policy-constants)|js-block-diagnostics|isolated-frontend-block-realm))|[\\/]src[\\/](?:shared[\\/]code-block[\\/](?:fetched-modules[\\/]|native-react-compiler-browser\.ts$|native-react-source-preparation\.ts$)|features[\\/]frontstage[\\/]lib[\\/](?:native-adapter[\\/](?!.*runtime-factory)|native-trusted-block[\\/]|runtime-cache[\\/]|native-modules[\\/](?:registry\.ts$|ant-design-colors-runtime\.ts$|antd-style-runtime\.ts$|native-block-surface-context\.ts$|native-dropdown-runtime\.tsx$|native-message-runtime\.tsx$|native-motion-runtime\.ts$|native-overlay-(?:host|layer)\.ts$|affix-anchor[\\/]|block-design[\\/]|menu[\\/]|modal[\\/]|notification[\\/]|surface[\\/]|overlay[\\/](?!native-(?:select|cascader|tree-select)-runtime))))/u,
      priority: 4
    },
    // Layer 3b: Quotient equivalence class for right-hand Frontstage Block Canvas orchestration
    // (loaded inside FrontstageWorkspaceShell's right pane without blocking Topbar or Sidebar).
    {
      name: 'frontstage-canvas',
      test: /[\\/]src[\\/]features[\\/]frontstage[\\/](?:pages[\\/](?:FrontStagePage\.tsx$|frontstage-page[\\/](?:controllers[\\/]|block-catalog-helpers\.ts$|page-action-helpers\.ts$))|state[\\/]|hooks[\\/](?!_tests|runtime-i18n[\\/]|use-frontstage-page-tree-mutations\.ts$)|components[\\/](?:PageCanvas\.tsx$|FrontstagePageTabs\.tsx$|canvas[\\/]|page-canvas[\\/]|data-blocks[\\/]DataBlockPageProvider\.tsx$))/u,
      priority: 4
    },
    // HTML starts discovery independently. Co-locate the lightweight anonymous
    // entry; authenticated/MCP/custom imports remain real dynamic boundaries.
    {
      name: 'auth-anonymous',
      test: /[\\/]src[\\/](?:bootstrap\.ts|main\.tsx|app[\\/](?:AnonymousAppRuntime|ApplicationRuntimeBootstrap)\.tsx|shared[\\/]api[\\/]system-metadata-seed\.ts|features[\\/]auth[\\/]api[\\/]early-console-startup-flight\.ts)$|vite[\\/]modulepreload-polyfill(?:\.js)?$/u,
      priority: 25
    },
    // Coalesced authenticated Ant Design shell/chrome primitives (zero table/list/tour/picker).
    {
      name: (id: string) => {
        const planned = planScenarioChunk(id);
        return planned && ANTD_SHELL_EQUIVALENCE_GROUPS.has(planned)
          ? 'antd-core'
          : null;
      },
      debugName: 'antd-core',
      priority: 8
    },
    ...ANTD_DEMAND_PRIORITIES.map(([groupName, priority]) => ({
      name: (id: string) =>
        planScenarioChunk(id) === groupName ? groupName : null,
      debugName: groupName,
      priority
    })),
    {
      name: (id: string) => {
        const planned = planScenarioChunk(id);
        if (!planned) return null;
        return ANTD_SHELL_EQUIVALENCE_GROUPS.has(planned)
          ? 'antd-core'
          : planned;
      },
      debugName: 'scenario-fallback'
    }
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
        const budget = assertSignInBudget(bundle);
        if (budget)
          this.emitFile({
            type: 'asset',
            fileName: 'sign-in-budget.json',
            source: JSON.stringify(budget, null, 2)
          });
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
        const signInSet = new Set(manifest.scenes.signIn?.staticClosure ?? []);
        const startupTierSet = new Set(
          (manifest.scenes.authenticatedStartup?.staticClosure ?? []).filter(
            (file) => file.endsWith('.js') && !signInSet.has(file)
          )
        );
        const shellI18nFiles = manifest.chunks
          .map((chunk) => chunk.file)
          .filter((file) => /\/i18n-shell-(?:zh|en)-[^/]+\.js$/u.test(`/${file}`));
        const shellTierSet = new Set(
          [
            ...(manifest.scenes.shell?.staticClosure ?? []),
            ...shellI18nFiles
          ].filter(
            (file) =>
              file.endsWith('.js') &&
              !signInSet.has(file) &&
              !startupTierSet.has(file)
          )
        );
        const workspaceShellTierSet = new Set(
          (manifest.scenes.workspaceShell?.staticClosure ?? []).filter(
            (file) =>
              file.endsWith('.js') &&
              !signInSet.has(file) &&
              !startupTierSet.has(file) &&
              !shellTierSet.has(file)
          )
        );
        const browseTierSet = new Set(
          (manifest.scenes.browse?.staticClosure ?? []).filter(
            (file) =>
              file.endsWith('.js') &&
              !signInSet.has(file) &&
              !startupTierSet.has(file) &&
              !shellTierSet.has(file) &&
              !workspaceShellTierSet.has(file)
          )
        );
        // Utility-tiered Shortest-Processing-Time First (SPT) paired with K=1 BDP pacing:
        // 1) authenticatedStartup (control-plane prefetch owner) first,
        // 2) shell (Topbar + active locale dictionaries) second,
        // 3) workspaceShell (Left Sidebar) third,
        // 4) browse (Right-hand Block Canvas) fourth.
        // Note: DedicatedWorker scripts are intentionally excluded from <link rel="modulepreload">
        // because Chromium's Window script module map does not share in-flight streams with new Worker().
        const byteLengthOf = (file: string): number => {
          const item = bundle[file];
          if (!item) return 0;
          if (item.type === 'chunk') return Buffer.byteLength(item.code ?? '');
          const src = item.source;
          return typeof src === 'string'
            ? Buffer.byteLength(src)
            : (src?.byteLength ?? 0);
        };
        const sortBySpt = (items: Iterable<string>): string[] =>
          [...items].sort(
            (left, right) =>
              byteLengthOf(left) - byteLengthOf(right) ||
              left.localeCompare(right)
          );
        const orderedPrewarmFiles = [
          ...sortBySpt(startupTierSet),
          ...sortBySpt(shellTierSet),
          ...sortBySpt(workspaceShellTierSet),
          ...sortBySpt(browseTierSet)
        ];
        this.emitFile({
          type: 'asset',
          fileName: 'authenticated-prewarm.json',
          source: `${JSON.stringify({ version: 1, files: orderedPrewarmFiles })}\n`
        });
      }
    }
  };
}
