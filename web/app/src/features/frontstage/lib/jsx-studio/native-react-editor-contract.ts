import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

export const FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS: readonly BlockSourceExtraLib[] =
  [
    {
      source: '@1flowbase/native-react-jsx',
      filePath: 'file:///1flowbase/native-react-jsx.d.ts',
      content: `declare namespace JSX {
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
  currentUser: { id: string; displayName?: string } | null;
  workspace: { id: string; name?: string };
  application: { id: string; name?: string } | null;
  page: { id: string; route: string; title?: string };
  props: Record<string, unknown>;
  inputs: Record<string, unknown>;
  outputs: {
    publish(values: Record<string, unknown>): { ok: boolean; stale: boolean; error?: string };
  };
  params: Record<string, unknown>;
  state: Record<string, unknown>;
  patch(next: Record<string, unknown>): void;
  api: {
    get<TResponse = unknown>(path: string, request?: NativeReactApiRequest): Promise<TResponse>;
    post<TResponse = unknown>(path: string, request?: NativeReactApiRequest): Promise<TResponse>;
    put<TResponse = unknown>(path: string, request?: NativeReactApiRequest): Promise<TResponse>;
    patch<TResponse = unknown>(path: string, request?: NativeReactApiRequest): Promise<TResponse>;
    delete<TResponse = unknown>(path: string, request?: NativeReactApiRequest): Promise<TResponse>;
  };
  events: { emit(name: string, payload?: Record<string, unknown>): void };
  theme: { mode: 'light' | 'dark'; tokens: Record<string, unknown> };
  ui: { locale?: string };
}

interface NativeReactApiRequest {
  path?: Record<string, unknown>;
  query?: Record<string, unknown>;
  headers?: Record<string, unknown>;
  body?: unknown;
}

interface NativeReactBlockProps {
  ctx: NativeReactBlockContext;
}
`
    }
  ];
