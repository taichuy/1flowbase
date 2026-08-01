import { Alert, Descriptions, List, Modal, Space, Table, Tag } from 'antd';

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
  if (result === 'unavailable' || result === 'failed') return 'red';
  return 'default';
}

export function McpBundleReviewModal({
  review,
  importing,
  integrityWarnings = [],
  onCancel,
  onImport
}: {
  review: McpBundleReview | null;
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
      open={review !== null}
      title={i18nText('settingsMcpManagement', 'auto.mcp_bundle_import_title')}
      okText={
        imported
          ? i18nText('settings', 'auto.close')
          : i18nText('settingsMcpManagement', 'auto.mcp_bundle_import_anyway')
      }
      cancelButtonProps={{ style: imported ? { display: 'none' } : undefined }}
      confirmLoading={importing}
      onCancel={onCancel}
      onOk={imported ? onCancel : onImport}
    >
      {review ? (
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
          {imported ? (
            <Alert
              showIcon
              type={
                importReport?.status === 'completed' ? 'success' : 'warning'
              }
              message={
                importReport?.status === 'completed'
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
          <Table
            size="small"
            rowKey={(item) => `${item.kind}:${item.id}`}
            pagination={false}
            dataSource={rows}
            columns={[
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
                  <Tag color={resultColor(value)}>{value}</Tag>
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
