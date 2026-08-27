import {
  Anchor as AntdAnchor,
  type AnchorProps
} from 'antd';
import {
  useCallback,
  useEffect,
  useId,
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
type AffixPhase = 'flow' | 'pinned' | 'end-clamp';

const AFFIX_ENTER_EPSILON = 0.5;
const AFFIX_EXIT_EPSILON = 2;

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
  const affixShellRef = useRef<HTMLDivElement | null>(null);
  const affixStickyRef = useRef<HTMLDivElement | null>(null);
  const affixPhaseRef = useRef<AffixPhase>('flow');
  const affixFlowHeightRef = useRef<number | null>(null);
  const affixFlowOffsetRef = useRef(0);
  activeHrefRef.current = activeHref;
  const scoped = useMemo(
    () => createScopedItems(items, instanceId),
    [instanceId, items]
  );
  const scrollOwner = getContainer?.() ?? surface?.scrollOwner;

  const measureAffix = useCallback(() => {
    const shell = affixShellRef.current;
    const sticky = affixStickyRef.current;
    if (
      !surface ||
      !scrollOwner ||
      !affix ||
      !shell ||
      !sticky
    ) {
      return;
    }
    const shellRect = shell.getBoundingClientRect();
    const stickyRect = sticky.getBoundingClientRect();
    if (affixPhaseRef.current === 'flow') {
      affixFlowHeightRef.current = stickyRect.height;
      affixFlowOffsetRef.current = stickyRect.top - shellRect.top;
    }
    const affixHeight = affixFlowHeightRef.current ?? stickyRect.height;
    const normalTop = shellRect.top + affixFlowOffsetRef.current;
    shell.style.height = `${affixHeight}px`;
    const hostRect = surface.targetRoot.host.getBoundingClientRect();
    const ownerRect =
      scrollOwner instanceof HTMLElement
        ? scrollOwner.getBoundingClientRect()
        : { top: 0, bottom: window.innerHeight };
    const offsetBottom =
      typeof affix === 'object' ? affix.offsetBottom : undefined;
    const desiredTop =
      offsetBottom === undefined
        ? ownerRect.top + (offsetTop ?? 0)
        : ownerRect.bottom - offsetBottom - affixHeight;
    const nextPhase = resolveAffixPhase({
      current: affixPhaseRef.current,
      desiredTop,
      endTop:
        offsetBottom === undefined
          ? hostRect.bottom - affixHeight
          : hostRect.top,
      normalTop,
      placement: offsetBottom === undefined ? 'top' : 'bottom'
    });
    const nextTop =
      nextPhase === 'flow'
        ? normalTop
        : nextPhase === 'pinned'
          ? desiredTop
          : offsetBottom === undefined
            ? hostRect.bottom - affixHeight
            : hostRect.top;
    const nextAffixed = nextPhase !== 'flow';
    if (nextAffixed) {
      const containingBlock = resolveFixedContainingBlock(
        surface.targetRoot.host
      );
      const containingRect = containingBlock?.getBoundingClientRect() ?? {
        left: 0,
        top: 0
      };
      sticky.style.left = `${shellRect.left - containingRect.left}px`;
      sticky.style.position = 'fixed';
      sticky.style.top = `${nextTop - containingRect.top}px`;
      sticky.style.width = `${shellRect.width}px`;
    } else {
      sticky.style.left = '';
      sticky.style.position = '';
      sticky.style.top = '';
      sticky.style.width = '';
    }
    const wasAffixed = affixPhaseRef.current !== 'flow';
    affixPhaseRef.current = nextPhase;
    if (nextAffixed !== wasAffixed) {
      if (typeof affix === 'object') affix.onChange?.(nextAffixed);
    }
  }, [affix, offsetTop, scrollOwner, surface]);

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
    <div
      ref={affixShellRef}
      data-flowbase-native-anchor-affix=""
      style={{ width: '100%' }}
    >
      <div
        ref={affixStickyRef}
      >
        {anchor}
      </div>
    </div>
  ) : (
    anchor
  );
}

function resolveAffixPhase({
  current,
  desiredTop,
  endTop,
  normalTop,
  placement
}: {
  current: AffixPhase;
  desiredTop: number;
  endTop: number;
  normalTop: number;
  placement: 'top' | 'bottom';
}): AffixPhase {
  if (placement === 'top') {
    if (current === 'flow') {
      if (normalTop > desiredTop - AFFIX_ENTER_EPSILON) return 'flow';
      return endTop <= desiredTop ? 'end-clamp' : 'pinned';
    }
    if (normalTop >= desiredTop + AFFIX_EXIT_EPSILON) return 'flow';
    if (current === 'end-clamp') {
      return endTop >= desiredTop + AFFIX_EXIT_EPSILON
        ? 'pinned'
        : 'end-clamp';
    }
    return endTop <= desiredTop - AFFIX_ENTER_EPSILON
      ? 'end-clamp'
      : 'pinned';
  }

  if (current === 'flow') {
    if (normalTop < desiredTop + AFFIX_ENTER_EPSILON) return 'flow';
    return endTop >= desiredTop ? 'end-clamp' : 'pinned';
  }
  if (normalTop <= desiredTop - AFFIX_EXIT_EPSILON) return 'flow';
  if (current === 'end-clamp') {
    return endTop <= desiredTop - AFFIX_EXIT_EPSILON
      ? 'pinned'
      : 'end-clamp';
  }
  return endTop >= desiredTop + AFFIX_ENTER_EPSILON
    ? 'end-clamp'
    : 'pinned';
}

function resolveFixedContainingBlock(host: Element): HTMLElement | null {
  let candidate = host.parentElement;
  while (candidate) {
    const style = window.getComputedStyle(candidate);
    if (
      style.transform !== 'none' ||
      style.perspective !== 'none' ||
      style.filter !== 'none' ||
      style.willChange.split(',').some((value) => value.trim() === 'transform')
    ) {
      return candidate;
    }
    candidate = candidate.parentElement;
  }
  return null;
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
