import {
  ConfigProvider,
  Dropdown as AntdDropdown,
  type DropdownProps
} from 'antd';
import {
  cloneElement,
  isValidElement,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
  type Ref,
  type SyntheticEvent
} from 'react';

import { useNativeBlockSurface } from './native-block-surface-context';

export function NativeBlockDropdown({
  children,
  disabled,
  getPopupContainer,
  onOpenChange,
  open,
  trigger,
  ...props
}: DropdownProps): ReactNode {
  const surface = useNativeBlockSurface();
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false);
  const [overlayGeneration, setOverlayGeneration] = useState(0);
  const previousLayoutEpoch = useRef(surface?.layoutEpoch);
  const hoverTrigger = (trigger ?? ['hover']).includes('hover');
  const menuDismissed = useRef(false);
  const controlled = open !== undefined;
  const resolvedOpen = controlled ? open : uncontrolledOpen;
  const resolvedOpenRef = useRef(resolvedOpen);
  resolvedOpenRef.current = resolvedOpen;
  const overlayHost = surface?.overlayHost;
  const targetRoot = surface?.targetRoot;
  const usesNativeLayer = !!overlayHost && !getPopupContainer;

  useLayoutEffect(() => {
    const nextLayoutEpoch = surface?.layoutEpoch;
    if (previousLayoutEpoch.current === nextLayoutEpoch) return;
    previousLayoutEpoch.current = nextLayoutEpoch;
    if (!controlled) {
      resolvedOpenRef.current = false;
      setUncontrolledOpen(false);
    }
    setOverlayGeneration((generation) => generation + 1);
  }, [controlled, surface?.layoutEpoch]);

  const transitionOpen = useCallback(
    (nextOpen: boolean, info: { source: 'trigger' | 'menu' }) => {
      // A menu dismissal wins over a previously scheduled hover-open callback.
      // Only a new physical trigger intent may rearm hover opening.
      if (
        nextOpen &&
        info.source === 'trigger' &&
        hoverTrigger &&
        menuDismissed.current
      )
        return;
      if (!nextOpen && info.source === 'menu') menuDismissed.current = true;
      if (resolvedOpenRef.current === nextOpen) return;
      resolvedOpenRef.current = nextOpen;
      if (!controlled) setUncontrolledOpen(nextOpen);
      onOpenChange?.(nextOpen, info);
    },
    [controlled, hoverTrigger, onOpenChange]
  );
  const resolvePopupContainer = useCallback(
    (triggerNode?: HTMLElement) =>
      overlayHost?.container ??
      (triggerNode ? getPopupContainer?.(triggerNode) : undefined) ??
      (targetRoot?.host as HTMLElement | undefined) ??
      triggerNode?.ownerDocument.body ??
      targetRoot?.ownerDocument.body ??
      document.body,
    [getPopupContainer, overlayHost, targetRoot]
  );
  const rearmTriggerIntent = (event: SyntheticEvent<HTMLSpanElement>) => {
    if (event.currentTarget.contains(event.target as Node)) {
      menuDismissed.current = false;
    }
  };
  const rearmHoverIntent = (event: MouseEvent<HTMLSpanElement>) => {
    // Portal events bubble through React, but do not enter the trigger DOM.
    if (!event.currentTarget.contains(event.target as Node)) return;
    if (
      event.relatedTarget instanceof Node &&
      event.currentTarget.contains(event.relatedTarget)
    )
      return;
    rearmTriggerIntent(event);
  };
  const normalizedChildren = useViewportFixedVirtualTrigger({
    children,
    enabled: usesNativeLayer && resolvedOpen,
    layoutEpoch: surface?.layoutEpoch,
    scrollOwner: surface?.scrollOwner
  });
  const dropdown = (
    <AntdDropdown
      {...props}
      key={overlayGeneration}
      disabled={disabled}
      getPopupContainer={resolvePopupContainer}
      onOpenChange={transitionOpen}
      open={resolvedOpen}
      trigger={trigger}
    >
      {normalizedChildren}
    </AntdDropdown>
  );
  const popupScopedDropdown = usesNativeLayer ? (
    <ConfigProvider getPopupContainer={resolvePopupContainer}>
      {dropdown}
    </ConfigProvider>
  ) : (
    dropdown
  );
  return hoverTrigger && !disabled ? (
    <span
      data-flowbase-native-dropdown-intent=""
      style={{ display: 'contents' }}
      onKeyDownCapture={(event) => {
        if (event.key === 'Enter' || event.key === ' ') rearmTriggerIntent(event);
      }}
      onPointerDownCapture={rearmTriggerIntent}
      onContextMenuCapture={rearmTriggerIntent}
      onMouseDownCapture={rearmTriggerIntent}
      onMouseOverCapture={rearmHoverIntent}
      onPointerOverCapture={rearmHoverIntent}
    >
      {popupScopedDropdown}
    </span>
  ) : (
    popupScopedDropdown
  );
}

type VirtualTriggerProps = {
  'aria-hidden'?: boolean | 'true' | 'false';
  ref?: Ref<HTMLElement>;
  style?: CSSProperties;
};

type ViewportCorrection = {
  left: number;
  top: number;
};

const ZERO_VIEWPORT_CORRECTION: ViewportCorrection = { left: 0, top: 0 };

function useViewportFixedVirtualTrigger({
  children,
  enabled,
  layoutEpoch,
  scrollOwner
}: {
  children: ReactNode;
  enabled: boolean;
  layoutEpoch?: string;
  scrollOwner?: HTMLElement | Window;
}): ReactNode {
  const child = isValidElement<VirtualTriggerProps>(children)
    ? (children as ReactElement<VirtualTriggerProps>)
    : null;
  const authoredLeft = resolvePixelCoordinate(child?.props.style?.left);
  const authoredTop = resolvePixelCoordinate(child?.props.style?.top);
  const triggerNodeRef = useRef<HTMLElement | null>(null);
  const [correction, setCorrection] = useState<ViewportCorrection>(
    ZERO_VIEWPORT_CORRECTION
  );
  const childRef = child?.props.ref;
  const captureTriggerNode = useCallback(
    (node: HTMLElement | null) => {
      triggerNodeRef.current = node;
      assignRef(childRef, node);
    },
    [childRef]
  );

  useLayoutEffect(() => {
    const node = triggerNodeRef.current;
    const ownerWindow = node?.ownerDocument.defaultView;
    if (
      !enabled ||
      !node ||
      !ownerWindow ||
      authoredLeft === null ||
      authoredTop === null
    ) {
      setCorrection(ZERO_VIEWPORT_CORRECTION);
      return;
    }
    const computedStyle = ownerWindow.getComputedStyle(node);
    const isViewportVirtualTrigger =
      computedStyle.position === 'fixed' &&
      computedStyle.pointerEvents === 'none' &&
      node.getAttribute('aria-hidden') === 'true';
    if (!isViewportVirtualTrigger) {
      setCorrection(ZERO_VIEWPORT_CORRECTION);
      return;
    }

    let animationFrame = 0;
    const normalize = () => {
      animationFrame = 0;
      const rect = node.getBoundingClientRect();
      setCorrection((current) => {
        const next = {
          left: current.left + authoredLeft - rect.left,
          top: current.top + authoredTop - rect.top
        };
        return Math.abs(next.left - current.left) < 0.5 &&
          Math.abs(next.top - current.top) < 0.5
          ? current
          : next;
      });
    };
    const scheduleNormalization = () => {
      if (animationFrame) ownerWindow.cancelAnimationFrame(animationFrame);
      animationFrame = ownerWindow.requestAnimationFrame(normalize);
    };

    normalize();
    scrollOwner?.addEventListener('scroll', scheduleNormalization, {
      passive: true
    });
    ownerWindow.addEventListener('resize', scheduleNormalization, {
      passive: true
    });
    return () => {
      if (animationFrame) ownerWindow.cancelAnimationFrame(animationFrame);
      scrollOwner?.removeEventListener('scroll', scheduleNormalization);
      ownerWindow.removeEventListener('resize', scheduleNormalization);
    };
  }, [authoredLeft, authoredTop, enabled, layoutEpoch, scrollOwner]);

  if (!child || authoredLeft === null || authoredTop === null) return children;
  return cloneElement(child, {
    ref: captureTriggerNode,
    style: {
      ...child.props.style,
      left: authoredLeft + correction.left,
      top: authoredTop + correction.top
    }
  });
}

function resolvePixelCoordinate(value: CSSProperties['left']): number | null {
  if (typeof value === 'number') return Number.isFinite(value) ? value : null;
  if (typeof value !== 'string') return null;
  const match = /^(-?(?:\d+|\d*\.\d+))px$/.exec(value.trim());
  if (!match) return null;
  const coordinate = Number(match[1]);
  return Number.isFinite(coordinate) ? coordinate : null;
}

function assignRef(
  ref: Ref<HTMLElement> | undefined,
  node: HTMLElement | null
) {
  if (typeof ref === 'function') ref(node);
  else if (ref) ref.current = node;
}
