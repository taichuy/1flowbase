import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { JsxStudioChildContainersPanel } from '../../components/jsx-studio/JsxStudioChildContainersPanel';
import type { ChildContainerNode } from '../../lib/child-container-tree';
import type { FrontstageBlockInstance } from '../../lib/page-document';

function createBlock(id: string, title: string): FrontstageBlockInstance {
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

    fireEvent.click(screen.getByText('Root'));
    fireEvent.mouseDown(
      screen.getByRole('combobox', { name: '新容器展示方式' })
    );
    fireEvent.click(await screen.findByText('弹窗'));
    fireEvent.click(screen.getByRole('button', { name: '新增子级' }));
    fireEvent.click(screen.getByText('新建弹窗容器'));

    fireEvent.change(screen.getByRole('textbox', { name: '标题' }), {
      target: { value: 'Material editor' }
    });
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '挂载区块' }));

    expect(
      screen.getByText('Launcher').closest('[aria-disabled="true"]')
    ).not.toBeNull();
    expect(
      screen.getByText('Assigned block').closest('[aria-disabled="true"]')
    ).not.toBeNull();
    fireEvent.click(screen.getByText('Content A'));
    fireEvent.click(screen.getByText('Content B'));
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

    fireEvent.click(screen.getByText('Root'));
    fireEvent.click(screen.getByRole('button', { name: '删除' }));
    expect(
      screen.getByText('请先移动或删除下级容器，再删除当前容器。')
    ).toBeInTheDocument();

    fireEvent.click(screen.getByText('Child'));
    fireEvent.click(screen.getByRole('button', { name: '删除' }));
    expect(
      screen.getByText('请先移除全部挂载区块，再删除当前容器。')
    ).toBeInTheDocument();
  });
});
