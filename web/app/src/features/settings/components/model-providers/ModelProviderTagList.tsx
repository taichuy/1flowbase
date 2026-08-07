import { Tag, Typography } from 'antd';
import { i18nText } from '../../../../shared/i18n/text';

export function ModelProviderTagList({
  modelIds,
  emptyText = i18nText("settings", "auto.not_set")
}: {
  modelIds: string[];
  emptyText?: string;
}) {
  if (modelIds.length === 0) {
    return <Typography.Text type="secondary">{emptyText}</Typography.Text>;
  }

  return (
    <div className="model-provider-panel__model-tag-list">
      {modelIds.map((modelId) => (
        <Tag
          key={modelId}
          variant="filled"
          className="model-provider-panel__model-tag"
        >
          {modelId}
        </Tag>
      ))}
    </div>
  );
}
