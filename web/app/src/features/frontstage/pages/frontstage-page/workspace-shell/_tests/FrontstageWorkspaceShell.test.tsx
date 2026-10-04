import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import { expect, test, vi } from 'vitest';

import {
  FrontstageWorkspaceShell,
  useFrontstageWorkspace
} from '../FrontstageWorkspaceShell';

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
