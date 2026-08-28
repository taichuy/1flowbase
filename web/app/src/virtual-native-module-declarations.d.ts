declare module 'virtual:1flowbase-native-module-declarations' {
  import type { BlockSourceExtraLib } from './shared/code-block/extra-lib';

  const declarations: readonly BlockSourceExtraLib[];
  export default declarations;
}

declare module 'virtual:1flowbase-native-antd-es-modules' {
  export const ANTD_ES_MODULE_DEFINITIONS: readonly {
    module_source: string;
    exports: string[];
  }[];
  export function loadAntDesignEsModule(
    moduleSource: string
  ): Promise<Record<string, unknown>>;
}
