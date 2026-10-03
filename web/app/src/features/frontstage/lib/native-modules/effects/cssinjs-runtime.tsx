import {
  StyleContext,
  StyleProvider as AntdStyleProvider,
  type StyleProviderProps
} from '@ant-design/cssinjs';
import { useContext } from 'react';

import { useNativeBlockSurface } from '../native-block-surface-context';

// CSS options are local; the Block runtime owns the cache and injection root.
export function StyleProvider(props: StyleProviderProps) {
  const inherited = useContext(StyleContext);
  const surface = useNativeBlockSurface();
  if (!surface)
    throw new Error('StyleProvider requires a native Block surface.');
  if (
    (props.container !== undefined && props.container !== surface.targetRoot) ||
    (props.cache !== undefined && props.cache !== inherited.cache)
  ) {
    throw new Error(
      'A Block StyleProvider must inherit its container and cache.'
    );
  }
  return (
    <AntdStyleProvider
      {...props}
      cache={inherited.cache}
      container={surface.targetRoot}
    />
  );
}
