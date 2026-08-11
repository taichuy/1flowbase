import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { JsxStudioChildContainersPanel } from '../../components/jsx-studio/JsxStudioChildContainersPanel';
import type { ChildContainerNode } from '../../lib/child-container-tree';
import type { FrontstageBlockInstance } from '../../lib/page-document';

function createBlock(
  id: string,
  title: string,
  childContainerTargetIds: string[] = []
): FrontstageBlockInstance {
  return {
    id,
    rendererVersion: 'v1',
    sourceId: id,
    codeRef: `${id}-code`,
    sourceCodeRef: `${id}-code`,
    catalog: { providerCode: null, installationId: null },
    contribution: { pluginId: null, pluginVersion: null, code: 'test' },
    props: { title },
    ports: { inputs: [], outputs: [] },
    childContainerTargetIds,
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0 },
    order: 0,
    runtime: { kind: 'native_react', entry: null, hint: 'native_react' }
  };
}

const ownerBlock = createBlock('launcher', 'Launcher');
const contentA = createBlock('content-a', 'Content A');
const contentB = createBlock('content-b', 'Content B');
const assignedBlock = createBlock('assigned', 'Assigned block');

const root: ChildContainerNode = {
  id: 'root',
  ownerBlockId: 'external-root-launcher',
  parentId: null,
  rank: '001000',
  presentation: 'drawer',
  title: 'Root',
  blockIds: []
};

const occupied: ChildContainerNode = {
  id: 'occupied',
  ownerBlockId: 'external-occupied-launcher',
  parentId: null,
  rank: '002000',
  presentation: 'inline',
  title: 'Occupied',
  blockIds: [assignedBlock.id]
};

function renderPanel(children: ReactNode) {
  return render(<App>{children}</App>);
}

describe('JSX Studio child containers panel', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
  });

  test('AC-003 creates a presented child, mounts multiple existing blocks, and saves one complete tree', async () => {
    const onSaveChildContainers = vi.fn().mockResolvedValue(true);
    renderPanel(
      <JsxStudioChildContainersPanel
        childContainers={[root, occupied]}
        ownerBlock={ownerBlock}
        pageBlocks={[ownerBlock, contentA, contentB, assignedBlock]}
        onSaveChildContainers={onSaveChildContainers}
      />
    );

    fireEvent.click(screen.getAllByText('Root')[0]!);
    fireEvent.click(
      within(
        screen.getByRole('radiogroup', { name: '新容器展示方式' })
      ).getByRole('radio', { name: '弹窗' })
    );
    fireEvent.click(screen.getByRole('button', { name: '新增子级' }));
    expect(screen.getByText('新建弹窗容器')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: '标题' })).toHaveValue(
      '新建弹窗容器'
    );
    expect(
      within(
        screen.getByRole('radiogroup', { name: '容器展示方式' })
      ).getByRole('radio', { name: '弹窗' })
    ).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Launcher' })).toBeDisabled();

    fireEvent.change(screen.getByRole('textbox', { name: '标题' }), {
      target: { value: 'Material editor' }
    });
    expect(
      screen.getByRole('checkbox', { name: 'Assigned block' })
    ).toBeDisabled();
    const contentACheckbox = screen.getByRole('checkbox', {
      name: 'Content A'
    });
    const contentBCheckbox = screen.getByRole('checkbox', {
      name: 'Content B'
    });
    expect(contentACheckbox).toBeEnabled();
    expect(contentBCheckbox).toBeEnabled();
    fireEvent.click(contentACheckbox);
    fireEvent.click(contentBCheckbox);
    fireEvent.click(screen.getByRole('button', { name: '保存子容器' }));

    await waitFor(() => expect(onSaveChildContainers).toHaveBeenCalledTimes(1));
    const savedTree = onSaveChildContainers.mock.calls[0]?.[0] as
      | ChildContainerNode[]
      | undefined;
    expect(savedTree).toHaveLength(3);
    expect(savedTree).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'root', parentId: null }),
        expect.objectContaining({ id: 'occupied', blockIds: ['assigned'] }),
        expect.objectContaining({
          ownerBlockId: 'launcher',
          parentId: 'root',
          presentation: 'modal',
          title: 'Material editor',
          blockIds: ['content-a', 'content-b']
        })
      ])
    );
  });

  test('AC-009 surfaces safe-delete errors for containers with children or mounted blocks', () => {
    const child: ChildContainerNode = {
      id: 'child',
      ownerBlockId: 'launcher',
      parentId: root.id,
      rank: '001000',
      presentation: 'modal',
      title: 'Child',
      blockIds: [contentA.id]
    };
    renderPanel(
      <JsxStudioChildContainersPanel
        childContainers={[root, child]}
        ownerBlock={ownerBlock}
        pageBlocks={[ownerBlock, contentA]}
        onSaveChildContainers={vi.fn()}
      />
    );

    const tree = screen.getByRole('tree');
    fireEvent.click(within(tree).getByText('Root'));
    fireEvent.click(screen.getByRole('button', { name: '删除容器 Root' }));
    expect(
      screen.getByText('请先移动或删除下级容器，再删除当前容器。')
    ).toBeInTheDocument();

    fireEvent.click(within(tree).getByText('Child'));
    fireEvent.click(screen.getByRole('button', { name: '删除容器 Child' }));
    expect(
      screen.getByText('请先移除全部挂载区块，再删除当前容器。')
    ).toBeInTheDocument();
  });

  test('AC-004/009 saves registered targets, inserts the event source, and blocks referenced deletion', async () => {
    const referencedOwner = createBlock('launcher', 'Launcher', ['root']);
    const ownedRoot = { ...root, ownerBlockId: referencedOwner.id };
    const onSaveBlock = vi.fn().mockResolvedValue(true);
    const onInsertCode = vi.fn();
    renderPanel(
      <JsxStudioChildContainersPanel
        childContainers={[ownedRoot]}
        ownerBlock={referencedOwner}
        pageBlocks={[referencedOwner, contentA]}
        onInsertCode={onInsertCode}
        onSaveBlock={onSaveBlock}
        onSaveChildContainers={vi.fn()}
      />
    );

    fireEvent.click(screen.getAllByText('Root')[0]!);
    fireEvent.click(screen.getByRole('button', { name: '插入打开事件 Root' }));
    expect(onInsertCode).toHaveBeenCalledWith({
      kind: 'source',
      source: `ctx.events.emit('open_child_container', { container_id: "root" });`
    });

    fireEvent.click(screen.getByRole('button', { name: '保存目标子容器' }));
    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock).toHaveBeenCalledWith(
      expect.objectContaining({ childContainerTargetIds: ['root'] })
    );

    fireEvent.click(screen.getByRole('button', { name: '删除容器 Root' }));
    expect(
      screen.getByText('请先移除区块目标引用，再删除当前容器。')
    ).toBeInTheDocument();
  });

  test('AC-002/003 moves a container across parents and persists presentation by stable controls', async () => {
    const parentA = { ...root, id: 'parent-a', title: 'Parent A' };
    const parentB = {
      ...occupied,
      id: 'parent-b',
      title: 'Parent B',
      blockIds: []
    };
    const child: ChildContainerNode = {
      id: 'movable',
      ownerBlockId: ownerBlock.id,
      parentId: parentA.id,
      rank: '001000',
      presentation: 'drawer',
      title: 'Movable',
      blockIds: []
    };
    const onSaveChildContainers = vi.fn().mockResolvedValue(true);
    renderPanel(
      <JsxStudioChildContainersPanel
        childContainers={[parentA, parentB, child]}
        ownerBlock={ownerBlock}
        pageBlocks={[ownerBlock]}
        onInsertCode={vi.fn()}
        onSaveBlock={vi.fn()}
        onSaveChildContainers={onSaveChildContainers}
      />
    );

    fireEvent.click(screen.getByText('Movable'));
    const modalPresentation = within(
      screen.getByRole('radiogroup', { name: '容器展示方式' })
    ).getByRole('radio', { name: '弹窗' });
    fireEvent.click(modalPresentation);
    expect(modalPresentation).toBeChecked();
    fireEvent.change(screen.getByRole('combobox', { name: '父级容器' }), {
      target: { value: 'parent-b' }
    });
    const saveButton = screen.getByRole('button', { name: '保存子容器' });
    expect(saveButton).toBeEnabled();
    fireEvent.click(saveButton);

    await waitFor(() => expect(onSaveChildContainers).toHaveBeenCalledTimes(1));
    expect(onSaveChildContainers).toHaveBeenCalledWith(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'movable',
          parentId: 'parent-b',
          presentation: 'modal'
        })
      ])
    );
  });
});
