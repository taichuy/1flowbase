import * as antDesignIconsModule from '@ant-design/icons';
import * as antdModule from 'antd';
import * as ReactModule from 'react';
import * as ReactJsxRuntimeModule from 'react/jsx-runtime';
import * as uiModule from '@1flowbase/ui';
import {
  ANTD_ES_MODULE_DEFINITIONS,
  loadAntDesignEsModule
} from 'virtual:1flowbase-native-antd-es-modules';
import {
  DND_KIT_MODULE_DEFINITIONS,
  loadDndKitModule
} from 'virtual:1flowbase-native-dnd-kit-modules';

import {
  createNativeReactModuleRegistry,
  type NativeReactFrontendModuleLoadResult,
  type NativeReactFrontendModuleRegistration,
  type NativeReactModuleDefinition,
  type NativeReactModuleRegistry,
  type NativeTrustedBlockInjectedModuleMap
} from '@1flowbase/page-runtime';

import {
  ANT_DESIGN_COLORS_EXPORTS,
  loadAntDesignColorsModule
} from './ant-design-colors-runtime';
import { ANTD_STYLE_EXPORTS, loadAntdStyleModule } from './antd-style-runtime';
import { NativeBlockAffix } from './native-affix-runtime';
import { NativeBlockAnchor } from './native-anchor-runtime';
import { NativeBlockDropdown } from './native-dropdown-runtime';
import { NativeBlockMessage } from './native-message-runtime';
import { NativeBlockMenu } from './menu/native-menu-runtime';

type ModuleNamespace = Record<string, unknown>;

const nativeAntdModule = {
  ...antdModule,
  Affix: NativeBlockAffix,
  Anchor: NativeBlockAnchor,
  Dropdown: NativeBlockDropdown,
  Menu: NativeBlockMenu,
  message: NativeBlockMessage
};

const ANT_DESIGN_X_EXPORTS = [
  'Actions',
  'Attachments',
  'Bubble',
  'CodeHighlighter',
  'Conversations',
  'FileCard',
  'Folder',
  'Mermaid',
  'Prompts',
  'Sender',
  'SenderSwitch',
  'Sources',
  'Suggestion',
  'Think',
  'ThoughtChain',
  'Welcome',
  'XProvider',
  'notification',
  'version'
] as const;

const registrations: readonly NativeReactFrontendModuleRegistration[] = [
  registration('react', Object.keys(reactModule()), async () => ({
    module: reactModule()
  })),
  registration(
    'react/jsx-runtime',
    Object.keys(ReactJsxRuntimeModule),
    async () => ({ module: ReactJsxRuntimeModule })
  ),
  registration('antd', Object.keys(antdModule), async () => ({
    module: nativeAntdModule
  })),
  ...ANTD_ES_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(module_source, exports, async () => ({
      module: await loadAntDesignEsModule(module_source)
    }))
  ),
  ...DND_KIT_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(module_source, exports, async () => ({
      module: await loadDndKitModule(module_source)
    }))
  ),
  registration('@ant-design/colors', ANT_DESIGN_COLORS_EXPORTS, async () => ({
    module: await loadAntDesignColorsModule()
  })),
  registration('antd-style', ANTD_STYLE_EXPORTS, async () => ({
    module: await loadAntdStyleModule()
  })),
  registration('@1flowbase/ui', Object.keys(uiModule), async () => ({
    module: uiModule
  })),
  registration('@1flowbase/block-sdk', ['blockSdkVersion'], async () => ({
    module: await import('@1flowbase/block-sdk')
  })),
  registration(
    '@1flowbase/native-components',
    ['ScrollableSurface', 'Surface'],
    async () => {
      const [module, style] = await Promise.all([
        import('@1flowbase/native-components'),
        import('@1flowbase/native-components/styles.css?inline')
      ]);
      return { module, styles: [{ css: style.default }] };
    }
  ),
  registration(
    '@ant-design/icons',
    Object.keys(antDesignIconsModule),
    async () => ({
      module: antDesignIconsModule
    })
  ),
  registration('@1flowbase/charts', ['EChart'], async () => ({
    module: await import('@1flowbase/charts')
  })),
  registration('@1flowbase/rich-text', ['VditorEditor'], async () => {
    const [module, vditorStyle, richTextStyle] = await Promise.all([
      import('@1flowbase/rich-text'),
      import('vditor/dist/index.css?inline'),
      import('@1flowbase/rich-text/styles.css?inline')
    ]);
    return {
      module,
      styles: [{ css: vditorStyle.default }, { css: richTextStyle.default }]
    };
  }),
  registration('@ant-design/x', [...ANT_DESIGN_X_EXPORTS], async () => ({
    module: await import('@ant-design/x')
  })),
  registration(
    '@ant-design/x-markdown',
    ['default', 'XMarkdown', 'AnimationText', 'useStreaming', 'version'],
    async () => ({ module: await import('@ant-design/x-markdown') })
  )
];

export const FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS: readonly NativeReactModuleDefinition[] =
  registrations.map(({ module_source, exports }) => ({
    module_source,
    exports: [...exports]
  }));

let sharedRegistry: NativeReactModuleRegistry | undefined;

export function getFrontstageNativeReactModuleRegistry(): NativeReactModuleRegistry {
  sharedRegistry ??= createNativeReactModuleRegistry(registrations);
  return sharedRegistry;
}

export function createFrontstageNativeReactModuleRegistry(
  overrides: NativeTrustedBlockInjectedModuleMap = {}
): NativeReactModuleRegistry {
  if (Object.keys(overrides).length === 0) {
    return getFrontstageNativeReactModuleRegistry();
  }
  return createNativeReactModuleRegistry(
    registrations.map((entry) => {
      const override = overrides[entry.module_source];
      if (!override) return entry;
      return {
        ...entry,
        async load(): Promise<NativeReactFrontendModuleLoadResult> {
          const loaded = await entry.load();
          return { ...loaded, module: { ...loaded.module, ...override } };
        }
      };
    })
  );
}

function registration(
  moduleSource: string,
  exports: readonly string[],
  load: () => Promise<NativeReactFrontendModuleLoadResult>
): NativeReactFrontendModuleRegistration {
  return { module_source: moduleSource, exports: [...exports], load };
}

function reactModule(): ModuleNamespace {
  return {
    ...ReactModule,
    default: 'default' in ReactModule ? ReactModule.default : ReactModule
  };
}
