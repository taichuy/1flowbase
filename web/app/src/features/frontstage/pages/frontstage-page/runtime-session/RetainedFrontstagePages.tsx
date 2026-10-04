import {
  cloneElement,
  useLayoutEffect,
  useReducer,
  useRef,
  type ReactElement
} from 'react';
import type { FrontStagePageProps } from '../page-props';
import type { FrontStageTreeNode } from '../../../lib/page-tree';
import { FrontstageRuntimeActivityContext } from '../../../lib/page-canvas/runtime-activity';

// Retain native DOM/state for a bounded idle interval, without a page-count limit.
export const FRONTSTAGE_SESSION_IDLE_MS = 30 * 60 * 1000;

type NavigationProps = Pick<
  FrontStagePageProps,
  'onNavigatePage' | 'onNavigateTab' | 'onNavigateBlock'
>;
type Entry = {
  element: ReactElement<FrontStagePageProps>;
  inactiveSince: number | null;
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
    inactiveSince: null,
    runtimeActive: false,
    navigationAllowed: false,
    guards: {}
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
  );
}

/** Kept inside the workspace/session key boundary: disposal unmounts native roots. */
export function RetainedFrontstagePages({
  activeKey,
  pageTree,
  children
}: {
  activeKey: string;
  pageTree?: FrontStageTreeNode[];
  children: ReactElement<FrontStagePageProps>;
}) {
  const entries = useRef(new Map<string, Entry>());
  const fallback = useRef<{ key: string; entry: Entry } | null>(null);
  const activeKeyRef = useRef(activeKey);
  activeKeyRef.current = activeKey;
  const mounted = useRef(true);
  const [, wakeForExpiry] = useReducer((value: number) => value + 1, 0);
  const now = Date.now();
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
      (denied && entryPageId === props.pageId) ||
      (entry.inactiveSince !== null &&
        now - entry.inactiveSince >= FRONTSTAGE_SESSION_IDLE_MS)
    ) {
      entries.current.delete(key);
      continue;
    }
    entry.runtimeActive = key === activeKey && ready;
    entry.navigationAllowed = entry.runtimeActive;
    if (!entry.runtimeActive && entry.inactiveSince === null)
      entry.inactiveSince = now;
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
    entry.element = children;
    entry.inactiveSince = null;
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
    const deadlines = [...entries.current.values()].flatMap((entry) =>
      entry.inactiveSince === null
        ? []
        : [entry.inactiveSince + FRONTSTAGE_SESSION_IDLE_MS]
    );
    if (!deadlines.length) return;
    const timer = setTimeout(
      wakeForExpiry,
      Math.max(1, Math.min(...deadlines) - Date.now())
    );
    return () => clearTimeout(timer);
  });

  return (
    <>
      {[...entries.current].map(([key, entry]) => {
        const visible = key === activeKey && ready;
        return (
          <div
            key={key}
            hidden={!visible}
            style={{ display: visible ? 'contents' : 'none' }}
          >
            {renderEntry(entry)}
          </div>
        );
      })}
      {fallback.current ? (
        <div key={`fallback:${activeKey}`} style={{ display: 'contents' }}>
          {renderEntry(fallback.current.entry)}
        </div>
      ) : null}
    </>
  );
}
