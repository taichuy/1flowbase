import { StyleProvider, createCache } from '@ant-design/cssinjs';
import { App as AntdApp, ConfigProvider } from 'antd';
import type { ConfigProviderProps } from 'antd/es/config-provider';
import {
  Component,
  useLayoutEffect,
  useMemo,
  useState,
  type ComponentType,
  type ErrorInfo,
  type ReactNode
} from 'react';
import { createPortal } from 'react-dom';

import type {
  BlockContext,
  BlockProtocolError
} from '@1flowbase/page-protocol';
import {
  attachNativeTrustedBlockPortalSurface,
  createNativeTrustedBlockPortalContainment,
  isNativeTrustedBlockRuntimeError,
  type NativeTrustedBlockPortalContainment,
  type NativeTrustedBlockPortalSurface,
  type NativeTrustedBlockPreparePlan,
  type NativeReactResolvedModuleAsset
} from '@1flowbase/page-runtime';

export interface FrontstageNativeTrustedBlockReactComponentProps {
  plan: NativeTrustedBlockPreparePlan;
  props: NativeTrustedBlockPreparePlan['props'];
  ctx: BlockContext;
  portalContainment: NativeTrustedBlockPortalContainment;
}

export type FrontstageNativeTrustedBlockReactComponent =
  ComponentType<FrontstageNativeTrustedBlockReactComponentProps>;

export interface FrontstageNativeTrustedBlockProviderScope {
  theme?: ConfigProviderProps['theme'];
  locale?: ConfigProviderProps['locale'];
}

export interface FrontstageNativeTrustedBlockProviderContext {
  plan: NativeTrustedBlockPreparePlan;
  root: Element;
  shadowRoot: ShadowRoot;
  mountElement: HTMLElement;
  portalContainment: NativeTrustedBlockPortalContainment;
}

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

export interface FrontstageNativeTrustedBlockPortalHostProps {
  root: Element;
  renderEpoch: string;
  plan: NativeTrustedBlockPreparePlan;
  component: FrontstageNativeTrustedBlockReactComponent;
  ctx: BlockContext;
  moduleAssets?: readonly NativeReactResolvedModuleAsset[];
  providerScope?: FrontstageNativeTrustedBlockProviderScope;
  providerWrapper?: FrontstageNativeTrustedBlockProviderWrapper;
  onRuntimeError?: FrontstageNativeTrustedBlockRuntimeErrorHandler;
}

/**
 * A declarative child of the owning surface React tree. The effect owns only
 * the Shadow DOM resource; React owns portal mount, update, and unmount.
 */
export function FrontstageNativeTrustedBlockPortalHost({
  root,
  renderEpoch,
  plan,
  component: BlockComponent,
  ctx,
  moduleAssets = [],
  providerScope,
  providerWrapper,
  onRuntimeError
}: FrontstageNativeTrustedBlockPortalHostProps): ReactNode {
  const [surface, setSurface] =
    useState<NativeTrustedBlockPortalSurface | null>(null);

  useLayoutEffect(() => {
    const nextSurface = attachNativeTrustedBlockPortalSurface({
      root,
      blockId: plan.blockId
    });
    setSurface(nextSurface);

    return () => {
      nextSurface.dispose();
    };
    // The root owns the Shadow DOM resource. renderEpoch is a React portal key,
    // so identity/retry replaces the component once without recreating DOM.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [root]);

  const styleCache = useMemo(() => (surface ? createCache() : null), [surface]);
  const portalContainment = useMemo(
    () => (surface ? createPortalContainment(surface.shadowRoot) : null),
    [surface]
  );

  useLayoutEffect(() => {
    if (!surface) return;
    const styles = moduleAssets
      .filter((asset) => asset.role === 'shadow_style')
      .map((asset) => {
        const element =
          surface.mountElement.ownerDocument.createElement('style');
        element.dataset.moduleSource = asset.module_source;
        element.dataset.assetSha256 = asset.sha256;
        element.textContent = new TextDecoder('utf-8', { fatal: true }).decode(
          asset.bytes
        );
        surface.shadowRoot.prepend(element);
        return element;
      });
    return () => styles.forEach((style) => style.remove());
  }, [moduleAssets, surface]);

  if (!surface || !styleCache || !portalContainment) return null;

  const providerContext: FrontstageNativeTrustedBlockProviderContext = {
    plan,
    root,
    shadowRoot: surface.shadowRoot,
    mountElement: surface.mountElement,
    portalContainment
  };
  const content = (
    <FrontstageNativeTrustedBlockErrorBoundary
      context={{ ...providerContext, blockId: plan.blockId }}
      onRuntimeError={onRuntimeError}
    >
      <BlockComponent
        plan={plan}
        props={plan.props}
        ctx={ctx}
        portalContainment={portalContainment}
      />
    </FrontstageNativeTrustedBlockErrorBoundary>
  );

  return createPortal(
    wrapWithHostProviders(
      content,
      providerContext,
      styleCache,
      providerScope,
      providerWrapper
    ),
    surface.mountElement,
    renderEpoch
  );
}

export function createFrontstageUnavailableBlockContext(
  plan: NativeTrustedBlockPreparePlan
): BlockContext {
  const state: Record<string, unknown> = {};

  return {
    currentUser: null,
    workspace: { id: 'workspace' },
    application: null,
    page: { id: plan.blockId, route: plan.blockId },
    inputs: {},
    outputs: {
      publish() {
        return {
          ok: false,
          stale: false,
          error: 'Native trusted block outputs are unavailable.'
        };
      }
    },
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
    return this.state.didCatch ? null : this.props.children;
  }
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
  return componentStack ? { ...context, componentStack } : context;
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === 'string' && error.trim() !== '') return error;
  return 'unknown error';
}

function wrapWithHostProviders(
  children: ReactNode,
  context: FrontstageNativeTrustedBlockProviderContext,
  styleCache: ReturnType<typeof createCache>,
  providerScope?: FrontstageNativeTrustedBlockProviderScope,
  providerWrapper?: FrontstageNativeTrustedBlockProviderWrapper
): ReactNode {
  const getShadowContainer = () => context.shadowRoot;
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

  return providerWrapper
    ? providerWrapper(scopedChildren, context)
    : scopedChildren;
}
