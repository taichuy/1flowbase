import { StyleProvider, createCache } from '@ant-design/cssinjs';
import { Anchor as AntdAnchor, type AnchorProps } from 'antd';
import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
  type ReactNode
} from 'react';
import { createPortal } from 'react-dom';

import {
  attachNativeAnchorAffixLayer,
  type NativeAnchorAffixLayer
} from './native-anchor-affix-layer';
import { useNativeBlockSurface } from './native-block-surface-context';

interface ScopedAnchorLink {
  internalHref: string;
  originalHref: string;
  targetOffset?: number;
}

type AnchorLinkItem = NonNullable<AnchorProps['items']>[number];

function NativeBlockAnchorComponent({
  items,
  getContainer,
  getCurrentAnchor,
  onChange,
  onClick,
  offsetTop,
  targetOffset,
  bounds = 5,
  affix = true,
  direction = 'vertical',
  showInkInFixed,
  style,
  ...props
}: AnchorProps) {
  const surface = useNativeBlockSurface();
  const instanceId = useId().replace(/[^a-zA-Z0-9]/gu, '');
  const [activeHref, setActiveHref] = useState('');
  const activeHrefRef = useRef(activeHref);
  const detectedHrefRef = useRef('');
  const affixSentinelRef = useRef<HTMLSpanElement | null>(null);
  const affixPlaceholderRef = useRef<HTMLDivElement | null>(null);
  const affixOptionsRef = useRef({
    offset: 0,
    placement: 'top' as 'top' | 'bottom'
  });
  const affixOnChangeRef = useRef<
    ((affixed?: boolean) => void) | undefined
  >(undefined);
  const [affixLayer, setAffixLayer] = useState<NativeAnchorAffixLayer | null>(
    null
  );
  const affixStyleCache = useMemo(() => createCache(), []);
  activeHrefRef.current = activeHref;
  const scoped = useMemo(
    () => createScopedItems(items, instanceId),
    [instanceId, items]
  );
  const targetRoot = surface?.targetRoot;
  const scrollOwner = getContainer?.() ?? surface?.scrollOwner;
  const affixEnabled = Boolean(affix);
  const offsetBottom =
    typeof affix === 'object' ? affix.offsetBottom : undefined;
  affixOptionsRef.current = {
    offset: offsetBottom ?? offsetTop ?? 0,
    placement: offsetBottom === undefined ? 'top' : 'bottom'
  };
  affixOnChangeRef.current =
    typeof affix === 'object' ? affix.onChange : undefined;
  const internalActiveHref = scoped.originalToInternal.get(activeHref) ?? '';
  const getInternalCurrentAnchor = useCallback(
    () => internalActiveHref,
    [internalActiveHref]
  );

  useLayoutEffect(() => {
    const placeholder = affixPlaceholderRef.current;
    const sentinel = affixSentinelRef.current;
    if (
      !targetRoot ||
      !scrollOwner ||
      !affixEnabled ||
      !placeholder ||
      !sentinel
    ) {
      return;
    }
    const blockId =
      targetRoot.host.getAttribute('data-flowbase-native-trusted-block-id') ??
      instanceId;
    const layer = attachNativeAnchorAffixLayer({
      blockId,
      onPinnedChange: (pinned) => affixOnChangeRef.current?.(pinned),
      options: () => affixOptionsRef.current,
      placeholder,
      scrollOwner,
      sentinel
    });
    setAffixLayer(layer);
    return () => layer.dispose();
  }, [affixEnabled, instanceId, scrollOwner, targetRoot]);

  useLayoutEffect(() => {
    const placeholder = affixPlaceholderRef.current;
    if (!affixLayer || !placeholder) return;
    const sync = () => {
      const height = affixLayer.mountElement.getBoundingClientRect().height;
      if (height > 0) placeholder.style.height = `${height}px`;
      affixLayer.refresh();
    };
    sync();
    const observer =
      typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(sync);
    observer?.observe(affixLayer.mountElement);
    window.addEventListener('resize', sync);
    return () => {
      observer?.disconnect();
      window.removeEventListener('resize', sync);
    };
  }, [affixLayer]);

  const measureAffix = useCallback(() => {
    affixLayer?.refresh();
  }, [affixLayer]);

  const publishActiveHref = useCallback(
    (nextHref: string) => {
      if (nextHref === detectedHrefRef.current) return;
      detectedHrefRef.current = nextHref;
      const customized = getCurrentAnchor
        ? getCurrentAnchor(nextHref)
        : nextHref;
      if (customized !== activeHrefRef.current) {
        activeHrefRef.current = customized;
        setActiveHref(customized);
      }
      onChange?.(nextHref);
    },
    [getCurrentAnchor, onChange]
  );

  const measureActiveHref = useCallback(() => {
    if (!targetRoot || !scrollOwner) return;
    const threshold = targetOffset ?? offsetTop ?? 0;
    let active: { href: string; top: number } | null = null;
    for (const link of scoped.links) {
      const target = resolveLocalTarget(targetRoot, link.originalHref);
      if (!target) continue;
      const linkThreshold = link.targetOffset ?? threshold;
      const top = getTargetOffsetTop(target, scrollOwner);
      if (top <= linkThreshold + bounds && (!active || top > active.top)) {
        active = { href: link.originalHref, top };
      }
    }
    publishActiveHref(active?.href ?? '');
  }, [
    bounds,
    offsetTop,
    publishActiveHref,
    scoped.links,
    scrollOwner,
    targetRoot,
    targetOffset
  ]);

  useEffect(() => {
    if (!targetRoot || !scrollOwner) return;
    let frame = 0;
    const scheduleMeasure = () => {
      if (frame) return;
      frame = window.requestAnimationFrame(() => {
        frame = 0;
        measureActiveHref();
        measureAffix();
      });
    };
    measureActiveHref();
    measureAffix();
    scrollOwner.addEventListener('scroll', scheduleMeasure, { passive: true });
    window.addEventListener('resize', scheduleMeasure);
    const observer =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(scheduleMeasure);
    for (const link of scoped.links) {
      const target = resolveLocalTarget(targetRoot, link.originalHref);
      if (target) observer?.observe(target);
    }
    observer?.observe(targetRoot.host);
    return () => {
      if (frame) window.cancelAnimationFrame(frame);
      observer?.disconnect();
      scrollOwner.removeEventListener('scroll', scheduleMeasure);
      window.removeEventListener('resize', scheduleMeasure);
    };
  }, [measureActiveHref, measureAffix, scoped.links, scrollOwner, targetRoot]);

  if (!targetRoot || !scrollOwner) {
    const fallbackItems = items === undefined ? {} : { items };
    return (
      <AntdAnchor
        {...props}
        {...fallbackItems}
        getContainer={getContainer}
        getCurrentAnchor={getCurrentAnchor}
        onChange={onChange}
        onClick={onClick}
        offsetTop={offsetTop}
        targetOffset={targetOffset}
        bounds={bounds}
        affix={affix}
        direction={direction}
        showInkInFixed={showInkInFixed}
        style={style}
      />
    );
  }

  const scopedItems = items === undefined ? {} : { items: scoped.items };
  const handleClick = (
    event: MouseEvent<HTMLElement>,
    link: { title: ReactNode; href: string }
  ) => {
    const originalHref = scoped.internalToOriginal.get(link.href) ?? link.href;
    onClick?.(event, { ...link, href: originalHref });
    if (event.defaultPrevented || !isLocalHref(originalHref)) return;
    event.preventDefault();
    const target = resolveLocalTarget(targetRoot, originalHref);
    if (!target) return;
    const localOffset = scoped.targetOffsets.get(originalHref);
    scrollTargetIntoOwner(
      target,
      scrollOwner,
      localOffset ?? targetOffset ?? offsetTop ?? 0
    );
    publishActiveHref(originalHref);
  };

  const anchor = (
    <AntdAnchor
      {...props}
      {...scopedItems}
      getContainer={() => scrollOwner}
      getCurrentAnchor={getInternalCurrentAnchor}
      onClick={handleClick}
      offsetTop={offsetTop}
      targetOffset={targetOffset}
      bounds={bounds}
      affix={false}
      direction={direction}
      showInkInFixed={affix ? true : showInkInFixed}
      style={style}
    />
  );
  return affix ? (
    <>
      <span
        ref={affixSentinelRef}
        data-flowbase-native-anchor-affix-sentinel=""
        aria-hidden="true"
        style={{ display: 'block', height: 0, pointerEvents: 'none' }}
      />
      <div
        ref={affixPlaceholderRef}
        data-flowbase-native-anchor-affix=""
        style={{
          display: 'flow-root',
          width: '100%'
        }}
      />
      {affixLayer
        ? createPortal(
            <StyleProvider
              cache={affixStyleCache}
              container={affixLayer.shadowRoot}
            >
              {anchor}
            </StyleProvider>,
            affixLayer.mountElement
          )
        : null}
    </>
  ) : (
    anchor
  );
}

export const NativeBlockAnchor = Object.assign(NativeBlockAnchorComponent, {
  Link: AntdAnchor.Link
}) as typeof AntdAnchor;

function createScopedItems(
  items: AnchorLinkItem[] | undefined,
  instanceId: string
): {
  items: AnchorLinkItem[] | undefined;
  links: ScopedAnchorLink[];
  internalToOriginal: Map<string, string>;
  originalToInternal: Map<string, string>;
  targetOffsets: Map<string, number>;
} {
  const links: ScopedAnchorLink[] = [];
  const internalToOriginal = new Map<string, string>();
  const originalToInternal = new Map<string, string>();
  const targetOffsets = new Map<string, number>();
  let sequence = 0;
  const visit = (entries: AnchorLinkItem[]): AnchorLinkItem[] =>
    entries.map((entry) => {
      if (!isLocalHref(entry.href)) {
        return {
          ...entry,
          children: entry.children ? visit(entry.children) : undefined
        };
      }
      sequence += 1;
      const internalHref = `#nativeanchor-${instanceId}-${sequence}`;
      links.push({
        internalHref,
        originalHref: entry.href,
        targetOffset: entry.targetOffset
      });
      internalToOriginal.set(internalHref, entry.href);
      originalToInternal.set(entry.href, internalHref);
      if (entry.targetOffset !== undefined) {
        targetOffsets.set(entry.href, entry.targetOffset);
      }
      return {
        ...entry,
        href: internalHref,
        children: entry.children ? visit(entry.children) : undefined
      };
    });
  return {
    items: items ? visit(items) : undefined,
    links,
    internalToOriginal,
    originalToInternal,
    targetOffsets
  };
}

function isLocalHref(href: string): boolean {
  return href.startsWith('#') && href.length > 1;
}

function resolveLocalTarget(
  root: ShadowRoot,
  href: string
): HTMLElement | null {
  if (!isLocalHref(href)) return null;
  const id = decodeURIComponent(href.slice(1));
  return root.getElementById(id) as HTMLElement | null;
}

function getTargetOffsetTop(
  target: HTMLElement,
  scrollOwner: HTMLElement | Window
): number {
  const targetRect = target.getBoundingClientRect();
  if (!(scrollOwner instanceof HTMLElement)) return targetRect.top;
  return targetRect.top - scrollOwner.getBoundingClientRect().top;
}

function scrollTargetIntoOwner(
  target: HTMLElement,
  scrollOwner: HTMLElement | Window,
  offset: number
): void {
  const relativeTop = getTargetOffsetTop(target, scrollOwner);
  const currentTop =
    scrollOwner instanceof HTMLElement ? scrollOwner.scrollTop : window.scrollY;
  const top = currentTop + relativeTop - offset;
  scrollOwner.scrollTo({ top, behavior: 'auto' });
}
