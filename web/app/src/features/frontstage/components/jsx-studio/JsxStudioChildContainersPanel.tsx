import {
  Alert,
  Button,
  Checkbox,
  Divider,
  Input,
  Radio,
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
  moveChildContainer,
  reorderChildContainer,
  resolveChildContainerPath,
  serializeChildContainerTree,
  type ChildContainerNode,
  type ChildContainerPresentation
} from '../../lib/child-container-tree';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { FrontstageJsxInsertion } from '../../lib/jsx-studio/source-insertion';

export interface JsxStudioChildContainersPanelProps {
  childContainers: readonly ChildContainerNode[];
  ownerBlock: FrontstageBlockInstance;
  pageBlocks: readonly FrontstageBlockInstance[];
  onInsertCode?: (insertion: FrontstageJsxInsertion) => void;
  onSaveBlock?: (block: FrontstageBlockInstance) => Promise<boolean | void>;
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
  onInsertCode,
  onSaveBlock,
  onSaveChildContainers,
  ownerBlock,
  pageBlocks
}: JsxStudioChildContainersPanelProps) {
  const [draft, setDraft] = useState(() => cloneTree(childContainers));
  const [selectedId, setSelectedId] = useState<string>();
  const [newPresentation, setNewPresentation] =
    useState<ChildContainerPresentation>('drawer');
  const [targetContainerIds, setTargetContainerIds] = useState(
    ownerBlock.childContainerTargetIds ?? []
  );
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [savingTargets, setSavingTargets] = useState(false);
  const [feedback, setFeedback] = useState<PanelFeedback | null>(null);
  const canEdit = Boolean(onSaveChildContainers);

  useEffect(() => {
    const nextDraft = cloneTree(childContainers);
    setDraft(nextDraft);
    setSelectedId((current) =>
      current && childContainers.some(({ id }) => id === current)
        ? current
        : undefined
    );
    setDirty(false);
    setFeedback(null);
  }, [childContainers]);
  useEffect(() => {
    setTargetContainerIds(ownerBlock.childContainerTargetIds ?? []);
  }, [ownerBlock.childContainerTargetIds]);

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
  const ownedContainers = draft.filter(
    ({ ownerBlockId }) => ownerBlockId === ownerBlock.id
  );
  const targetReferences = [
    ...new Set(
      pageBlocks.flatMap((block) => block.childContainerTargetIds ?? [])
    )
  ];
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
  const createDraft = () => {
    return {
      ownerBlockId: ownerBlock.id,
      presentation: newPresentation,
      title: i18nText('frontstage', 'auto.child_container_new_title', {
        value1: presentationLabel(newPresentation)
      }),
      blockIds: []
    };
  };
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
        <Radio.Group
          aria-label={i18nText(
            'frontstage',
            'auto.child_container_new_presentation'
          )}
          disabled={!canEdit}
          value={newPresentation}
          onChange={(event) =>
            setNewPresentation(event.target.value as ChildContainerPresentation)
          }
        >
          {(['drawer', 'modal', 'inline'] as const).map((presentation) => (
            <Radio.Button key={presentation} value={presentation}>
              {presentationLabel(presentation)}
            </Radio.Button>
          ))}
        </Radio.Group>
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

      <Divider />
      <section className="frontstage-jsx-studio__resource-section">
        <Typography.Text strong>
          {i18nText('frontstage', 'auto.child_container_targets')}
        </Typography.Text>
        <Select<string[]>
          mode="multiple"
          aria-label={i18nText('frontstage', 'auto.child_container_targets')}
          disabled={!onSaveBlock}
          value={targetContainerIds}
          options={ownedContainers.map((container) => ({
            value: container.id,
            label: container.title
          }))}
          onChange={setTargetContainerIds}
        />
        <Button
          disabled={!onSaveBlock}
          loading={savingTargets}
          onClick={async () => {
            if (!onSaveBlock) return;
            setSavingTargets(true);
            setFeedback(null);
            try {
              const saved = await onSaveBlock({
                ...ownerBlock,
                childContainerTargetIds: [...targetContainerIds]
              });
              if (saved !== false) {
                setFeedback({
                  type: 'success',
                  message: i18nText(
                    'frontstage',
                    'auto.child_container_targets_saved'
                  )
                });
              }
            } catch {
              setFeedback({
                type: 'error',
                message: i18nText(
                  'frontstage',
                  'auto.child_container_targets_save_failed'
                )
              });
            } finally {
              setSavingTargets(false);
            }
          }}
        >
          {i18nText('frontstage', 'auto.save_child_container_targets')}
        </Button>
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
            <Radio.Group
              aria-label={i18nText(
                'frontstage',
                'auto.child_container_presentation'
              )}
              disabled={!canEdit}
              value={selected.presentation}
              onChange={(event) =>
                updateSelected({
                  presentation: event.target.value as ChildContainerPresentation
                })
              }
            >
              {(['drawer', 'modal', 'inline'] as const).map((presentation) => (
                <Radio.Button key={presentation} value={presentation}>
                  {presentationLabel(presentation)}
                </Radio.Button>
              ))}
            </Radio.Group>
            <select
              aria-label={i18nText('frontstage', 'auto.child_container_parent')}
              disabled={!canEdit}
              value={selected.parentId ?? '__page_root__'}
              onChange={(event) => {
                const parentValue = event.target.value;
                const parentId =
                  parentValue === '__page_root__' ? null : parentValue;
                try {
                  const destinationSiblingCount = draft.filter(
                    (container) =>
                      container.parentId === parentId &&
                      container.id !== selected.id
                  ).length;
                  applyDraft(
                    moveChildContainer(
                      draft,
                      selected.id,
                      parentId,
                      destinationSiblingCount
                    )
                  );
                } catch (error) {
                  setFeedback({ type: 'error', message: errorMessage(error) });
                }
              }}
            >
              <option value="__page_root__">
                {i18nText('frontstage', 'auto.child_container_page_root')}
              </option>
              {draft
                .filter(({ id }) => id !== selected.id)
                .map((container) => (
                  <option key={container.id} value={container.id}>
                    {container.title}
                  </option>
                ))}
            </select>
            <Checkbox.Group<string>
              aria-label={i18nText('frontstage', 'auto.child_container_blocks')}
              disabled={!canEdit}
              value={selected.blockIds}
              onChange={(blockIds) => updateSelected({ blockIds })}
            >
              <Space direction="vertical">
                {pageBlocks.map((block) => {
                  const assignedContainerId = assignedContainerByBlock.get(
                    block.id
                  );
                  const disabled =
                    block.id === selected.ownerBlockId ||
                    forbiddenOwnerBlockIds.has(block.id) ||
                    (assignedContainerId !== undefined &&
                      assignedContainerId !== selected.id);
                  const label =
                    typeof block.props.title === 'string'
                      ? block.props.title
                      : i18nText('frontstage', 'auto.block_with_id', {
                          value1: block.id
                        });
                  return (
                    <Checkbox
                      key={block.id}
                      value={block.id}
                      disabled={disabled}
                    >
                      {label}
                    </Checkbox>
                  );
                })}
              </Space>
            </Checkbox.Group>
            <Alert
              showIcon
              type="info"
              title={i18nText(
                'frontstage',
                'auto.child_container_variables_guidance'
              )}
            />
            <Space wrap>
              {selected.ownerBlockId === ownerBlock.id && onInsertCode ? (
                <Button
                  aria-label={i18nText(
                    'frontstage',
                    'auto.insert_open_child_container_event_with_title',
                    { value1: selected.title }
                  )}
                  onClick={() =>
                    onInsertCode({
                      kind: 'source',
                      source: `ctx.events.emit('open_child_container', { container_id: ${JSON.stringify(selected.id)} });`
                    })
                  }
                >
                  {i18nText(
                    'frontstage',
                    'auto.insert_open_child_container_event'
                  )}
                </Button>
              ) : null}
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
                aria-label={i18nText(
                  'frontstage',
                  'auto.delete_child_container_with_title',
                  { value1: selected.title }
                )}
                disabled={!canEdit}
                onClick={() => {
                  try {
                    const next = deleteChildContainer(draft, selected.id, {
                      targetContainerIds: targetReferences
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
