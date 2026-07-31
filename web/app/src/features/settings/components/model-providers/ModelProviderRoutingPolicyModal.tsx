import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type HTMLAttributes
} from 'react';

import { DragOutlined } from '@ant-design/icons';
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent
} from '@dnd-kit/core';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Button, Space, Switch, Table, Typography } from 'antd';
import type { ColumnsType } from 'antd/es/table';

import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import type { SettingsModelProviderMainInstance } from '../../api/model-providers';

type RoutingPolicy =
  SettingsModelProviderMainInstance['model_routing_policies'][number];
type DistributionRule = RoutingPolicy['distribution_rule'];

export interface RoutingPolicyTarget {
  source_instance_id: string;
  source_instance_display_name: string;
  routing_enabled: boolean;
}

type SortableRowContextValue = Pick<
  ReturnType<typeof useSortable>,
  'attributes' | 'listeners' | 'setActivatorNodeRef'
>;

const SortableRowContext = createContext<SortableRowContextValue | null>(null);

function orderedTargetIds(
  policy: RoutingPolicy | undefined,
  targets: RoutingPolicyTarget[]
) {
  const availableIds = new Set(
    targets.map((target) => target.source_instance_id)
  );
  const configuredIds = (policy?.provider_instance_ids ?? []).filter((id) =>
    availableIds.has(id)
  );
  const configuredSet = new Set(configuredIds);
  return [
    ...configuredIds,
    ...targets
      .map((target) => target.source_instance_id)
      .filter((id) => !configuredSet.has(id))
  ];
}

function excludedTargetIds(
  policy: RoutingPolicy | undefined,
  targets: RoutingPolicyTarget[]
) {
  if (policy) {
    return new Set(policy.excluded_provider_instance_ids);
  }
  return new Set(
    targets
      .filter((target) => !target.routing_enabled)
      .map((target) => target.source_instance_id)
  );
}

function SortableTableRow({
  ...props
}: HTMLAttributes<HTMLTableRowElement> & { 'data-row-key': string }) {
  const id = String(props['data-row-key']);
  const {
    attributes,
    listeners,
    setActivatorNodeRef,
    setNodeRef,
    transform,
    transition,
    isDragging
  } = useSortable({ id });
  const contextValue = useMemo(
    () => ({ attributes, listeners, setActivatorNodeRef }),
    [attributes, listeners, setActivatorNodeRef]
  );

  return (
    <SortableRowContext.Provider value={contextValue}>
      <tr
        {...props}
        ref={setNodeRef}
        style={{
          ...props.style,
          transform: CSS.Transform.toString(transform),
          transition,
          ...(isDragging ? { position: 'relative', zIndex: 1 } : {})
        }}
      />
    </SortableRowContext.Provider>
  );
}

function DragHandle() {
  const sortable = useContext(SortableRowContext);
  if (!sortable) {
    return null;
  }

  return (
    <Button
      ref={sortable.setActivatorNodeRef}
      type="text"
      size="small"
      className="model-provider-panel__routing-policy-drag-handle"
      icon={<DragOutlined />}
      aria-label={i18nText('settings', 'auto.drag_to_sort')}
      {...sortable.attributes}
      {...sortable.listeners}
    />
  );
}

export function ModelProviderRoutingPolicyModal({
  open,
  modelId,
  policy,
  targets,
  saving,
  onCancel,
  onSave
}: {
  open: boolean;
  modelId: string;
  policy: RoutingPolicy | undefined;
  targets: RoutingPolicyTarget[];
  saving: boolean;
  onCancel: () => void;
  onSave: (input: {
    distribution_rule: DistributionRule;
    provider_instance_ids: string[];
    excluded_provider_instance_ids: string[];
  }) => void;
}) {
  const [distributionRule, setDistributionRule] = useState<DistributionRule>(
    policy?.distribution_rule ?? 'none'
  );
  const [providerInstanceIds, setProviderInstanceIds] = useState<string[]>([]);
  const [excludedProviderInstanceIds, setExcludedProviderInstanceIds] =
    useState<Set<string>>(new Set());
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );
  const targetById = useMemo(
    () =>
      new Map(
        targets.map((target) => [target.source_instance_id, target] as const)
      ),
    [targets]
  );
  const orderedTargets = useMemo(
    () =>
      providerInstanceIds
        .map((instanceId) => targetById.get(instanceId))
        .filter((target): target is RoutingPolicyTarget => Boolean(target)),
    [providerInstanceIds, targetById]
  );

  useEffect(() => {
    if (!open) {
      return;
    }
    setDistributionRule(policy?.distribution_rule ?? 'none');
    setProviderInstanceIds(orderedTargetIds(policy, targets));
    setExcludedProviderInstanceIds(excludedTargetIds(policy, targets));
  }, [open, policy, targets]);

  const columns: ColumnsType<RoutingPolicyTarget> = [
    {
      key: 'drag',
      width: 48,
      align: 'center',
      render: () => <DragHandle />
    },
    {
      key: 'instance',
      dataIndex: 'source_instance_display_name',
      title: i18nText('settings', 'auto.instance_alt')
    },
    {
      key: 'routing_enabled',
      title: i18nText('settings', 'auto.participate_in_group'),
      width: 128,
      align: 'center',
      render: (_, target) => (
        <Switch
          aria-label={i18nText('settings', 'auto.participate_in_group_named', {
            value1: target.source_instance_display_name
          })}
          checked={!excludedProviderInstanceIds.has(target.source_instance_id)}
          onChange={(checked) => {
            setExcludedProviderInstanceIds((current) => {
              const next = new Set(current);
              if (checked) {
                next.delete(target.source_instance_id);
              } else {
                next.add(target.source_instance_id);
              }
              return next;
            });
          }}
        />
      )
    }
  ];

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) {
      return;
    }
    setProviderInstanceIds((current) => {
      const activeIndex = current.indexOf(String(active.id));
      const overIndex = current.indexOf(String(over.id));
      if (activeIndex < 0 || overIndex < 0) {
        return current;
      }
      return arrayMove(current, activeIndex, overIndex);
    });
  };

  const submit = () =>
    onSave({
      distribution_rule: distributionRule,
      provider_instance_ids: providerInstanceIds,
      excluded_provider_instance_ids: providerInstanceIds.filter((id) =>
        excludedProviderInstanceIds.has(id)
      )
    });

  return (
    <FixedHeightModal
      open={open}
      width={640}
      height="min(560px, calc(100vh - 120px))"
      className="model-provider-panel__routing-policy-modal"
      title={i18nText('settings', 'auto.edit_routing_policy')}
      destroyOnHidden
      footer={
        <Space>
          <Button onClick={onCancel}>
            {i18nText('settings', 'auto.cancel')}
          </Button>
          <Button type="primary" loading={saving} onClick={submit}>
            {i18nText('settings', 'auto.save')}
          </Button>
        </Space>
      }
      onCancel={onCancel}
    >
      <div className="model-provider-panel__routing-policy-form">
        <div>
          <Typography.Text type="secondary">
            {i18nText('settings', 'auto.model_id_alt')}
          </Typography.Text>
          <Typography.Paragraph copyable>{modelId}</Typography.Paragraph>
        </div>
        <label className="model-provider-panel__routing-policy-field">
          <Typography.Text>
            {i18nText('settings', 'auto.distribution_rule')}
          </Typography.Text>
          <select
            aria-label={i18nText('settings', 'auto.distribution_rule')}
            className="model-provider-panel__distribution-rule-select"
            value={distributionRule}
            onChange={(event) =>
              setDistributionRule(event.currentTarget.value as DistributionRule)
            }
          >
            <option value="none">
              {i18nText('settings', 'auto.distribution_rule_none')}
            </option>
            <option value="round_robin">
              {i18nText('settings', 'auto.distribution_rule_round_robin')}
            </option>
            <option value="retry_round_robin">
              {i18nText('settings', 'auto.distribution_rule_retry_round_robin')}
            </option>
          </select>
        </label>
        <div className="model-provider-panel__routing-policy-field">
          <Typography.Text>
            {i18nText('settings', 'auto.group_order')}
          </Typography.Text>
          <Typography.Text type="secondary">
            {i18nText('settings', 'auto.drag_to_adjust_group_order')}
          </Typography.Text>
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            modifiers={[restrictToVerticalAxis]}
            onDragEnd={handleDragEnd}
          >
            <SortableContext
              items={providerInstanceIds}
              strategy={verticalListSortingStrategy}
            >
              <Table<RoutingPolicyTarget>
                className="model-provider-panel__routing-policy-table"
                columns={columns}
                components={{ body: { row: SortableTableRow } }}
                dataSource={orderedTargets}
                pagination={false}
                rowKey="source_instance_id"
                rowClassName={(target) =>
                  excludedProviderInstanceIds.has(target.source_instance_id)
                    ? 'model-provider-panel__routing-policy-row--excluded'
                    : ''
                }
                size="small"
                tableLayout="fixed"
              />
            </SortableContext>
          </DndContext>
        </div>
      </div>
    </FixedHeightModal>
  );
}
