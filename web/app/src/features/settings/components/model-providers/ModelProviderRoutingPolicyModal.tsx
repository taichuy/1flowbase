import { useEffect, useMemo, useState } from 'react';

import { Button, Space, Typography } from 'antd';
import { DragOutlined } from '@ant-design/icons';

import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import type { SettingsModelProviderMainInstance } from '../../api/model-providers';

type RoutingPolicy =
  SettingsModelProviderMainInstance['model_routing_policies'][number];
type DistributionRule = RoutingPolicy['distribution_rule'];

export interface RoutingPolicyTarget {
  source_instance_id: string;
  source_instance_display_name: string;
}

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
  }) => void;
}) {
  const [distributionRule, setDistributionRule] = useState<DistributionRule>(
    policy?.distribution_rule ?? 'none'
  );
  const [providerInstanceIds, setProviderInstanceIds] = useState<string[]>([]);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const targetById = useMemo(
    () =>
      new Map(
        targets.map((target) => [target.source_instance_id, target] as const)
      ),
    [targets]
  );

  useEffect(() => {
    if (!open) {
      return;
    }
    setDistributionRule(policy?.distribution_rule ?? 'none');
    setProviderInstanceIds(orderedTargetIds(policy, targets));
    setDraggingId(null);
  }, [open, policy, targets]);

  const moveTarget = (sourceId: string, targetId: string) => {
    if (sourceId === targetId) {
      return;
    }
    setProviderInstanceIds((current) => {
      const sourceIndex = current.indexOf(sourceId);
      const targetIndex = current.indexOf(targetId);
      if (sourceIndex < 0 || targetIndex < 0) {
        return current;
      }
      const next = [...current];
      next.splice(sourceIndex, 1);
      next.splice(targetIndex, 0, sourceId);
      return next;
    });
  };

  const moveByOffset = (instanceId: string, offset: number) => {
    setProviderInstanceIds((current) => {
      const sourceIndex = current.indexOf(instanceId);
      const targetIndex = sourceIndex + offset;
      if (sourceIndex < 0 || targetIndex < 0 || targetIndex >= current.length) {
        return current;
      }
      const next = [...current];
      [next[sourceIndex], next[targetIndex]] = [
        next[targetIndex],
        next[sourceIndex]
      ];
      return next;
    });
  };

  const submit = () =>
    onSave({
      distribution_rule: distributionRule,
      provider_instance_ids: providerInstanceIds
    });

  return (
    <FixedHeightModal
      open={open}
      width={560}
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
          <ol className="model-provider-panel__routing-policy-targets">
            {providerInstanceIds.map((instanceId, index) => (
              <li
                key={instanceId}
                draggable
                className="model-provider-panel__routing-policy-target"
                onDragStart={() => setDraggingId(instanceId)}
                onDragEnd={() => setDraggingId(null)}
                onDragOver={(event) => event.preventDefault()}
                onDrop={() => {
                  if (draggingId) {
                    moveTarget(draggingId, instanceId);
                  }
                  setDraggingId(null);
                }}
              >
                <DragOutlined aria-hidden />
                <Typography.Text className="model-provider-panel__routing-policy-target-name">
                  {targetById.get(instanceId)?.source_instance_display_name ??
                    instanceId}
                </Typography.Text>
                <Space size={4}>
                  <Button
                    size="small"
                    disabled={index === 0}
                    aria-label={i18nText('settings', 'auto.move_up')}
                    onClick={() => moveByOffset(instanceId, -1)}
                  >
                    ↑
                  </Button>
                  <Button
                    size="small"
                    disabled={index === providerInstanceIds.length - 1}
                    aria-label={i18nText('settings', 'auto.move_down')}
                    onClick={() => moveByOffset(instanceId, 1)}
                  >
                    ↓
                  </Button>
                </Space>
              </li>
            ))}
          </ol>
        </div>
      </div>
    </FixedHeightModal>
  );
}
