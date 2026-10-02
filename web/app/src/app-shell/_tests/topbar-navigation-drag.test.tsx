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
import { useTopbarDragStore } from '../../features/frontstage/lib/navigation-drag/topbar-drag-store';
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
    setDragImage: vi.fn(),
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
  useTopbarDragStore.setState({ drag: null });
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
  [0, 1, 100, 'after_id'],
  [1, 0, -1, 'before_id']
])(
  'dragging item %s to item %s persists its insertion position',
  async (source, target, clientX, positionField) => {
    renderLabels();
    const dataTransfer = transfer();
    fireEvent.dragStart(
      screen.getByRole('button', { name: `拖拽排序${nodes[source].title}` }),
      { dataTransfer }
    );
    const label = screen
      .getByRole('link', { name: nodes[target].title! })
      .closest('.app-shell-dynamic-nav-item')!;
    vi.spyOn(label, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      width: 100
    } as DOMRect);
    const overEvent = createEvent.dragOver(label, { dataTransfer });
    Object.defineProperty(overEvent, 'clientX', { value: clientX });
    fireEvent(label, overEvent);
    expect(screen.getByRole('status')).toHaveTextContent(
      `${nodes[source].title} → ${nodes[target].title}`
    );
    expect(dataTransfer.dropEffect).toBe('move');
    const dropEvent = createEvent.drop(label, { dataTransfer });
    // Drop coordinates deliberately disagree: the visible projection is authoritative.
    Object.defineProperty(dropEvent, 'clientX', {
      value: clientX < 0 ? 200 : -1
    });
    fireEvent(label, dropEvent);
    await waitFor(() =>
      expect(mutations.moveNode).toHaveBeenCalledWith(nodes[source].id, {
        parentId: null,
        [positionField]: nodes[target].id
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

test('leaving the target and ending a drag clear the projection without saving', () => {
  renderLabels();
  const dataTransfer = transfer();
  const handle = screen.getByRole('button', { name: '拖拽排序报表' });
  fireEvent.dragStart(handle, { dataTransfer });
  const label = screen
    .getByRole('link', { name: '应用' })
    .closest('.app-shell-dynamic-nav-item')!;
  vi.spyOn(label, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    width: 100
  } as DOMRect);
  const over = createEvent.dragOver(label, { dataTransfer });
  Object.defineProperty(over, 'clientX', { value: 90 });
  fireEvent(label, over);
  expect(screen.getByRole('status')).toBeInTheDocument();
  fireEvent.dragLeave(label);
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
  fireEvent.drop(label, { dataTransfer });
  fireEvent.dragEnd(handle, { dataTransfer });
  expect(mutations.moveNode).not.toHaveBeenCalled();
  expect(useTopbarDragStore.getState().drag).toBeNull();
});
