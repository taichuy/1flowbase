import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

const REACT_DECLARATIONS = `declare module 'react' {
  export type ReactNode = unknown;
  export type CSSProperties = Record<string, string | number | undefined>;
  export type ComponentType<P = {}> = (props: P) => ReactNode;
  export type ElementType = string | ComponentType<any>;
  export interface HTMLAttributes<T> { className?: string; style?: CSSProperties; children?: ReactNode; [key: string]: unknown; }
  export const Fragment: ComponentType<{ children?: ReactNode }>;
  export function createElement(type: ElementType, props?: Record<string, unknown> | null, ...children: ReactNode[]): ReactNode;
  export function useState<T>(initial: T | (() => T)): [T, (value: T | ((previous: T) => T)) => void];
  export function useEffect(effect: () => void | (() => void), dependencies?: readonly unknown[]): void;
  export function useMemo<T>(factory: () => T, dependencies: readonly unknown[]): T;
  export function useCallback<T extends (...args: any[]) => any>(callback: T, dependencies: readonly unknown[]): T;
  export function useRef<T>(initial: T): { current: T };
  export function useSyncExternalStore<T>(subscribe: (listener: () => void) => () => void, getSnapshot: () => T): T;
  const React: { createElement: typeof createElement; Fragment: typeof Fragment };
  export default React;
}
`;

const BLOCK_SDK_DECLARATIONS = `declare module '@1flowbase/block-sdk' {
  export type BlockContextRecord = Record<string, unknown>;
  export interface BlockContextIdentity { readonly id: string; readonly displayName?: string; }
  export interface BlockContextEntity { readonly id: string; readonly name?: string; }
  export interface BlockContextPage { readonly id: string; readonly route: string; readonly title?: string; }
  export interface BlockContext {
    readonly currentUser: BlockContextIdentity | null;
    readonly workspace: BlockContextEntity;
    readonly application: BlockContextEntity | null;
    readonly page: BlockContextPage;
    readonly inputs: Readonly<BlockContextRecord>;
    readonly outputs: { publish(values: BlockContextRecord): { ok: boolean; stale: boolean; error?: string } };
    readonly params: Readonly<BlockContextRecord>;
    readonly props: Readonly<BlockContextRecord>;
    readonly state: Readonly<BlockContextRecord>;
    patch(next: BlockContextRecord): void | Promise<void>;
    readonly api: {
      get<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
      post<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
      put<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
      patch<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
      delete<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
    };
    readonly events: { emit(name: string, payload?: BlockContextRecord): void };
    readonly theme: { readonly mode: 'light' | 'dark'; readonly tokens: Readonly<BlockContextRecord> };
    readonly ui: { readonly locale?: string; readonly density?: 'compact' | 'comfortable' };
  }
  export interface BlockComponentProps { readonly ctx: BlockContext; }
  export const blockSdkVersion: string;
}
`;

export function createFrontendModuleExtraLib(
  moduleSource: string,
  exports: readonly string[]
): BlockSourceExtraLib {
  return {
    source: moduleSource,
    filePath: `file:///node_modules/${moduleSource}/index.d.ts`,
    content:
      moduleSource === 'react'
        ? REACT_DECLARATIONS
        : moduleSource === '@1flowbase/block-sdk'
          ? BLOCK_SDK_DECLARATIONS
          : createGenericModuleDeclarations(moduleSource, exports)
  };
}

function createGenericModuleDeclarations(
  moduleSource: string,
  exports: readonly string[]
): string {
  const namedExports = exports
    .filter((exportName) => exportName !== 'default')
    .map((exportName) => `  export const ${exportName}: any;`)
    .join('\n');
  const defaultExport = exports.includes('default')
    ? '\n  const defaultExport: any;\n  export default defaultExport;'
    : '';
  return `declare module '${moduleSource}' {\n${namedExports}${defaultExport}\n}\n`;
}
