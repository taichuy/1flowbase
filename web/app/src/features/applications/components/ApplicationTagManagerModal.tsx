import { Button, Checkbox, Empty, Flex, Form, Input, Modal, Space, Tag, Typography } from 'antd';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { ApplicationTagCatalogEntry } from '../api/applications';

interface ApplicationTagManagerModalProps {
  open: boolean;
  application:
    | {
        name: string;
        tags: Array<{ id: string; name: string }>;
      }
    | null;
  catalogTags: ApplicationTagCatalogEntry[];
  saving?: boolean;
  creating?: boolean;
  onCancel: () => void;
  onSubmit: (tagIds: string[]) => void;
  onCreateTag: (name: string) => Promise<{ id: string; name: string }>;
}

export function ApplicationTagManagerModal({
  open,
  application,
  catalogTags,
  saving = false,
  creating = false,
  onCancel,
  onSubmit,
  onCreateTag
}: ApplicationTagManagerModalProps) {
  const { t } = useTranslation('applications');
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);
  const [newTagName, setNewTagName] = useState('');
  const [errorText, setErrorText] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setSelectedTagIds([]);
      setNewTagName('');
      setErrorText(null);
      return;
    }

    setSelectedTagIds(application?.tags.map((tag) => tag.id) ?? []);
    setNewTagName('');
    setErrorText(null);
  }, [application, open]);

  const handleCreateTag = async () => {
    const normalizedName = newTagName.trim();
    if (!normalizedName) {
      setErrorText(t('auto.new_tag_name_required'));
      return;
    }

    setErrorText(null);
    await onCreateTag(normalizedName);
    setNewTagName('');
  };

  return (
    <Modal
      open={open}
      title={t('auto.manage_application_tags')}
      okText={t('auto.save_tags')}
      cancelText={t('auto.cancel')}
      confirmLoading={saving}
      onCancel={onCancel}
      onOk={() => onSubmit(selectedTagIds)}
      destroyOnHidden
      forceRender
    >
      <Flex vertical gap={16}>
        <Form layout="vertical">
          <Form.Item label={t('auto.new_tag_name')} validateStatus={errorText ? 'error' : ''} help={errorText}>
            <Space.Compact style={{ width: '100%' }}>
              <Input
                aria-label={t('auto.new_tag_name')}
                value={newTagName}
                onChange={(event) => setNewTagName(event.target.value)}
              />
              <Button loading={creating} onClick={() => void handleCreateTag()}>
                {t('auto.create_tag')}</Button>
            </Space.Compact>
          </Form.Item>
        </Form>

        {catalogTags.length === 0 ? (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={t('auto.no_optional_tags')} />
        ) : (
          <Flex vertical gap={12}>
            <Typography.Text type="secondary">{t('auto.tag_write_back_notice')}</Typography.Text>
            <Flex wrap gap={12}>
              {catalogTags.map((tag) => (
                <Checkbox
                  key={tag.id}
                  aria-label={tag.name}
                  checked={selectedTagIds.includes(tag.id)}
                  onChange={(event) => {
                    setSelectedTagIds((current) =>
                      event.target.checked
                        ? [...current, tag.id]
                        : current.filter((item) => item !== tag.id)
                    );
                  }}
                >
                  <Space size={6}>
                    <span>{tag.name}</span>
                    <Tag variant="filled">{tag.application_count}</Tag>
                  </Space>
                </Checkbox>
              ))}
            </Flex>
          </Flex>
        )}
      </Flex>
    </Modal>
  );
}
