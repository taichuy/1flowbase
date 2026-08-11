import {
  Alert,
  Button,
  Divider,
  Input,
  Select,
  Space,
  Tag,
  Tree,
  Typography
} from 'antd';
import type { DataNode } from 'antd/es/tree';
import { useEffect, useMemo, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import {
  ChildContainerTreeError,
  addChildContainer,
  addSiblingChildContainer,
  deleteChildContainer,
  reorderChildContainer,
  resolveChildContainerPath,
  serializeChildContainerTree,
  type ChildContainerNode,
  type ChildContainerPresentation
} from '../../lib/child-container-tree';
import type { FrontstageBlockInstance } from '../../lib/page-document';

export interface JsxStudioChildContainersPanelProps {
  childContainers: readonly ChildContainerNode[];
  ownerBlock: FrontstageBlockInstance;
  pageBlocks: readonly FrontstageBlockInstance[];
  onSaveChildContainers?: (
    containers: ChildContainerNode[]
  ) => Promise<boolean | void>;
}

type PanelFeedback = { type: 'error' | 'success'; message: string };

function cloneTree(containers: readonly ChildContainerNode[]) {
  return containers.map((container) => ({
    ...container,
    blockIds: [...container.blockIds]
  }));
}

function presentationLabel(presentation: ChildContainerPresentation) {
  if (presentation === 'drawer') {
    return i18nText('frontstage', 'auto.child_container_drawer');
  }
  if (presentation === 'modal') {
    return i18nText('frontstage', 'auto.child_container_modal');
  }
  return i18nText('frontstage', 'auto.child_container_inline');
}

function errorMessage(error: unknown) {
  if (!(error instanceof ChildContainerTreeError)) {
    return i18nText('frontstage', 'auto.child_container_change_failed');
  }
  if (error.code === 'container_has_children') {
    return i18nText(
      'frontstage',
      'auto.child_container_delete_blocked_children'
    );
  }
  if (error.code === 'container_not_empty') {
    return i18nText('frontstage', 'auto.child_container_delete_blocked_blocks');
  }
  if (error.code === 'container_referenced') {
    return i18nText(
      'frontstage',
      'auto.child_container_delete_blocked_reference'
    );
  }
  if (error.code === 'owner_self_containment') {
    return i18nText(
      'frontstage',
      'auto.child_container_owner_self_containment'
    );
  }
  if (error.code === 'duplicate_block_assignment') {
    return i18nText(
      'frontstage',
      'auto.child_container_duplicate_block_assignment'
    );
  }
  return i18nText('frontstage', 'auto.child_container_change_failed');
}

function createTreeData(containers: readonly ChildContainerNode[]): DataNode[] {
  const childrenByParent = new Map<string | null, ChildContainerNode[]>();
  for (const container of containers) {
    const siblings = childrenByParent.get(container.parentId) ?? [];
    siblings.push(container);
    childrenByParent.set(container.parentId, siblings);
  }
  for (const siblings of childrenByParent.values()) {
    siblings.sort(
      (left, right) =>
        left.rank.localeCompare(right.rank) || left.id.localeCompare(right.id)
    );
  }
  const visit = (parentId: string | null): DataNode[] =>
    (childrenByParent.get(parentId) ?? []).map((container) => ({
      key: container.id,
      title: (
        <Space size={4}>
          <Typography.Text>{container.title}</Typography.Text>
          <Tag bordered={false}>
            {presentationLabel(container.presentation)}
          </Tag>
        </Space>
      ),
      children: visit(container.id)
    }));
  return visit(null);
}

export function JsxStudioChildContainersPanel({
  childContainers,
  onSaveChildContainers,
  ownerBlock,
  pageBlocks
}: JsxStudioChildContainersPanelProps) {
  const [draft, setDraft] = useState(() => cloneTree(childContainers));
  const [selectedId, setSelectedId] = useState<string>();
  const [newPresentation, setNewPresentation] =
    useState<ChildContainerPresentation>('drawer');
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [feedback, setFeedback] = useState<PanelFeedback | null>(null);
  const canEdit = Boolean(onSaveChildContainers);

  useEffect(() => {
    setDraft(cloneTree(childContainers));
    setSelectedId((current) =>
      current && childContainers.some(({ id }) => id === current)
        ? current
        : undefined
    );
    setDirty(false);
    setFeedback(null);
  }, [childContainers]);

  const selected = draft.find(({ id }) => id === selectedId);
  const selectedPath = useMemo(
    () => (selectedId ? resolveChildContainerPath(draft, selectedId) : null),
    [draft, selectedId]
  );
  const forbiddenOwnerBlockIds = new Set(
    selectedPath?.map(({ ownerBlockId }) => ownerBlockId) ?? []
  );
  const assignedContainerByBlock = new Map<string, string>();
  for (const container of draft) {
    for (const blockId of container.blockIds) {
      assignedContainerByBlock.set(blockId, container.id);
    }
  }
  const siblings = selected
    ? draft
        .filter(({ parentId }) => parentId === selected.parentId)
        .sort(
          (left, right) =>
            left.rank.localeCompare(right.rank) ||
            left.id.localeCompare(right.id)
        )
    : [];
  const selectedSiblingIndex = selected
    ? siblings.findIndex(({ id }) => id === selected.id)
    : -1;

  const applyDraft = (next: ChildContainerNode[]) => {
    try {
      serializeChildContainerTree(next);
      setDraft(next);
      setDirty(true);
      setFeedback(null);
    } catch (error) {
      setFeedback({ type: 'error', message: errorMessage(error) });
    }
  };
  const createDraft = () => ({
    ownerBlockId: ownerBlock.id,
    presentation: newPresentation,
    title: i18nText('frontstage', 'auto.child_container_new_title', {
      value1: presentationLabel(newPresentation)
    }),
    blockIds: []
  });
  const addRoot = () => {
    try {
      applyDraft(addChildContainer(draft, null, createDraft()));
    } catch (error) {
      setFeedback({ type: 'error', message: errorMessage(error) });
    }
  };
  const addChild = () => {
    if (!selected) return;
    try {
      applyDraft(addChildContainer(draft, selected.id, createDraft()));
    } catch (error) {
      setFeedback({ type: 'error', message: errorMessage(error) });
    }
  };
  const addSibling = () => {
    if (!selected) return;
    try {
      applyDraft(addSiblingChildContainer(draft, selected.id, createDraft()));
    } catch (error) {
      setFeedback({ type: 'error', message: errorMessage(error) });
    }
  };
  const updateSelected = (change: Partial<ChildContainerNode>) => {
    if (!selected) return;
    applyDraft(
      draft.map((container) =>
        container.id === selected.id ? { ...container, ...change } : container
      )
    );
  };

  return (
    <div className="frontstage-jsx-studio__resource-scroll">
      <Space direction="vertical" size={4}>
        <Typography.Title level={5}>
          {i18nText('frontstage', 'auto.child_containers')}
        </Typography.Title>
        <Typography.Paragraph type="secondary">
          {i18nText('frontstage', 'auto.child_containers_description')}
        </Typography.Paragraph>
      </Space>

      {!canEdit ? (
        <Alert
          showIcon
          type="error"
          title={i18nText(
            'frontstage',
            'auto.child_container_save_unavailable'
          )}
          description={i18nText(
            'frontstage',
            'auto.child_container_save_unavailable_description'
          )}
        />
      ) : null}
      {feedback ? (
        <Alert showIcon type={feedback.type} title={feedback.message} />
      ) : null}

      <section className="frontstage-jsx-studio__resource-section">
        <Select<ChildContainerPresentation>
          aria-label={i18nText(
            'frontstage',
            'auto.child_container_new_presentation'
          )}
          disabled={!canEdit}
          value={newPresentation}
          options={(['drawer', 'modal', 'inline'] as const).map((value) => ({
            value,
            label: presentationLabel(value)
          }))}
          onChange={setNewPresentation}
        />
        <Space wrap>
          <Button disabled={!canEdit} onClick={addRoot}>
            {i18nText('frontstage', 'auto.child_container_add_root')}
          </Button>
          <Button disabled={!canEdit || !selected} onClick={addChild}>
            {i18nText('frontstage', 'auto.child_container_add_child')}
          </Button>
          <Button disabled={!canEdit || !selected} onClick={addSibling}>
            {i18nText('frontstage', 'auto.child_container_add_sibling')}
          </Button>
        </Space>
        {draft.length === 0 ? (
          <Typography.Text type="secondary">
            {i18nText('frontstage', 'auto.child_container_empty')}
          </Typography.Text>
        ) : (
          <Tree
            blockNode
            defaultExpandAll
            selectedKeys={selectedId ? [selectedId] : []}
            treeData={createTreeData(draft)}
            onSelect={(keys) =>
              setSelectedId(keys[0] ? String(keys[0]) : undefined)
            }
          />
        )}
      </section>

      {selected ? (
        <>
          <Divider />
          <section className="frontstage-jsx-studio__resource-section">
            <Input
              aria-label={i18nText('frontstage', 'auto.title')}
              disabled={!canEdit}
              value={selected.title}
              onChange={(event) =>
                updateSelected({ title: event.target.value })
              }
            />
            <Select<ChildContainerPresentation>
              aria-label={i18nText(
                'frontstage',
                'auto.child_container_presentation'
              )}
              disabled={!canEdit}
              value={selected.presentation}
              options={(['drawer', 'modal', 'inline'] as const).map(
                (value) => ({
                  value,
                  label: presentationLabel(value)
                })
              )}
              onChange={(presentation) => updateSelected({ presentation })}
            />
            <Select<string[]>
              mode="multiple"
              aria-label={i18nText('frontstage', 'auto.child_container_blocks')}
              disabled={!canEdit}
              value={selected.blockIds}
              options={pageBlocks.map((block) => {
                const assignedContainerId = assignedContainerByBlock.get(
                  block.id
                );
                return {
                  value: block.id,
                  label:
                    typeof block.props.title === 'string'
                      ? block.props.title
                      : i18nText('frontstage', 'auto.block_with_id', {
                          value1: block.id
                        }),
                  disabled:
                    forbiddenOwnerBlockIds.has(block.id) ||
                    (assignedContainerId !== undefined &&
                      assignedContainerId !== selected.id)
                };
              })}
              onChange={(blockIds) => updateSelected({ blockIds })}
            />
            <Alert
              showIcon
              type="info"
              title={i18nText(
                'frontstage',
                'auto.child_container_variables_guidance'
              )}
            />
            <Space wrap>
              <Button
                disabled={!canEdit || selectedSiblingIndex <= 0}
                onClick={() =>
                  applyDraft(
                    reorderChildContainer(
                      draft,
                      selected.id,
                      selectedSiblingIndex - 1
                    )
                  )
                }
              >
                {i18nText('frontstage', 'auto.move_up')}
              </Button>
              <Button
                disabled={
                  !canEdit || selectedSiblingIndex >= siblings.length - 1
                }
                onClick={() =>
                  applyDraft(
                    reorderChildContainer(
                      draft,
                      selected.id,
                      selectedSiblingIndex + 1
                    )
                  )
                }
              >
                {i18nText('frontstage', 'auto.move_down')}
              </Button>
              <Button
                danger
                disabled={!canEdit}
                onClick={() => {
                  try {
                    const next = deleteChildContainer(draft, selected.id, {
                      targetContainerIds: []
                    });
                    applyDraft(next);
                    setSelectedId(undefined);
                  } catch (error) {
                    setFeedback({
                      type: 'error',
                      message: errorMessage(error)
                    });
                  }
                }}
              >
                {i18nText('frontstage', 'auto.delete')}
              </Button>
            </Space>
          </section>
        </>
      ) : null}

      <Divider />
      <Button
        block
        type="primary"
        disabled={!canEdit || !dirty}
        loading={saving}
        onClick={async () => {
          if (!onSaveChildContainers) return;
          setSaving(true);
          setFeedback(null);
          try {
            const saved = await onSaveChildContainers(cloneTree(draft));
            if (saved !== false) {
              setDirty(false);
              setFeedback({
                type: 'success',
                message: i18nText('frontstage', 'auto.child_containers_saved')
              });
            }
          } catch {
            setFeedback({
              type: 'error',
              message: i18nText(
                'frontstage',
                'auto.child_containers_save_failed'
              )
            });
          } finally {
            setSaving(false);
          }
        }}
      >
        {i18nText('frontstage', 'auto.save_child_containers')}
      </Button>
    </div>
  );
}
