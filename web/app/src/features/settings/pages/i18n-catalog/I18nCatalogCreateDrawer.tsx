import { Button, Drawer, Form, Input, Select, Space } from 'antd';
import { useTranslation } from 'react-i18next';

export interface CreateCustomTranslationValues {
  key: string;
  locale: string;
  translation: string;
}

export function I18nCatalogCreateDrawer({
  open,
  saving,
  onClose,
  onCreate
}: {
  open: boolean;
  saving: boolean;
  onClose: () => void;
  onCreate: (values: CreateCustomTranslationValues) => void;
}) {
  const { t } = useTranslation('settings');
  const [form] = Form.useForm<CreateCustomTranslationValues>();

  return (
    <Drawer
      destroyOnClose
      onClose={onClose}
      open={open}
      title={t('auto.translation_catalog_create_custom_key')}
      width="min(480px, 100vw)"
      data-testid="i18n-catalog-create-drawer"
    >
      <Form
        form={form}
        initialValues={{ locale: 'zh_Hans' }}
        layout="vertical"
        onFinish={onCreate}
      >
        <Form.Item
          label={t('auto.key')}
          name="key"
          rules={[{ required: true, whitespace: true }]}
        >
          <Input />
        </Form.Item>
        <Form.Item
          label={t('auto.translation_catalog_locale')}
          name="locale"
          rules={[{ required: true }]}
        >
          <Select
            options={[
              { value: 'zh_Hans', label: 'zh_Hans' },
              { value: 'en_US', label: 'en_US' }
            ]}
          />
        </Form.Item>
        <Form.Item
          label={t('auto.translation_catalog_custom_translation')}
          name="translation"
          rules={[{ required: true, whitespace: true }]}
        >
          <Input.TextArea autoSize={{ minRows: 3, maxRows: 8 }} />
        </Form.Item>
        <Space>
          <Button type="primary" htmlType="submit" loading={saving}>
            {t('auto.translation_catalog_create')}
          </Button>
          <Button onClick={onClose}>
            {t('auto.translation_catalog_cancel')}
          </Button>
        </Space>
      </Form>
    </Drawer>
  );
}
