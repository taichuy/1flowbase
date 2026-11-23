import { Suspense } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import { expect, test, vi } from 'vitest';

import { LoadingState } from '../../../../../../shared/ui/loading-state/LoadingState';
import { FrontstageWorkspaceShell } from '../FrontstageWorkspaceShell';
import { useFrontstageWorkspace } from '../workspace-context';

vi.mock('../../../../hooks/runtime-i18n/use-block-i18n', () => ({
  useBlockI18n: () => ({ t: (key: string) => key })
}));

vi.mock('../../../../components/FrontStagePageTreeSidebar', () => ({
  FrontStagePageTreeSidebar: ({
    selectedPageId
  }: {
    selectedPageId: string;
  }) => <nav data-testid="sidebar">{selectedPageId}</nav>
}));
vi.mock('../../page-tree-form-modal', () => ({
  PageTreeFormModal: () => <div data-testid="tree-form" />
}));
const tree = [
  { id: 'a', title: 'A', kind: 'page' as const },
  { id: 'b', title: 'B', kind: 'page' as const }
];
function Body({ pageId }: { pageId: string }) {
  const workspace = useFrontstageWorkspace(pageId);
  const [count, setCount] = useState(0);
  return (
    <button data-testid={pageId} onClick={() => setCount(count + 1)}>
      {workspace.selectedPageNode?.title}:{count}
    </button>
  );
}
function view(pageId: string, showSidebar = true) {
  return (
    <FrontstageWorkspaceShell
      workspaceId="w1"
      pageId={pageId}
      initialPageTree={tree}
      showSidebar={showSidebar}
      onNavigatePage={vi.fn()}
    >
      <div hidden={pageId !== 'a'}>
        <Body pageId="a" />
      </div>
      <div hidden={pageId !== 'b'}>
        <Body pageId="b" />
      </div>
    </FrontstageWorkspaceShell>
  );
}

test('shares one shell/form while retained bodies keep their own node and state', async () => {
  const utils = render(view('a'));
  const original = screen.getByTestId('a');
  fireEvent.click(original);
  utils.rerender(view('b'));
  await waitFor(() =>
    expect(screen.getByTestId('sidebar')).toHaveTextContent(/^b$/, { normalizeWhitespace: false })
  );
  expect(screen.getAllByTestId('section-page-layout')).toHaveLength(1);
  expect(screen.getAllByTestId('sidebar')).toHaveLength(1);
  expect(screen.getAllByTestId('tree-form')).toHaveLength(1);
  expect(original).toHaveTextContent(/^A:1$/, { normalizeWhitespace: false });
  expect(screen.getByTestId('b')).toHaveTextContent(/^B:0$/, { normalizeWhitespace: false });
  utils.rerender(view('a'));
  expect(screen.getByTestId('a')).toBe(original);
  expect(original).toHaveTextContent(/^A:1$/, { normalizeWhitespace: false });
});

test('sidebar availability never replaces the content branch or its state', () => {
  const utils = render(view('a'));
  const original = screen.getByTestId('a');
  fireEvent.click(original);
  utils.rerender(view('a', false));
  expect(screen.getByTestId('a')).toBe(original);
  expect(original).toHaveTextContent(/^A:1$/, { normalizeWhitespace: false });
  utils.rerender(view('a', true));
  expect(screen.getByTestId('a')).toBe(original);
});

test('progressive rendering: left sidebar stays visible and interactive while right-hand canvas suspends with non-fullscreen LoadingState', async () => {
  const neverResolves = new Promise<never>(() => undefined);
  function SuspendingCanvas() {
    throw neverResolves;
  }

  const { QueryClient, QueryClientProvider } = await import('@tanstack/react-query');
  const { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } =
    await import('@tanstack/react-router');
  vi.doMock('../../../FrontStagePage', () => ({
    FrontStagePage: SuspendingCanvas
  }));
  const { FrontstageWorkspacePage } = await import('../../../FrontstageWorkspacePage');
  const { useAuthStore } = await import('../../../../../../state/auth-store');

  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf',
    actor: {
      id: 'actor-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'w1'
    },
    me: null
  });

  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const rootNode = {
    id: 'g-route',
    title: 'Route',
    kind: 'group' as const,
    parent_id: null,
    rank: '001000',
    icon: null,
    tooltip: null,
    is_hidden: false,
    show_title: false,
    placement: 'topbar' as const,
    slug: 'route',
    content_presentation: 'single' as const,
    children: [
      {
        id: 'p-reports',
        title: 'Reports',
        kind: 'page' as const,
        parent_id: 'g-route',
        rank: '001000',
        icon: null,
        tooltip: null,
        is_hidden: false,
        show_title: false,
        slug: null,
        placement: 'sidebar' as const,
        content_presentation: 'single' as const,
        children: []
      }
    ]
  };
  const rootRoute = createRootRoute();
  const pageRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/route/pages/p-reports',
    component: () => (
      <Suspense fallback={<LoadingState />}>
        <FrontstageWorkspacePage
          workspaceId="w1"
          pageId="p-reports"
          rootNode={rootNode}
        />
      </Suspense>
    )
  });
  const router = createRouter({
    routeTree: rootRoute.addChildren([pageRoute]),
    history: createMemoryHistory({ initialEntries: ['/route/pages/p-reports'] })
  });

  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );

  // Left sidebar renders immediately without waiting for heavy FrontStagePage canvas
  expect(await screen.findByTestId('sidebar')).toHaveTextContent('p-reports');
  const statusNodes = screen.queryAllByRole('status');
  for (const statusNode of statusNodes) {
    expect(statusNode).not.toHaveClass('loading-state--fullscreen');
  }
});

test('marks pageContent loading true while pageTabsQuery is pending and exposes 1:1 WorkspaceShellSkeleton geometry', async () => {
  const { setSeededPublicSystemMetadata } = await import(
    '../../../../../../shared/api/system-metadata-seed'
  );
  const { WorkspaceShellSkeleton } = await import(
    '../../../../../../features/auth/components/anonymous-loading/WorkspaceShellSkeleton'
  );

  setSeededPublicSystemMetadata({
    system_name: '1flowbase',
    logo_url: '/icon.svg'
  });

  const view = render(<WorkspaceShellSkeleton mode="full" />);
  expect(screen.getByTestId('workspace-shell-skeleton')).toHaveTextContent(
    '1flowbase'
  );
  view.unmount();

  let capturedIsPageContentLoading: boolean | undefined;
  vi.resetModules();
  vi.doMock('../../../../api/page-tabs', async (importOriginal) => {
    const actual = await importOriginal<typeof import('../../../../api/page-tabs')>();
    return {
      ...actual,
      fetchFrontstagePageTabs: () => new Promise(() => undefined)
    };
  });
  vi.doMock('../../../FrontStagePage', () => ({
    FrontStagePage: (props: { isPageContentLoading?: boolean }) => {
      capturedIsPageContentLoading = props.isPageContentLoading;
      return <div data-testid="canvas-probe">{String(props.isPageContentLoading)}</div>;
    }
  }));

  const { QueryClient, QueryClientProvider } = await import('@tanstack/react-query');
  const { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } =
    await import('@tanstack/react-router');
  const { FrontstageWorkspacePage } = await import('../../../FrontstageWorkspacePage');
  const { useAuthStore } = await import('../../../../../../state/auth-store');

  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf',
    actor: {
      id: 'actor-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'w1'
    },
    me: null
  });

  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const rootNode = {
    id: 'g-route',
    title: 'Route',
    kind: 'group' as const,
    parent_id: null,
    rank: '001000',
    icon: null,
    tooltip: null,
    is_hidden: false,
    show_title: false,
    placement: 'topbar' as const,
    slug: 'route',
    content_presentation: 'single' as const,
    children: [
      {
        id: 'p-reports',
        title: 'Reports',
        kind: 'page' as const,
        parent_id: 'g-route',
        rank: '001000',
        icon: null,
        tooltip: null,
        is_hidden: false,
        show_title: false,
        slug: null,
        placement: 'sidebar' as const,
        content_presentation: 'single' as const,
        children: []
      }
    ]
  };
  const rootRoute = createRootRoute();
  const pageRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/route/pages/p-reports',
    component: () => (
      <FrontstageWorkspacePage
        workspaceId="w1"
        pageId="p-reports"
        rootNode={rootNode}
      />
    )
  });
  const router = createRouter({
    routeTree: rootRoute.addChildren([pageRoute]),
    history: createMemoryHistory({ initialEntries: ['/route/pages/p-reports'] })
  });

  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );

  await waitFor(() => {
    expect(screen.getByTestId('canvas-probe')).toBeInTheDocument();
  });
  expect(capturedIsPageContentLoading).toBe(true);
});
