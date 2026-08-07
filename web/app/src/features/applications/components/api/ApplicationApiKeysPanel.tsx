import { useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import { DeleteOutlined, KeyOutlined, QuestionCircleOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { App, Button, Form, Input, Modal, Space, Table, Tooltip, Typography } from 'antd';

import { formatDateTime } from '../../../../shared/i18n/format';
import { copyTextToClipboard } from '../../../../shared/ui/clipboard/copy-text';
import { applicationDetailQueryKey } from '../../api/applications';
import {
  applicationApiKeysQueryKey,
  createApplicationApiKey,
  fetchApplicationApiKeys,
  revokeApplicationApiKey,
  type ApplicationApiKey,
  type CreatedApplicationApiKey
} from '../../api/public-api';

function formatShanghaiDateTime(value: string) {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return formatDateTime(date, {
    timeZone: 'Asia/Shanghai',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hourCycle: 'h23'
  });
}

function formatOptionalDateTime(value: string | null | undefined, t: TFunction<'applications'>) {
  return value ? formatShanghaiDateTime(value) : t('auto.not_used');
}

export function ApplicationApiKeysPanel({
  applicationId,
  csrfToken,
  onCreatedToken,
  variant = 'panel'
}: {
  applicationId: string;
  csrfToken: string;
  onCreatedToken: (token: string | null) => void;
  variant?: 'panel' | 'embedded';
}) {
  const { t } = useTranslation('applications');
  const queryClient = useQueryClient();
  const { message } = App.useApp();
  const [createOpen, setCreateOpen] = useState(false);
  const [listOpen, setListOpen] = useState(false);
  const [createdKey, setCreatedKey] = useState<CreatedApplicationApiKey | null>(null);
  const [form] = Form.useForm<{ name: string }>();
  const keysQuery = useQuery({
    queryKey: applicationApiKeysQueryKey(applicationId),
    queryFn: () => fetchApplicationApiKeys(applicationId)
  });
  const createMutation = useMutation({
    mutationFn: (name: string) => createApplicationApiKey(applicationId, name, csrfToken),
    onSuccess: (key) => {
      setCreatedKey(key);
      onCreatedToken(key.token);
      setCreateOpen(false);
      form.resetFields();
      void queryClient.invalidateQueries({
        queryKey: applicationApiKeysQueryKey(applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: applicationDetailQueryKey(applicationId)
      });
    }
  });
  const revokeMutation = useMutation({
    mutationFn: (keyId: string) => revokeApplicationApiKey(applicationId, keyId, csrfToken),
    onSuccess: () => {
      message.success(t('auto.api_key_deleted'));
      void queryClient.invalidateQueries({
        queryKey: applicationApiKeysQueryKey(applicationId)
      });
      void queryClient.invalidateQueries({
        queryKey: applicationDetailQueryKey(applicationId)
      });
    }
  });

  const keys = keysQuery.data ?? [];
  const maskTokenPreview = (value: string) => {
    return `${value}****`;
  };
  const copyCreatedToken = () => {
    const token = createdKey?.token;
    if (!token) {
      return;
    }
    copyTextToClipboard(token).then(
      () => message.success(t('auto.api_key_copied')),
      () => message.error(t('auto.copy_failed'))
    );
  };
  const keyTable = (
    <Table<ApplicationApiKey>
      rowKey="id"
      loading={keysQuery.isLoading}
      dataSource={keys}
      pagination={false}
      columns={[
        { title: t('auto.name'), dataIndex: 'name' },
        {
          title: (
            <Space size={6}>
              <span>{t('auto.key')}</span>
              <Tooltip title={t('auto.api_key_prefix_notice')}>
                <QuestionCircleOutlined aria-label={t('auto.text')} />
              </Tooltip>
            </Space>
          ),
          dataIndex: 'token_prefix',
          width: 210,
          render: (value: string) => (
            <Typography.Text code>{maskTokenPreview(value)}</Typography.Text>
          )
        },
        {
          title: t('auto.created_at'),
          dataIndex: 'created_at',
          width: 200,
          render: (value: string) => formatShanghaiDateTime(value)
        },
        {
          title: t('auto.last_used_at'),
          dataIndex: 'last_used_at',
          width: 200,
          render: (value: string | null | undefined) => formatOptionalDateTime(value, t)
        },
        {
          title: t('auto.operation'),
          key: 'actions',
          width: 96,
          render: (_, record) => (
            <Button
              danger
              icon={<DeleteOutlined />}
              size="small"
              type="text"
              aria-label={t('auto.delete')}
              loading={revokeMutation.isPending}
              onClick={() => revokeMutation.mutate(record.id)}
            />
          )
        }
      ]}
    />
  );

  if (variant === 'embedded') {
    return (
      <>
        <Button
          aria-label={t('auto.api_key')}
          className="application-api-key-trigger"
          icon={<KeyOutlined />}
          onClick={() => setListOpen(true)}
        >
          {t('auto.api_key')}</Button>
        <Modal
          title="API Keys"
          open={listOpen}
          destroyOnHidden
          width={840}
          footer={null}
          onCancel={() => setListOpen(false)}
        >
          <Space orientation="vertical" size={16} className="application-api-key-list-modal">
            <div className="application-api-panel__header">
              <Typography.Text type="secondary">
                {t('auto.created')}{keys.length} {t('auto.key_count_suffix')}</Typography.Text>
              <Button type="primary" onClick={() => setCreateOpen(true)}>
                {t('auto.create_key')}</Button>
            </div>
            {keyTable}
          </Space>
        </Modal>
        <Modal
          title={t('auto.create_api_key')}
          open={createOpen}
          destroyOnHidden
          okText={t('auto.create')}
          confirmLoading={createMutation.isPending}
          onCancel={() => setCreateOpen(false)}
          onOk={() => form.submit()}
        >
          <Form
            form={form}
            layout="vertical"
            onFinish={(values) => createMutation.mutate(values.name)}
          >
            <Form.Item
              name="name"
              label={t('auto.key_name')}
              rules={[{ required: true, message: t('auto.key_name_required') }]}
            >
              <Input />
            </Form.Item>
          </Form>
        </Modal>
        <Modal
          title={t('auto.save_created_api_key')}
          open={Boolean(createdKey)}
          className="application-api-created-key-modal"
          destroyOnHidden
          onCancel={() => setCreatedKey(null)}
          footer={[
            <Button key="close" type="text" onClick={() => setCreatedKey(null)}>
              {t('auto.close')}</Button>,
            <Button
              key="copy"
              aria-label={t('auto.copy')}
              className="application-api-created-token-copy"
              onClick={copyCreatedToken}
            >
              {t('auto.copy')}</Button>
          ]}
        >
          <Space orientation="vertical" className="application-api-token-modal">
            <Typography.Text>{t('auto.full_token_shown_once')}</Typography.Text>
            <Typography.Text type="secondary">
              {t('auto.full_token_hidden_after_close')}</Typography.Text>
            <Typography.Text className="application-api-created-token">
              {createdKey?.token}
            </Typography.Text>
          </Space>
        </Modal>
      </>
    );
  }

  return (
    <section className="application-api-panel">
      <div className="application-api-panel__header">
        <div>
          <Typography.Title level={4}>{t('auto.api_keys')}</Typography.Title>
          <Typography.Text type="secondary">{t('auto.current_application_public_api_usage')}</Typography.Text>
        </div>
        <Button type="primary" onClick={() => setCreateOpen(true)}>
          {t('auto.create_key')}</Button>
      </div>
      {keyTable}
      <Modal
        title={t('auto.create_api_key')}
        open={createOpen}
        destroyOnHidden
        okText={t('auto.create')}
        confirmLoading={createMutation.isPending}
        onCancel={() => setCreateOpen(false)}
        onOk={() => form.submit()}
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={(values) => createMutation.mutate(values.name)}
        >
          <Form.Item
            name="name"
            label={t('auto.key_name')}
            rules={[{ required: true, message: t('auto.key_name_required') }]}
          >
            <Input />
          </Form.Item>
        </Form>
      </Modal>
      <Modal
        title={t('auto.save_created_api_key')}
        open={Boolean(createdKey)}
        className="application-api-created-key-modal"
        destroyOnHidden
        onCancel={() => setCreatedKey(null)}
        footer={[
          <Button key="close" type="text" onClick={() => setCreatedKey(null)}>
            {t('auto.close')}</Button>,
          <Button
            key="copy"
            aria-label={t('auto.copy')}
            className="application-api-created-token-copy"
            onClick={copyCreatedToken}
          >
            {t('auto.copy')}</Button>
        ]}
      >
        <Space orientation="vertical" className="application-api-token-modal">
          <Typography.Text>{t('auto.full_token_shown_once')}</Typography.Text>
          <Typography.Text type="secondary">
            {t('auto.full_token_hidden_after_close')}</Typography.Text>
          <Typography.Text className="application-api-created-token">
            {createdKey?.token}
          </Typography.Text>
        </Space>
      </Modal>
    </section>
  );
}
