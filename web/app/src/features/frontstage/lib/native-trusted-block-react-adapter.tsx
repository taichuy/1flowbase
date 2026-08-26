import { StyleProvider, createCache } from '@ant-design/cssinjs';
import { App as AntdApp, ConfigProvider } from 'antd';
import type { ConfigProviderProps } from 'antd/es/config-provider';
import {
  Component,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ComponentType,
  type ErrorInfo,
  type ReactNode
} from 'react';
import { createPortal } from 'react-dom';

import type {
  BlockContext,
  BlockContextSeed,
  BlockProtocolError
} from '@1flowbase/page-protocol';
import {
  attachNativeTrustedBlockPortalSurface,
  createNativeBlockExternalAssetScope,
  createNativeTrustedBlockPortalContainment,
  isNativeTrustedBlockRuntimeError,
  type NativeBlockExternalAssetScope,
  type NativeTrustedBlockPortalContainment,
  type NativeTrustedBlockPortalSurface,
  type NativeTrustedBlockPreparePlan,
  type NativeReactResolvedModuleAsset
} from '@1flowbase/page-runtime';

import { i18nText } from '../../../shared/i18n/text';

import type {
  PreparedTrustedFrontendContribution,
  TrustedFrontendContributionHandle
} from './native-trusted-block-contribution-lifecycle';

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

const sharedModuleStyleSheets = new WeakMap<
  Document,
  Map<string, CSSStyleSheet>
>();

export interface FrontstageNativeTrustedBlockPortalHostProps {
  root: Element;
  renderEpoch: string;
  plan: NativeTrustedBlockPreparePlan;
  component: FrontstageNativeTrustedBlockReactComponent;
  ctx: BlockContextSeed;
  moduleAssets?: readonly NativeReactResolvedModuleAsset[];
  providerScope?: FrontstageNativeTrustedBlockProviderScope;
  providerWrapper?: FrontstageNativeTrustedBlockProviderWrapper;
  onRuntimeError?: FrontstageNativeTrustedBlockRuntimeErrorHandler;
  contribution?: PreparedTrustedFrontendContribution;
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
  onRuntimeError,
  contribution
}: FrontstageNativeTrustedBlockPortalHostProps): ReactNode {
  const [surface, setSurface] =
    useState<NativeTrustedBlockPortalSurface | null>(null);
  const lifecycleRef = useRef<TrustedFrontendContributionHandle | null>(null);
  const renderInput = useMemo(
    () => ({ plan, BlockComponent, ctx, moduleAssets, providerScope }),
    [BlockComponent, ctx, moduleAssets, plan, providerScope]
  );
  const mountedRenderInput = useRef<typeof renderInput | null>(null);

  useEffect(() => {
    const lifecycle = contribution?.createHandle() ?? null;
    lifecycleRef.current = lifecycle;
    let nextSurface: NativeTrustedBlockPortalSurface | null = null;
    const attachSurface = () => {
      nextSurface = attachNativeTrustedBlockPortalSurface({
        root,
        blockId: plan.blockId
      });
    };
    if (lifecycle) {
      lifecycle.mount({
        mount: attachSurface,
        dispose: () => nextSurface?.dispose()
      });
    } else {
      attachSurface();
    }
    const mountedSurface =
      nextSurface as NativeTrustedBlockPortalSurface | null;
    if (!mountedSurface) {
      throw new Error(
        i18nText('frontstage', 'auto.runtime_preview_unavailable')
      );
    }
    mountedRenderInput.current = renderInput;
    setSurface(mountedSurface);

    return () => {
      mountedRenderInput.current = null;
      lifecycleRef.current = null;
      if (lifecycle) {
        lifecycle.dispose();
      } else {
        mountedSurface.dispose();
      }
    };
    // Surface disposal stays in the passive phase so React removes every
    // portal-owned ShadowRoot child before the surface clears host-owned DOM.
    // renderEpoch remains the React portal key and does not recreate the DOM.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [root, contribution]);

  useEffect(() => {
    const lifecycle = lifecycleRef.current;
    if (!lifecycle || mountedRenderInput.current === renderInput) return;
    lifecycle.update();
    mountedRenderInput.current = renderInput;
  }, [renderInput]);

  const styleCache = useMemo(() => (surface ? createCache() : null), [surface]);
  const portalContainment = useMemo(
    () => (surface ? createPortalContainment(surface.shadowRoot) : null),
    [surface]
  );
  const externalAssetScope = useMemo<NativeBlockExternalAssetScope | null>(
    () =>
      surface
        ? createNativeBlockExternalAssetScope({ root: surface.shadowRoot })
        : null,
    [surface]
  );
  const blockContext = useMemo<BlockContext | null>(
    () =>
      surface && externalAssetScope
        ? {
            ...ctx,
            root: surface.shadowRoot,
            assets: externalAssetScope.assets
          }
        : null,
    [ctx, externalAssetScope, surface]
  );

  useLayoutEffect(
    () => () => externalAssetScope?.dispose(),
    [externalAssetScope]
  );

  useLayoutEffect(() => {
    if (!surface) return;
    return attachModuleStyleAssets(surface, moduleAssets);
  }, [moduleAssets, surface]);

  if (!surface || !styleCache || !portalContainment || !blockContext) {
    return null;
  }

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
        ctx={blockContext}
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

function attachModuleStyleAssets(
  surface: NativeTrustedBlockPortalSurface,
  moduleAssets: readonly NativeReactResolvedModuleAsset[]
): () => void {
  const styleAssets = moduleAssets.filter(
    (asset) => asset.role === 'shadow_style'
  );
  const ownerDocument = surface.mountElement.ownerDocument;
  const StyleSheet = ownerDocument.defaultView?.CSSStyleSheet;
  if (
    StyleSheet &&
    typeof StyleSheet.prototype.replaceSync === 'function' &&
    'adoptedStyleSheets' in surface.shadowRoot
  ) {
    let documentSheets = sharedModuleStyleSheets.get(ownerDocument);
    if (!documentSheets) {
      documentSheets = new Map();
      sharedModuleStyleSheets.set(ownerDocument, documentSheets);
    }
    const adopted = styleAssets.map((asset) => {
      let sheet = documentSheets.get(asset.sha256);
      if (!sheet) {
        sheet = new StyleSheet();
        sheet.replaceSync(decodeModuleStyle(asset));
        documentSheets.set(asset.sha256, sheet);
      }
      return sheet;
    });
    surface.shadowRoot.adoptedStyleSheets = [
      ...surface.shadowRoot.adoptedStyleSheets,
      ...adopted
    ];
    return () => {
      surface.shadowRoot.adoptedStyleSheets =
        surface.shadowRoot.adoptedStyleSheets.filter(
          (sheet) => !adopted.includes(sheet)
        );
    };
  }

  const styles = styleAssets.map((asset) => {
    const element = ownerDocument.createElement('style');
    element.dataset.moduleSource = asset.module_source;
    element.dataset.assetSha256 = asset.sha256;
    element.textContent = decodeModuleStyle(asset);
    surface.shadowRoot.prepend(element);
    return element;
  });
  return () => styles.forEach((style) => style.remove());
}

function decodeModuleStyle(asset: NativeReactResolvedModuleAsset): string {
  return new TextDecoder('utf-8', { fatal: true }).decode(asset.bytes);
}

export function createFrontstageUnavailableBlockContext(
  plan: NativeTrustedBlockPreparePlan
): BlockContextSeed {
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
    navigation: {
      openBlock() {
        throw createUnavailableContextError('ctx.navigation.openBlock');
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
  const isolatedPrefix = createShadowStylePrefix(context.plan.blockId);
  const scopedChildren = (
    <StyleProvider cache={styleCache} container={context.shadowRoot}>
      <ConfigProvider
        getPopupContainer={getShadowContainer}
        getTargetContainer={getShadowContainer}
        locale={providerScope?.locale}
        prefixCls={isolatedPrefix}
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

function createShadowStylePrefix(blockId: string): string {
  let hash = 2166136261;
  for (const character of blockId) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return `ant-native-${(hash >>> 0).toString(36)}`;
}
