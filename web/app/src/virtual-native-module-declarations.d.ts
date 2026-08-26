declare module 'virtual:1flowbase-native-module-declarations' {
  import type { BlockSourceExtraLib } from './shared/code-block/extra-lib';

  const declarations: readonly BlockSourceExtraLib[];
  export default declarations;
}
