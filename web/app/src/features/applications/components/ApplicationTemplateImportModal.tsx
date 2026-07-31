import {
  Alert,
  Descriptions,
  Input,
  List,
  Modal,
  Space,
  Tag,
  Typography
} from 'antd';
import { useTranslation } from 'react-i18next';

import type { AgentFlowTemplatePreview } from '../api/applications';

interface ApplicationTemplateImportModalProps {
  open: boolean;
  preview: AgentFlowTemplatePreview | null;
  name: string;
  importing: boolean;
  onNameChange: (value: string) => void;
  onCancel: () => void;
  onImport: () => void;
}

function dependencyLabel(dependency: AgentFlowTemplatePreview['dependencies'][number]) {
  if (dependency.dependency.kind === 'model_provider') {
    return [
      dependency.dependency.provider_code,
      dependency.dependency.model_id
    ]
      .filter(Boolean)
      .join(' / ');
  }

  if (dependency.dependency.kind === 'plugin_node') {
    return [
      dependency.dependency.plugin_id,
      dependency.dependency.plugin_version,
      dependency.dependency.contribution_code
    ]
      .filter(Boolean)
      .join(' / ');
  }

  return dependency.dependency.node_type ?? dependency.dependency.kind;
}

export function ApplicationTemplateImportModal({
  open,
  preview,
  name,
  importing,
  onNameChange,
  onCancel,
  onImport
}: ApplicationTemplateImportModalProps) {
  const { t } = useTranslation('applications');
  const unresolvedNodes = preview?.unresolved_nodes ?? [];
  const missingDependencies =
    preview?.dependencies.filter((dependency) => dependency.status !== 'ready') ?? [];

  return (
    <Modal
      open={open}
      title={t('auto.import_template_file')}
      okText={t('auto.import_template')}
      cancelText={t('auto.cancel')}
      okButtonProps={{ disabled: !preview || name.trim().length === 0 }}
      confirmLoading={importing}
      width={720}
      onCancel={onCancel}
      onOk={onImport}
    >
      {preview ? (
        <Space direction="vertical" size={16} style={{ width: '100%' }}>
          <Descriptions column={1} size="small" bordered>
            <Descriptions.Item label={t('auto.application_name')}>
              <Input
                aria-label={t('auto.application_name')}
                value={name}
                maxLength={80}
                onChange={(event) => onNameChange(event.target.value)}
              />
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.application_description')}>
              {preview.application.description || t('auto.application_description_empty')}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.template_dependency_summary')}>
              <Space size="small" wrap>
                <Tag color={missingDependencies.length > 0 ? 'warning' : 'success'}>
                  {t('auto.missing_dependency_count', {
                    value1: missingDependencies.length
                  })}
                </Tag>
                <Tag color={unresolvedNodes.length > 0 ? 'warning' : 'success'}>
                  {t('auto.unresolved_node_count', {
                    value1: unresolvedNodes.length
                  })}
                </Tag>
              </Space>
            </Descriptions.Item>
          </Descriptions>

          {missingDependencies.length > 0 ? (
            <List
              size="small"
              header={<Typography.Text strong>{t('auto.missing_dependencies')}</Typography.Text>}
              dataSource={missingDependencies}
              renderItem={(dependency) => (
                <List.Item>
                  <Space direction="vertical" size={2}>
                    <Typography.Text>{dependencyLabel(dependency)}</Typography.Text>
                    <Typography.Text type="secondary">
                      {dependency.reason ?? dependency.status}
                    </Typography.Text>
                  </Space>
                </List.Item>
              )}
            />
          ) : null}

          {unresolvedNodes.length > 0 ? (
            <List
              size="small"
              header={<Typography.Text strong>{t('auto.unresolved_nodes')}</Typography.Text>}
              dataSource={unresolvedNodes}
              renderItem={(node) => (
                <List.Item>
                  <Space direction="vertical" size={2}>
                    <Typography.Text>
                      {node.alias} · {node.node_id}
                    </Typography.Text>
                    <Typography.Text type="secondary">
                      {node.original_type} · {node.reason}
                    </Typography.Text>
                  </Space>
                </List.Item>
              )}
            />
          ) : (
            <Alert type="success" showIcon message={t('auto.template_ready_to_import')} />
          )}
        </Space>
      ) : null}
    </Modal>
  );
}
