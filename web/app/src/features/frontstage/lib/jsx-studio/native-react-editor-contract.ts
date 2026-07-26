import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

export const FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS: readonly BlockSourceExtraLib[] =
  [
    {
      source: 'react',
      filePath: 'file:///node_modules/@types/react/index.d.ts',
      content: `declare module 'react' {
  export type CSSProperties = Record<string, string | number | undefined>;
  export const Fragment: unique symbol;
  export function useState<T>(initial: T): [T, (next: T | ((current: T) => T)) => void];
  export function useEffect(effect: () => void | (() => void), deps?: readonly unknown[]): void;
}

declare namespace JSX {
  interface IntrinsicElements {
    [elementName: string]: Record<string, unknown>;
  }
}
`
    },
    {
      source: '@1flowbase/native-react-context',
      filePath: 'file:///1flowbase/native-react-context.d.ts',
      content: `interface NativeReactBlockContext {
  props: Record<string, unknown>;
  inputs: Record<string, unknown>;
  outputs: {
    publish(values: Record<string, unknown>): { ok: boolean; stale: boolean; error?: string };
  };
  params: Record<string, unknown>;
  state: Record<string, unknown>;
  patch(next: Record<string, unknown>): void;
  theme: { mode: 'light' | 'dark'; tokens: Record<string, unknown> };
  ui: { locale?: string };
}

interface NativeReactBlockProps {
  ctx: NativeReactBlockContext;
}
`
    }
  ];
