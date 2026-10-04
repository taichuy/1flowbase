import {
  cloneElement,
  useLayoutEffect,
  useEffect,
  useReducer,
  useRef,
  type ReactElement
} from 'react';
import type { FrontStagePageProps } from '../page-props';
import type { FrontStageTreeNode } from '../../../lib/page-tree';
import { FrontstageRuntimeActivityContext } from '../../../lib/page-canvas/runtime-activity';

import { RetentionPolicy } from './retention-policy';
import { RetentionStats } from './retention-stats';
import {
  browserRetentionStorage,
  readHeapSample,
  readRetentionBudget,
  retainedMarkupCost,
  serializedCost,
  RetentionPressure,
  RETENTION_SAMPLE_MS
} from './retention-memory';
import { FrontstageRetentionProtectionContext } from './retention-protection';

type NavigationProps = Pick<
  FrontStagePageProps,
  'onNavigatePage' | 'onNavigateTab' | 'onNavigateBlock'
>;
type Entry = {
  element: ReactElement<FrontStagePageProps>;
  wrapper: HTMLDivElement | null;
  cost: number | null;
  protectedFromEviction: boolean;
  setProtection: (value: boolean) => void;
  setWrapper: (node: HTMLDivElement | null) => void;
  runtimeActive: boolean;
  navigationAllowed: boolean;
  guards: NavigationProps;
};

function createEntry(
  element: ReactElement<FrontStagePageProps>,
  isCurrent: (entry: Entry) => boolean
): Entry {
  const entry: Entry = {
    element,
    wrapper: null,
    cost: null,
    protectedFromEviction: false,
    setProtection: () => {},
    setWrapper: () => {},
    runtimeActive: false,
    navigationAllowed: false,
    guards: {}
  };
  entry.setProtection = (value) => {
    entry.protectedFromEviction = value;
    entry.cost = null;
  };
  entry.setWrapper = (node) => {
    entry.wrapper = node;
    entry.cost = null;
  };
  // Read the latest callback through the entry, keeping native effect props stable.
  entry.guards = {
    onNavigatePage: (...args) => {
      if (entry.navigationAllowed && isCurrent(entry))
        entry.element.props.onNavigatePage?.(...args);
    },
    onNavigateTab: (...args) => {
      if (entry.navigationAllowed && isCurrent(entry))
        entry.element.props.onNavigateTab?.(...args);
    },
    onNavigateBlock: (...args) => {
      if (entry.navigationAllowed && isCurrent(entry))
        entry.element.props.onNavigateBlock?.(...args);
    }
  };
  return entry;
}

function collectPageIds(nodes: FrontStageTreeNode[], ids = new Set<string>()) {
  for (const node of nodes) {
    if (node.kind === 'page') ids.add(node.id);
    if (node.kind === 'group') collectPageIds(node.children ?? [], ids);
  }
  return ids;
}

function renderEntry(entry: Entry) {
  return (
    <FrontstageRetentionProtectionContext.Provider value={entry.setProtection}>
      <FrontstageRuntimeActivityContext.Provider value={entry.runtimeActive}>
        {cloneElement(entry.element, {
          runtimeActive: entry.runtimeActive,
          onNavigatePage: entry.element.props.onNavigatePage
            ? entry.guards.onNavigatePage
            : undefined,
          onNavigateTab: entry.element.props.onNavigateTab
            ? entry.guards.onNavigateTab
            : undefined,
          onNavigateBlock: entry.element.props.onNavigateBlock
            ? entry.guards.onNavigateBlock
            : undefined
        })}
      </FrontstageRuntimeActivityContext.Provider>
    </FrontstageRetentionProtectionContext.Provider>
  );
}

/** Kept inside the workspace/session key boundary: disposal unmounts native roots. */
export function RetainedFrontstagePages({
  activeKey,
  pageTree,
  statisticsScope,
  retentionBudgetBytes,
  children
}: {
  activeKey: string;
  pageTree?: FrontStageTreeNode[];
  /** Actor/workspace identity only. Never include CSRF/session tokens or inputs. */
  statisticsScope?: string;
  /** Optional explicit accounting budget; omitted uses this browser's settings. */
  retentionBudgetBytes?: number;
  children: ReactElement<FrontStagePageProps>;
}) {
  const entries = useRef(new Map<string, Entry>());
  const fallback = useRef<{ key: string; entry: Entry } | null>(null);
  const activeKeyRef = useRef(activeKey);
  activeKeyRef.current = activeKey;
  const mounted = useRef(true);
  const [, renderAfterEviction] = useReducer((value: number) => value + 1, 0);
  const cache = useRef<{
    stats: RetentionStats;
    policy: RetentionPolicy;
    pressure: RetentionPressure;
  } | null>(null);
  if (!cache.current) {
    const stats = new RetentionStats({
      scope: statisticsScope ?? '',
      storage: statisticsScope ? browserRetentionStorage() : null
    });
    cache.current = {
      stats,
      policy: new RetentionPolicy(stats),
      pressure: new RetentionPressure()
    };
  }
  const activation = useRef({ key: activeKey, counted: false });
  const props = children.props;
  // A failed tree query is not evidence that a page was deleted.
  const allowedPages =
    pageTree && !props.hasPageTreeLoadError ? collectPageIds(pageTree) : null;
  const denied = Boolean(
    props.isPageContentPermissionDenied || props.isBlockRuntimePermissionDenied
  );
  const failed = Boolean(
    denied ||
    props.hasPageTreeLoadError ||
    props.hasPageContentLoadError ||
    props.hasBlockRootsLoadError ||
    props.hasBlockRuntimeLoadError
  );
  const ready =
    !failed &&
    Boolean(
      props.pageId &&
      (!allowedPages || allowedPages.has(props.pageId)) &&
      props.pageContent?.page.id === props.pageId &&
      props.pageContent?.tab.id === props.tabId
    );
  for (const [key, entry] of entries.current) {
    const entryPageId = entry.element.props.pageId;
    if (
      (allowedPages && entryPageId && !allowedPages.has(entryPageId)) ||
      (denied && entryPageId === props.pageId)
    ) {
      entries.current.delete(key);
      cache.current.policy.remove(key);
      continue;
    }
    const nextActive = key === activeKey && ready;
    if (entry.runtimeActive !== nextActive) entry.cost = null;
    entry.runtimeActive = nextActive;
    entry.navigationAllowed = entry.runtimeActive;
  }
  // Never replace a hot snapshot with a failure or content from another route.
  if (ready) {
    let entry = entries.current.get(activeKey);
    if (!entry) {
      entry = createEntry(
        children,
        (candidate) =>
          mounted.current &&
          activeKeyRef.current === activeKey &&
          entries.current.get(activeKey) === candidate
      );
      entries.current.set(activeKey, entry);
    }
    if (entry.element !== children) entry.cost = null;
    entry.element = children;
    entry.runtimeActive = true;
    entry.navigationAllowed = true;
  }
  const activeEntry = entries.current.get(activeKey);
  const needsFallback = !ready || !activeEntry;
  if (needsFallback) {
    if (fallback.current?.key !== activeKey) {
      fallback.current = {
        key: activeKey,
        entry: createEntry(
          children,
          (candidate) =>
            mounted.current &&
            activeKeyRef.current === activeKey &&
            fallback.current?.entry === candidate
        )
      };
    }
    fallback.current.entry.element = children;
    fallback.current.entry.runtimeActive = ready;
    fallback.current.entry.navigationAllowed = true;
  } else {
    fallback.current = null;
  }
  useLayoutEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useLayoutEffect(() => {
    if (activation.current.key !== activeKey)
      activation.current = { key: activeKey, counted: false };
    if (ready && !activation.current.counted) {
      activation.current.counted = true;
      cache.current!.policy.activate(activeKey);
    }
  }, [activeKey, ready]);

  useEffect(() => {
    const owner = cache.current!;
    let idle: number | undefined;
    let timer: ReturnType<typeof setTimeout>;
    let cancelled = false;
    const sweep = () => {
      if (cancelled) return;
      const candidates = [...entries.current].flatMap(([key, entry]) => {
        // The current route is protected even during transient revalidation errors.
        if (key === activeKeyRef.current) return [];
        // Native effects are suspended while hidden. Account once after each
        // activation/content/write transition, not by serializing every hidden
        // DOM tree on every heap-pressure sample.
        if (entry.cost === null) {
          const { pageContent, blockRoots, blockRuntimeAssembly } =
            entry.element.props;
          entry.cost = Math.max(
            1,
            serializedCost({ pageContent, blockRoots, blockRuntimeAssembly }) +
              retainedMarkupCost(entry.wrapper)
          );
        }
        return [
          { key, protected: entry.protectedFromEviction, cost: entry.cost }
        ];
      });
      const heap = readHeapSample();
      const configured =
        typeof retentionBudgetBytes === 'number' &&
        Number.isFinite(retentionBudgetBytes) &&
        retentionBudgetBytes >= 0
          ? retentionBudgetBytes
          : readRetentionBudget(undefined, heap);
      const budget = owner.pressure.budget(
        heap,
        configured,
        candidates.reduce((sum, item) => sum + item.cost, 0)
      );
      const victims = owner.policy.reconcile(candidates, budget);
      for (const key of victims) entries.current.delete(key);
      owner.stats.flush();
      if (victims.length) renderAfterEviction();
      timer = setTimeout(schedule, RETENTION_SAMPLE_MS);
    };
    const schedule = () => {
      if (typeof window.requestIdleCallback === 'function') {
        idle = window.requestIdleCallback(sweep, { timeout: 1000 });
      } else timer = setTimeout(sweep, 0);
    };
    const flush = () => owner.stats.flush();
    schedule();
    window.addEventListener('pagehide', flush);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      if (idle !== undefined) window.cancelIdleCallback(idle);
      window.removeEventListener('pagehide', flush);
      flush();
    };
  }, [activeKey, ready, retentionBudgetBytes]);

  return (
    <>
      {[...entries.current].map(([key, entry]) => {
        const visible = key === activeKey && ready;
        return (
          <div
            key={key}
            ref={entry.setWrapper}
            data-frontstage-retained-page={key}
            hidden={!visible}
            style={{ display: visible ? 'block' : 'none' }}
          >
            {renderEntry(entry)}
          </div>
        );
      })}
      {fallback.current ? (
        <div key={`fallback:${activeKey}`} style={{ display: 'block' }}>
          {renderEntry(fallback.current.entry)}
        </div>
      ) : null}
    </>
  );
}
