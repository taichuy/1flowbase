import {
  Button,
  Descriptions,
  Divider,
  Drawer,
  Form,
  Input,
  Space,
  Tag,
  Typography
} from 'antd';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import type { SettingsI18nCatalogEntry } from '../../api/i18n-catalog';

interface TranslationFormValues {
  translation: string;
}

export function I18nCatalogEntryDrawer({
  entry,
  loading,
  open,
  saving,
  onClose,
  onDelete,
  onRestore,
  onSave
}: {
  entry: SettingsI18nCatalogEntry | null;
  loading: boolean;
  open: boolean;
  saving: boolean;
  onClose: () => void;
  onDelete: () => void;
  onRestore: () => void;
  onSave: (translation: string) => void;
}) {
  const { t } = useTranslation('settings');
  const [form] = Form.useForm<TranslationFormValues>();

  useEffect(() => {
    if (!entry) return;
    form.setFieldsValue({
      translation:
        entry.origin === 'custom'
          ? (entry.custom_translation ?? entry.effective_value)
          : (entry.override_translation ?? entry.effective_value)
    });
  }, [entry, form]);

  const canDelete = entry?.origin === 'custom';
  const canRestore = Boolean(entry?.override_translation);
  const originLabel = entry
    ? {
        official: t('auto.translation_catalog_origin_official'),
        official_override: t(
          'auto.translation_catalog_origin_official_override'
        ),
        custom: t('auto.translation_catalog_origin_custom'),
        english: t('auto.translation_catalog_origin_english')
      }[entry.origin]
    : '';

  return (
    <Drawer
      destroyOnClose
      loading={loading}
      onClose={onClose}
      open={open}
      placement="right"
      title={t('auto.translation_catalog_entry_details')}
      width="min(480px, 100vw)"
      data-testid="i18n-catalog-entry-drawer"
    >
      {entry ? (
        <>
          <Descriptions bordered column={1} size="small">
            <Descriptions.Item label={t('auto.translation_catalog_module')}>
              <Typography.Text code>{entry.module}</Typography.Text>
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.translation_catalog_msgid')}>
              <Typography.Text code>{entry.msgid}</Typography.Text>
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.translation_catalog_locale')}>
              {entry.locale}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.translation_catalog_origin')}>
              <Tag>{originLabel}</Tag>
            </Descriptions.Item>
            <Descriptions.Item
              label={t('auto.translation_catalog_official_layer')}
            >
              {entry.official_translation ?? '—'}
            </Descriptions.Item>
            <Descriptions.Item
              label={t('auto.translation_catalog_override_layer')}
            >
              {entry.override_translation ?? '—'}
            </Descriptions.Item>
            <Descriptions.Item
              label={t('auto.translation_catalog_custom_layer')}
            >
              {entry.custom_translation ?? '—'}
            </Descriptions.Item>
            <Descriptions.Item
              label={t('auto.translation_catalog_effective_value')}
            >
              {entry.effective_value}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.translation_catalog_revision')}>
              {entry.revision}
            </Descriptions.Item>
          </Descriptions>

          <Divider />
          <Form
            form={form}
            layout="vertical"
            onFinish={({ translation }) => onSave(translation)}
          >
            <Form.Item
              label={
                entry.origin === 'custom'
                  ? t('auto.translation_catalog_custom_translation')
                  : t('auto.translation_catalog_override_translation')
              }
              name="translation"
              rules={[{ required: true, whitespace: true }]}
            >
              <Input.TextArea autoSize={{ minRows: 3, maxRows: 8 }} />
            </Form.Item>
            <Space wrap>
              <Button type="primary" htmlType="submit" loading={saving}>
                {t('auto.translation_catalog_save')}
              </Button>
              {canRestore ? (
                <Button onClick={onRestore} loading={saving}>
                  {t('auto.translation_catalog_restore_entry')}
                </Button>
              ) : null}
              {canDelete ? (
                <Button danger onClick={onDelete} disabled={saving}>
                  {t('auto.translation_catalog_delete_custom_key')}
                </Button>
              ) : null}
            </Space>
          </Form>
        </>
      ) : null}
    </Drawer>
  );
}
