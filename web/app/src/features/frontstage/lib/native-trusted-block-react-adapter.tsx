import {
  Component,
  type ComponentType,
  type ErrorInfo,
  type ReactNode
} from 'react';
import { StyleProvider, createCache } from '@ant-design/cssinjs';
import { App as AntdApp, ConfigProvider } from 'antd';
import type { ConfigProviderProps } from 'antd/es/config-provider';
import { createRoot as defaultCreateRoot } from 'react-dom/client';

import type {
  BlockContext,
  BlockProtocolError
} from '@1flowbase/page-protocol';
import {
  createNativeTrustedBlockPortalContainment,
  isNativeTrustedBlockRuntimeError,
  type NativeTrustedBlockHostAdapter,
  type NativeTrustedBlockPortalContainment,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';

const NATIVE_TRUSTED_BLOCK_ROOT_ATTRIBUTE =
  'data-flowbase-native-trusted-block-root';
const NATIVE_TRUSTED_BLOCK_ID_ATTRIBUTE =
  'data-flowbase-native-trusted-block-id';
const NATIVE_TRUSTED_BLOCK_MOUNT_ATTRIBUTE =
  'data-flowbase-native-trusted-block-mount';

export interface FrontstageNativeTrustedBlockReactComponentProps {
  plan: NativeTrustedBlockPreparePlan;
  props: NativeTrustedBlockPreparePlan['props'];
  ctx: BlockContext;
  portalContainment: NativeTrustedBlockPortalContainment;
}

export type FrontstageNativeTrustedBlockReactComponent =
  ComponentType<FrontstageNativeTrustedBlockReactComponentProps>;

export type FrontstageNativeTrustedBlockResolveComponent = (
  plan: NativeTrustedBlockPreparePlan
) => FrontstageNativeTrustedBlockReactComponent;

export interface FrontstageNativeTrustedBlockReactRoot {
  render(children: ReactNode): void;
  unmount(): void;
}

export type FrontstageNativeTrustedBlockCreateRoot = (
  root: Element
) => FrontstageNativeTrustedBlockReactRoot;

export interface FrontstageNativeTrustedBlockProviderContext {
  plan: NativeTrustedBlockPreparePlan;
  root: Element;
  shadowRoot: ShadowRoot;
  mountElement: HTMLElement;
  portalContainment: NativeTrustedBlockPortalContainment;
}

export interface FrontstageNativeTrustedBlockProviderScope {
  theme?: ConfigProviderProps['theme'];
  locale?: ConfigProviderProps['locale'];
}

export type FrontstageNativeTrustedBlockResolveProviderScope = (
  context: FrontstageNativeTrustedBlockProviderContext
) => FrontstageNativeTrustedBlockProviderScope | undefined;

export type FrontstageNativeTrustedBlockResolveContext = (
  context: FrontstageNativeTrustedBlockProviderContext
) => BlockContext;

export type FrontstageNativeTrustedBlockProviderWrapper = (
  children: ReactNode,
  context: FrontstageNativeTrustedBlockProviderContext
) => ReactNode;

export interface FrontstageNativeTrustedBlockRuntimeErrorContext extends FrontstageNativeTrustedBlockProviderContext {
  blockId: string;
  componentStack?: string;
}

export type FrontstageNativeTrustedBlockRuntimeErrorHandler = (
  error: BlockProtocolError,
  context: FrontstageNativeTrustedBlockRuntimeErrorContext
) => void;

export interface FrontstageNativeTrustedBlockReactAdapterOptions {
  resolveComponent: FrontstageNativeTrustedBlockResolveComponent;
  createRoot?: FrontstageNativeTrustedBlockCreateRoot;
  resolveBlockContext?: FrontstageNativeTrustedBlockResolveContext;
  resolveProviderScope?: FrontstageNativeTrustedBlockResolveProviderScope;
  providerWrapper?: FrontstageNativeTrustedBlockProviderWrapper;
  onRuntimeError?: FrontstageNativeTrustedBlockRuntimeErrorHandler;
}

export function createFrontstageNativeTrustedBlockReactAdapter(
  options: FrontstageNativeTrustedBlockReactAdapterOptions
): NativeTrustedBlockHostAdapter {
  const ownedShadowRoots = new WeakMap<Element, ShadowRoot>();
  const activeRoots = new WeakSet<Element>();

  return {
    async mount(input) {
      const rootElement = validateRootElement(input.root);
      const shadowSurface = createNativeTrustedBlockShadowSurface(
        rootElement,
        input.plan.blockId,
        ownedShadowRoots,
        activeRoots
      );
      const styleScope = applyNativeTrustedBlockStyleScope(
        rootElement,
        input.plan.blockId
      );
      let reactRoot: FrontstageNativeTrustedBlockReactRoot | undefined;
      let didUnmount = false;

      try {
        const portalContainment = createPortalContainment(
          shadowSurface.shadowRoot
        );
        reactRoot = (options.createRoot ?? defaultCreateRoot)(
          shadowSurface.mountElement
        );
        const styleCache = createCache();
        const renderPlan = (plan: NativeTrustedBlockPreparePlan) => {
          const Component = options.resolveComponent(plan);
          const providerContext = {
            plan,
            root: rootElement,
            shadowRoot: shadowSurface.shadowRoot,
            mountElement: shadowSurface.mountElement,
            portalContainment
          };
          const blockContext = resolveControlledBlockContext(
            providerContext,
            options.resolveBlockContext
          );
          reactRoot?.render(
            wrapWithHostProviders(
              <FrontstageNativeTrustedBlockErrorBoundary
                context={{ ...providerContext, blockId: plan.blockId }}
                onRuntimeError={options.onRuntimeError}
              >
                <Component
                  plan={plan}
                  props={plan.props}
                  ctx={blockContext}
                  portalContainment={portalContainment}
                />
              </FrontstageNativeTrustedBlockErrorBoundary>,
              providerContext,
              styleCache,
              options.resolveProviderScope,
              options.providerWrapper
            )
          );
        };
        renderPlan(input.plan);

        return {
          update(plan) {
            if (didUnmount) return;
            renderPlan(plan);
          },
          dispose() {
            if (didUnmount) {
              return;
            }

            didUnmount = true;
            try {
              reactRoot?.unmount();
            } finally {
              try {
                shadowSurface.dispose();
              } finally {
                styleScope.restore();
              }
            }
          }
        };
      } catch (error) {
        try {
          reactRoot?.unmount();
        } finally {
          try {
            shadowSurface.dispose();
          } finally {
            styleScope.restore();
          }
        }
        throw error;
      }
    }
  };
}

function resolveControlledBlockContext(
  context: FrontstageNativeTrustedBlockProviderContext,
  resolveBlockContext: FrontstageNativeTrustedBlockResolveContext | undefined
): BlockContext {
  return (
    resolveBlockContext?.(context) ??
    createFrontstageUnavailableBlockContext(context.plan)
  );
}

export function createFrontstageUnavailableBlockContext(
  plan: NativeTrustedBlockPreparePlan
): BlockContext {
  const state: Record<string, unknown> = {};

  return {
    currentUser: null,
    workspace: { id: 'workspace' },
    application: { id: 'application' },
    page: {
      id: plan.blockId,
      route: plan.blockId
    },
    inputs: {},
    params: {},
    props: { ...plan.props },
    state,
    patch(patch) {
      Object.assign(state, patch);
    },
    api: {
      get: rejectUnavailable('ctx.api.get'),
      post: rejectUnavailable('ctx.api.post'),
      put: rejectUnavailable('ctx.api.put'),
      patch: rejectUnavailable('ctx.api.patch'),
      delete: rejectUnavailable('ctx.api.delete'),
      head: rejectUnavailable('ctx.api.head'),
      options: rejectUnavailable('ctx.api.options'),
      stream: () => ({
        [Symbol.asyncIterator]() {
          return {
            next: async () => {
              throw createUnavailableContextError('ctx.api.stream');
            }
          };
        }
      })
    },
    events: {
      emit() {
        throw createUnavailableContextError('ctx.events.emit');
      }
    },
    theme: { mode: 'light', tokens: {} },
    ui: {}
  };
}

function rejectUnavailable<Args extends unknown[]>(
  capability: string
): (...args: Args) => Promise<never> {
  return async () => {
    throw createUnavailableContextError(capability);
  };
}

function createUnavailableContextError(capability: string): Error {
  return new Error(
    `Native trusted block ${capability} is unavailable until the host injects a controlled BlockContext.`
  );
}

interface FrontstageNativeTrustedBlockErrorBoundaryProps {
  children: ReactNode;
  context: FrontstageNativeTrustedBlockRuntimeErrorContext;
  onRuntimeError?: FrontstageNativeTrustedBlockRuntimeErrorHandler;
}

interface FrontstageNativeTrustedBlockErrorBoundaryState {
  didCatch: boolean;
}

class FrontstageNativeTrustedBlockErrorBoundary extends Component<
  FrontstageNativeTrustedBlockErrorBoundaryProps,
  FrontstageNativeTrustedBlockErrorBoundaryState
> {
  state: FrontstageNativeTrustedBlockErrorBoundaryState = { didCatch: false };

  static getDerivedStateFromError(): FrontstageNativeTrustedBlockErrorBoundaryState {
    return { didCatch: true };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    this.props.onRuntimeError?.(
      createRuntimeRenderError(error),
      createRuntimeErrorContext(this.props.context, errorInfo)
    );
  }

  render(): ReactNode {
    if (this.state.didCatch) {
      return null;
    }

    return this.props.children;
  }
}

function validateRootElement(root: unknown): Element {
  if (typeof Element === 'undefined' || !(root instanceof Element)) {
    throw new Error(
      'Native trusted block React adapter root must be a DOM Element.'
    );
  }

  return root;
}

function createPortalContainment(
  root: ShadowRoot
): NativeTrustedBlockPortalContainment {
  const result = createNativeTrustedBlockPortalContainment({ root });
  if (!result.ok) {
    throw new Error(
      result.errors.map((error) => error.message).join(' ') ||
        'Native trusted block portal containment creation failed.'
    );
  }

  return result.containment;
}

function createNativeTrustedBlockShadowSurface(
  root: Element,
  blockId: string,
  ownedShadowRoots: WeakMap<Element, ShadowRoot>,
  activeRoots: WeakSet<Element>
): {
  shadowRoot: ShadowRoot;
  mountElement: HTMLElement;
  dispose(): void;
} {
  if (activeRoots.has(root)) {
    throw new Error('Native trusted block root is already mounted.');
  }

  let shadowRoot = ownedShadowRoots.get(root);
  if (!shadowRoot) {
    if (root.shadowRoot) {
      throw new Error(
        'Native trusted block root must not contain a pre-existing ShadowRoot.'
      );
    }
    shadowRoot = root.attachShadow({ mode: 'open' });
    ownedShadowRoots.set(root, shadowRoot);
  }

  shadowRoot.replaceChildren();
  const mountElement = document.createElement('div');
  mountElement.setAttribute(NATIVE_TRUSTED_BLOCK_MOUNT_ATTRIBUTE, '');
  mountElement.setAttribute(NATIVE_TRUSTED_BLOCK_ID_ATTRIBUTE, blockId);
  shadowRoot.append(mountElement);
  activeRoots.add(root);

  let didDispose = false;
  return {
    shadowRoot,
    mountElement,
    dispose() {
      if (didDispose) return;
      didDispose = true;
      activeRoots.delete(root);
      shadowRoot.replaceChildren();
    }
  };
}

function createRuntimeRenderError(error: unknown): BlockProtocolError {
  if (isNativeTrustedBlockRuntimeError(error) && error.errors.length > 0) {
    return error.errors[0];
  }

  return {
    code: 'runtime_error',
    path: 'runtime.render',
    message: getErrorMessage(error)
  };
}

function createRuntimeErrorContext(
  context: FrontstageNativeTrustedBlockRuntimeErrorContext,
  errorInfo: ErrorInfo
): FrontstageNativeTrustedBlockRuntimeErrorContext {
  const componentStack = errorInfo.componentStack?.trim();

  if (!componentStack) {
    return context;
  }

  return {
    ...context,
    componentStack
  };
}

interface NativeTrustedBlockStyleScopeSnapshot {
  attribute: string;
  value: string | null;
}

function applyNativeTrustedBlockStyleScope(
  root: Element,
  blockId: string
): { restore(): void } {
  const snapshots: NativeTrustedBlockStyleScopeSnapshot[] = [
    snapshotAttribute(root, NATIVE_TRUSTED_BLOCK_ROOT_ATTRIBUTE),
    snapshotAttribute(root, NATIVE_TRUSTED_BLOCK_ID_ATTRIBUTE)
  ];

  root.setAttribute(NATIVE_TRUSTED_BLOCK_ROOT_ATTRIBUTE, '');
  root.setAttribute(NATIVE_TRUSTED_BLOCK_ID_ATTRIBUTE, blockId);

  return {
    restore() {
      snapshots.forEach((snapshot) => {
        if (snapshot.value === null) {
          root.removeAttribute(snapshot.attribute);
          return;
        }

        root.setAttribute(snapshot.attribute, snapshot.value);
      });
    }
  };
}

function snapshotAttribute(
  root: Element,
  attribute: string
): NativeTrustedBlockStyleScopeSnapshot {
  return {
    attribute,
    value: root.getAttribute(attribute)
  };
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === 'string' && error.trim() !== '') {
    return error;
  }

  return 'unknown error';
}

function wrapWithHostProviders(
  children: ReactNode,
  context: FrontstageNativeTrustedBlockProviderContext,
  styleCache: ReturnType<typeof createCache>,
  resolveProviderScope?: FrontstageNativeTrustedBlockResolveProviderScope,
  providerWrapper?: FrontstageNativeTrustedBlockProviderWrapper
): ReactNode {
  const getShadowContainer = () => context.shadowRoot;
  const providerScope = resolveProviderScope?.(context);
  const scopedChildren = (
    <StyleProvider autoClear cache={styleCache} container={context.shadowRoot}>
      <ConfigProvider
        getPopupContainer={getShadowContainer}
        getTargetContainer={getShadowContainer}
        locale={providerScope?.locale}
        theme={providerScope?.theme}
      >
        <AntdApp>{children}</AntdApp>
      </ConfigProvider>
    </StyleProvider>
  );

  if (providerWrapper) {
    return providerWrapper(scopedChildren, context);
  }

  return scopedChildren;
}
