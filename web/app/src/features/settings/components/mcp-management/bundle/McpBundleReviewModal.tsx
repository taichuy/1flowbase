import {
  Alert,
  Descriptions,
  List,
  Modal,
  Space,
  Spin,
  Table,
  Tag
} from 'antd';

import { i18nText } from '../../../../../shared/i18n/text';
import type {
  SettingsMcpBundleImportReport,
  SettingsMcpBundlePreview
} from '../../../api/mcp-management';

export type McpBundleReview =
  | SettingsMcpBundlePreview
  | SettingsMcpBundleImportReport;

function versionWarning(status: SettingsMcpBundlePreview['version_status']) {
  if (status === 'exported_from_older_system') {
    return i18nText('settingsMcpManagement', 'auto.mcp_bundle_source_older');
  }
  if (status === 'exported_from_newer_system') {
    return i18nText('settingsMcpManagement', 'auto.mcp_bundle_source_newer');
  }
  if (status === 'unknown_system_version') {
    return i18nText('settingsMcpManagement', 'auto.mcp_bundle_source_unknown');
  }
  return null;
}

function itemReason(reason: string | null) {
  switch (reason) {
    case 'interface_missing':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_interface_missing'
      );
    case 'tool_id_conflict':
    case 'instance_id_conflict':
    case 'connection_id_conflict':
      return i18nText('settingsMcpManagement', 'auto.mcp_bundle_id_conflict');
    case 'connection_missing':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_connection_missing'
      );
    case 'credentials_missing':
      return i18nText(
        'settingsMcpManagement',
        'auto.upstream_credentials_missing'
      );
    case 'binding_tool_missing':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_binding_tool_missing'
      );
    default:
      return reason ?? '-';
  }
}

function resultColor(result: string) {
  if (result === 'imported') return 'green';
  if (result === 'already_present') return 'blue';
  if (result === 'unavailable' || result === 'failed') return 'red';
  return 'default';
}

function itemResult(result: string) {
  switch (result) {
    case 'imported':
      return i18nText('settingsMcpManagement', 'auto.upstream_imported');
    case 'already_present':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_already_present'
      );
    case 'unavailable':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_result_unavailable'
      );
    case 'skipped':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_result_skipped'
      );
    case 'failed':
      return i18nText('settingsMcpManagement', 'auto.mcp_bundle_failed');
    default:
      return result;
  }
}

function itemEffect(effect: string) {
  if (effect === 'create') {
    return i18nText('settingsMcpManagement', 'auto.mcp_bundle_effect_create');
  }
  if (effect === 'update') {
    return i18nText('settingsMcpManagement', 'auto.mcp_bundle_effect_update');
  }
  return effect;
}

export function McpBundleReviewModal({
  open,
  review,
  loading = false,
  importing,
  integrityWarnings = [],
  onCancel,
  onImport
}: {
  open?: boolean;
  review: McpBundleReview | null;
  loading?: boolean;
  importing: boolean;
  integrityWarnings?: string[];
  onCancel: () => void;
  onImport: () => void;
}) {
  const warning = review ? versionWarning(review.version_status) : null;
  const importReport = review && 'status' in review ? review : null;
  const imported = Boolean(importReport);
  const rows = review
    ? [
        ...review.tools.map((item) => ({ ...item, kind: 'Tool' })),
        ...review.instances.map((item) => ({ ...item, kind: 'Instance' })),
        ...review.connections.map((item) => ({ ...item, kind: 'Connection' }))
      ]
    : [];

  return (
    <Modal
      width={760}
      open={open ?? review !== null}
      title={i18nText('settingsMcpManagement', 'auto.mcp_bundle_import_title')}
      okText={
        loading
          ? i18nText('settingsMcpManagement', 'auto.mcp_bundle_preview')
          : imported
            ? i18nText('settings', 'auto.close')
            : i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_confirm_overwrite'
              )
      }
      cancelButtonProps={{ style: imported ? { display: 'none' } : undefined }}
      okButtonProps={{ disabled: loading }}
      confirmLoading={importing}
      onCancel={onCancel}
      onOk={imported ? onCancel : onImport}
    >
      {loading ? (
        <Spin />
      ) : review ? (
        <Space direction="vertical" size="middle" style={{ width: '100%' }}>
          {integrityWarnings.length > 0 ? (
            <Alert
              showIcon
              type="warning"
              message={
                <List
                  size="small"
                  dataSource={integrityWarnings}
                  renderItem={(message) => <List.Item>{message}</List.Item>}
                />
              }
            />
          ) : null}
          {warning ? <Alert showIcon type="warning" message={warning} /> : null}
          {!imported ? (
            <Alert
              showIcon
              type="info"
              message={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_overwrite_notice'
              )}
            />
          ) : null}
          {review.shared_tool_impacts.length > 0 ? (
            <Alert
              showIcon
              type="warning"
              message={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_shared_tool_impact'
              )}
              description={
                <List
                  size="small"
                  dataSource={review.shared_tool_impacts}
                  renderItem={(impact) => (
                    <List.Item>
                      {impact.tool_id}: {impact.instance_ids.join(', ')}
                    </List.Item>
                  )}
                />
              }
            />
          ) : null}
          {imported ? (
            <Alert
              showIcon
              type={
                importReport?.status === 'completed' ||
                importReport?.status === 'already_applied'
                  ? 'success'
                  : 'warning'
              }
              message={
                importReport?.status === 'already_applied'
                  ? i18nText(
                      'settingsMcpManagement',
                      'auto.mcp_bundle_already_applied'
                    )
                  : importReport?.status === 'completed'
                    ? i18nText(
                        'settingsMcpManagement',
                        'auto.mcp_bundle_import_completed'
                      )
                    : i18nText(
                        'settingsMcpManagement',
                        'auto.mcp_bundle_import_completed_with_warnings'
                      )
              }
            />
          ) : null}
          <Descriptions size="small" column={2}>
            <Descriptions.Item
              label={i18nText('settingsMcpManagement', 'auto.mcp_bundle_name')}
            >
              {review.manifest.organization}/{review.manifest.bundle_id}
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_version'
              )}
            >
              {review.manifest.bundle_version}
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_source_version'
              )}
            >
              {review.manifest.exported_from_system_version}
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_current_version'
              )}
            >
              {review.current_system_version}
            </Descriptions.Item>
          </Descriptions>
          <Descriptions size="small" column={3} bordered>
            <Descriptions.Item
              label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_changes'
              )}
            >
              {review.effect_summary.changes}
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_already_present'
              )}
            >
              {review.effect_summary.already_present}
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_conflicts'
              )}
            >
              {review.effect_summary.conflicts}
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_unavailable'
              )}
            >
              {review.effect_summary.unavailable}
            </Descriptions.Item>
            <Descriptions.Item
              label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_bundle_failed'
              )}
            >
              {review.effect_summary.failed}
            </Descriptions.Item>
          </Descriptions>
          <Table
            size="small"
            rowKey={(item) => `${item.kind}:${item.id}`}
            pagination={false}
            dataSource={rows}
            columns={[
              {
                title: i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_effect'
                ),
                dataIndex: 'effect',
                render: (value: string) => itemEffect(value)
              },
              {
                title: i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_item'
                ),
                dataIndex: 'id'
              },
              {
                title: i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_kind'
                ),
                dataIndex: 'kind'
              },
              {
                title: i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_result'
                ),
                dataIndex: 'result',
                render: (value: string) => (
                  <Tag color={resultColor(value)}>{itemResult(value)}</Tag>
                )
              },
              {
                title: i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_reason'
                ),
                dataIndex: 'reason',
                render: (value: string | null) => itemReason(value)
              }
            ]}
          />
        </Space>
      ) : null}
    </Modal>
  );
}
