import {
  Anchor as AntdAnchor,
  type AnchorProps
} from 'antd';
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

import { useNativeBlockSurface } from './native-block-surface-context';

interface ScopedAnchorLink {
  internalHref: string;
  originalHref: string;
  targetOffset?: number;
}

type AnchorLinkItem = NonNullable<AnchorProps['items']>[number];

const AFFIX_ENTER_EPSILON = 0.5;
const AFFIX_EXIT_EPSILON = 2;
const affixOverflowLeases = new WeakMap<HTMLElement, AffixOverflowLease>();

interface AffixOverflowLease {
  count: number;
  overflowX: string;
  overflowY: string;
}

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
  const affixStickyRef = useRef<HTMLDivElement | null>(null);
  const affixedRef = useRef(false);
  activeHrefRef.current = activeHref;
  const scoped = useMemo(
    () => createScopedItems(items, instanceId),
    [instanceId, items]
  );
  const scrollOwner = getContainer?.() ?? surface?.scrollOwner;
  const affixEnabled = Boolean(affix);

  useLayoutEffect(() => {
    affixedRef.current = false;
    if (!surface || !affixEnabled) return;
    return acquireAffixOverflowLease(surface.targetRoot);
  }, [affixEnabled, surface]);

  const measureAffix = useCallback(() => {
    const sentinel = affixSentinelRef.current;
    const sticky = affixStickyRef.current;
    if (
      !scrollOwner ||
      !affix ||
      !sentinel ||
      !sticky
    ) {
      return;
    }
    const sentinelRect = sentinel.getBoundingClientRect();
    const stickyRect = sticky.getBoundingClientRect();
    const ownerRect =
      scrollOwner instanceof HTMLElement
        ? scrollOwner.getBoundingClientRect()
        : { top: 0, bottom: window.innerHeight };
    const offsetBottom =
      typeof affix === 'object' ? affix.offsetBottom : undefined;
    const desiredTop =
      offsetBottom === undefined
        ? ownerRect.top + (offsetTop ?? 0)
        : ownerRect.bottom - offsetBottom - stickyRect.height;
    const nextAffixed = resolveAffixed({
      affixed: affixedRef.current,
      desiredTop,
      normalTop: sentinelRect.top,
      placement: offsetBottom === undefined ? 'top' : 'bottom'
    });
    if (nextAffixed !== affixedRef.current) {
      affixedRef.current = nextAffixed;
      if (typeof affix === 'object') affix.onChange?.(nextAffixed);
    }
  }, [affix, offsetTop, scrollOwner]);

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
    if (!surface || !scrollOwner) return;
    const threshold = targetOffset ?? offsetTop ?? 0;
    let active: { href: string; top: number } | null = null;
    for (const link of scoped.links) {
      const target = resolveLocalTarget(surface.targetRoot, link.originalHref);
      if (!target) continue;
      const linkThreshold = link.targetOffset ?? threshold;
      const top = getTargetOffsetTop(target, scrollOwner);
      if (top <= linkThreshold + bounds && (!active || top > active.top)) {
        active = { href: link.originalHref, top };
      }
    }
    publishActiveHref(active?.href ?? '');
  }, [bounds, offsetTop, publishActiveHref, scoped.links, scrollOwner, surface, targetOffset]);

  useEffect(() => {
    if (!surface || !scrollOwner) return;
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
      const target = resolveLocalTarget(surface.targetRoot, link.originalHref);
      if (target) observer?.observe(target);
    }
    observer?.observe(surface.targetRoot.host);
    return () => {
      if (frame) window.cancelAnimationFrame(frame);
      observer?.disconnect();
      scrollOwner.removeEventListener('scroll', scheduleMeasure);
      window.removeEventListener('resize', scheduleMeasure);
    };
  }, [measureActiveHref, measureAffix, scoped.links, scrollOwner, surface]);

  if (!surface || !scrollOwner) {
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
        showInkInFixed={showInkInFixed}
        style={style}
      />
    );
  }

  const internalActiveHref =
    scoped.originalToInternal.get(activeHref) ?? '';
  const scopedItems = items === undefined ? {} : { items: scoped.items };
  const handleClick = (
    event: MouseEvent<HTMLElement>,
    link: { title: ReactNode; href: string }
  ) => {
    const originalHref = scoped.internalToOriginal.get(link.href) ?? link.href;
    onClick?.(event, { ...link, href: originalHref });
    if (event.defaultPrevented || !isLocalHref(originalHref)) return;
    event.preventDefault();
    const target = resolveLocalTarget(surface.targetRoot, originalHref);
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
      getCurrentAnchor={() => internalActiveHref}
      onClick={handleClick}
      offsetTop={offsetTop}
      targetOffset={targetOffset}
      bounds={bounds}
      affix={false}
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
        data-flowbase-native-anchor-affix=""
        style={{
          display: 'flow-root',
          position: 'sticky',
          width: '100%',
          ...(typeof affix === 'object' && affix.offsetBottom !== undefined
            ? { bottom: affix.offsetBottom }
            : { top: offsetTop ?? 0 })
        }}
      >
        <div ref={affixStickyRef}>{anchor}</div>
      </div>
    </>
  ) : (
    anchor
  );
}

function resolveAffixed({
  affixed,
  desiredTop,
  normalTop,
  placement
}: {
  affixed: boolean;
  desiredTop: number;
  normalTop: number;
  placement: 'top' | 'bottom';
}): boolean {
  if (placement === 'top') {
    return affixed
      ? normalTop < desiredTop + AFFIX_EXIT_EPSILON
      : normalTop <= desiredTop - AFFIX_ENTER_EPSILON;
  }
  return affixed
    ? normalTop > desiredTop - AFFIX_EXIT_EPSILON
    : normalTop >= desiredTop + AFFIX_ENTER_EPSILON;
}

function acquireAffixOverflowLease(root: ShadowRoot): () => void {
  const mount = root.querySelector<HTMLElement>(
    '[data-flowbase-native-trusted-block-mount]'
  );
  if (!mount) {
    throw new Error('Native block Anchor requires an active portal mount.');
  }
  const activeLease = affixOverflowLeases.get(mount);
  if (activeLease) {
    activeLease.count += 1;
  } else {
    affixOverflowLeases.set(mount, {
      count: 1,
      overflowX: mount.style.overflowX,
      overflowY: mount.style.overflowY
    });
    // A horizontal-only overflow container computes to overflow-y:auto and
    // becomes the sticky scroll owner even when it has no vertical range.
    // Affixed blocks therefore use the real surface scroll owner on both axes.
    mount.style.overflowX = 'visible';
    mount.style.overflowY = 'visible';
  }
  return () => {
    const lease = affixOverflowLeases.get(mount);
    if (!lease) return;
    lease.count -= 1;
    if (lease.count > 0) return;
    mount.style.overflowX = lease.overflowX;
    mount.style.overflowY = lease.overflowY;
    affixOverflowLeases.delete(mount);
  };
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

function resolveLocalTarget(root: ShadowRoot, href: string): HTMLElement | null {
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
