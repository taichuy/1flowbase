import { useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Flex,
  Form,
  Input,
  Space,
  Switch,
  Typography,
  Upload
} from 'antd';
import UploadOutlined from '@ant-design/icons/es/icons/UploadOutlined';
import { useTranslation } from 'react-i18next';
import type { SystemMetadata } from '@1flowbase/api-client/auth';
import { useAuthStore } from '../../../../state/auth-store';
import { systemMetadataQueryKey } from '../../../../shared/api/system-metadata';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import {
  fetchConsoleSystemSettings,
  saveConsoleSystemSettings,
  systemSettingsQueryKey
} from '../../api/system-branding';
import './system-settings.css';

export function SystemSettingsPanel() {
  const { t } = useTranslation('settings');
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [form] = Form.useForm<SystemMetadata>();
  const logo_url = Form.useWatch('logo_url', form);
  const query = useQuery({
    queryKey: systemSettingsQueryKey,
    queryFn: fetchConsoleSystemSettings
  });
  useEffect(() => {
    if (query.data) form.setFieldsValue(query.data);
  }, [query.data, form]);
  const mutation = useMutation({
    mutationFn: (value: SystemMetadata) =>
      saveConsoleSystemSettings(value, csrfToken ?? ''),
    onSuccess(value) {
      queryClient.setQueryData(systemSettingsQueryKey, value);
      queryClient.setQueryData(systemMetadataQueryKey, value);
      form.setFieldsValue(value);
      void message.success(t('system_settings.saved'));
    }
  });

  if (query.isPending) return <LoadingState compact />;
  if (query.isError)
    return (
      <Alert
        type="error"
        showIcon
        title={t('system_settings.load_failed')}
        action={
          <Button onClick={() => void query.refetch()}>
            {t('system_settings.retry')}
          </Button>
        }
      />
    );

  return (
    <SettingsSectionSurface heightMode="fill">
      <div className="system-settings">
        <Typography.Title level={4}>
          {t('auto.basic_information')}
        </Typography.Title>
        <Typography.Paragraph type="secondary">
          {t('system_settings.description')}
        </Typography.Paragraph>
        <Form
          form={form}
          layout="vertical"
          onFinish={(value) => mutation.mutate(value)}
          disabled={mutation.isPending}
        >
          <Form.Item name="logo_url" hidden>
            <Input />
          </Form.Item>
          <Form.Item
            label={t('system_settings.logo')}
            extra={t('system_settings.logo_help')}
          >
            <Flex align="center" gap={16} wrap>
              {logo_url ? (
                <img
                  className="system-settings__logo"
                  src={logo_url}
                  alt={t('system_settings.logo')}
                />
              ) : null}
              <Upload
                accept="image/png,image/jpeg,image/svg+xml"
                showUploadList={false}
                beforeUpload={(file) => {
                  if (
                    !['image/png', 'image/jpeg', 'image/svg+xml'].includes(
                      file.type
                    ) ||
                    file.size > 512 * 1024
                  ) {
                    void message.error(t('system_settings.logo_invalid'));
                    return Upload.LIST_IGNORE;
                  }
                  const reader = new FileReader();
                  reader.onload = () =>
                    form.setFieldValue('logo_url', reader.result as string);
                  reader.onerror = () =>
                    void message.error(t('system_settings.logo_failed'));
                  reader.readAsDataURL(file);
                  return false;
                }}
              >
                <Button icon={<UploadOutlined />}>
                  {t('system_settings.upload')}
                </Button>
              </Upload>
            </Flex>
          </Form.Item>
          <Form.Item
            name="system_name"
            label={t('system_settings.system_name')}
            rules={[
              {
                required: true,
                whitespace: true,
                message: t('system_settings.name_required')
              }
            ]}
            extra={t('system_settings.system_name_help')}
          >
            <Input maxLength={100} />
          </Form.Item>
          <Form.Item
            name="hide_login_tagline"
            label={t('system_settings.hide_login_tagline')}
            extra={t('system_settings.hide_login_tagline_help')}
            valuePropName="checked"
          >
            <Switch aria-label={t('system_settings.hide_login_tagline')} />
          </Form.Item>
          <Form.Item
            name="system_description"
            label={t('system_settings.system_description')}
          >
            <Input.TextArea rows={3} maxLength={500} showCount />
          </Form.Item>
          <Form.Item
            name="filing_html"
            label={t('system_settings.filing_html')}
            extra={t('system_settings.filing_help')}
          >
            <Input.TextArea rows={4} />
          </Form.Item>
          {mutation.isError ? (
            <Alert
              className="system-settings__error"
              type="error"
              showIcon
              title={t('system_settings.save_failed')}
            />
          ) : null}
          <Space>
            <Button
              type="primary"
              htmlType="submit"
              loading={mutation.isPending}
            >
              {t('auto.save')}
            </Button>
            <Button
              onClick={() => {
                form.setFieldsValue(query.data!);
                mutation.reset();
              }}
            >
              {t('auto.reset')}
            </Button>
          </Space>
        </Form>
      </div>
    </SettingsSectionSurface>
  );
}
