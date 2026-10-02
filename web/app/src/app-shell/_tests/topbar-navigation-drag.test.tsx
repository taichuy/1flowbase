import {
  createEvent,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { App } from 'antd';
import { beforeEach, expect, test, vi } from 'vitest';
import type { FrontstagePageTreeNode } from '../../features/frontstage/api/page-tree';
import { TopbarNavigationItemLabel } from '../TopbarNavigationDesigner';

const mutations = vi.hoisted(() => ({
  isPending: false,
  moveNode: vi.fn().mockResolvedValue(undefined)
}));
vi.mock(
  '../../features/frontstage/hooks/use-frontstage-page-tree-mutations',
  () => ({
    useFrontstagePageTreeMutations: () => mutations
  })
);

const nodes = ['报表', '应用'].map<FrontstagePageTreeNode>((title, index) => ({
  id: `node-${index}`,
  title,
  kind: 'page',
  placement: 'topbar',
  content_presentation: 'single',
  children: []
}));

function renderLabels() {
  render(
    <App>
      {nodes.map((node) => (
        <TopbarNavigationItemLabel
          key={node.id}
          workspaceId="workspace-1"
          node={node}
          siblings={nodes}
        >
          <a href={`/${node.id}`}>{node.title}</a>
        </TopbarNavigationItemLabel>
      ))}
    </App>
  );
}

function transfer() {
  const data = new Map<string, string>();
  return {
    types: [] as string[],
    effectAllowed: '',
    dropEffect: '',
    setData(type: string, value: string) {
      data.set(type, value);
      this.types.push(type);
    },
    getData(type: string) {
      return data.get(type) ?? '';
    }
  };
}

beforeEach(() => {
  mutations.moveNode.mockClear();
  mutations.isPending = false;
});

test('clicking the drag handle opens no ordering menu or mutation', () => {
  renderLabels();
  const handle = screen.getByRole('button', { name: '拖拽排序报表' });
  expect(handle).toHaveAttribute('draggable', 'true');
  fireEvent.click(handle);
  expect(screen.queryByText('上移')).not.toBeInTheDocument();
  expect(screen.queryByText('下移')).not.toBeInTheDocument();
  expect(mutations.moveNode).not.toHaveBeenCalled();
  expect(screen.getByRole('button', { name: '配置报表' })).toBeInTheDocument();
});

test.each([
  [0, 1, 100, '002500'],
  [1, 0, -1, '000000']
])(
  'dragging item %s to item %s persists its insertion position',
  async (source, target, clientX, rank) => {
    renderLabels();
    const dataTransfer = transfer();
    fireEvent.dragStart(
      screen.getByRole('button', { name: `拖拽排序${nodes[source].title}` }),
      { dataTransfer }
    );
    const label = screen
      .getByRole('link', { name: nodes[target].title! })
      .closest('.app-shell-dynamic-nav-item')!;
    fireEvent.dragOver(label, { dataTransfer });
    expect(dataTransfer.dropEffect).toBe('move');
    const dropEvent = createEvent.drop(label, { dataTransfer });
    Object.defineProperty(dropEvent, 'clientX', { value: clientX });
    fireEvent(label, dropEvent);
    await waitFor(() =>
      expect(mutations.moveNode).toHaveBeenCalledWith(nodes[source].id, {
        parentId: null,
        rank
      })
    );
  }
);

test('self drops and unrelated drag payloads do not reorder navigation', () => {
  renderLabels();
  const dataTransfer = transfer();
  const handle = screen.getByRole('button', { name: '拖拽排序报表' });
  const label = handle.closest('.app-shell-dynamic-nav-item')!;
  fireEvent.dragStart(handle, { dataTransfer });
  fireEvent.drop(label, { dataTransfer });
  fireEvent.drop(label, { dataTransfer: transfer() });
  expect(mutations.moveNode).not.toHaveBeenCalled();
});

test('pending mutations disable the drag handle', () => {
  mutations.isPending = true;
  renderLabels();
  expect(screen.getByRole('button', { name: '拖拽排序报表' })).toBeDisabled();
  expect(screen.getByRole('button', { name: '拖拽排序报表' })).toHaveAttribute(
    'draggable',
    'false'
  );
});
