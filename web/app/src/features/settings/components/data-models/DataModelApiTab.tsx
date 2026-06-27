import { Descriptions, Flex, Tag } from 'antd';

import type { SettingsDataModel } from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';

export function DataModelApiTab({
  model
}: {
  model: SettingsDataModel;
}) {
  const exposed =
    model.status === 'published' && model.runtime_availability === 'available';
  const exposureStatus = exposed
    ? 'api_exposed_ready'
    : 'published_not_exposed';
  const exposureLabel = exposed
    ? i18nText("settings", "auto.api_exposed_ready")
    : exposureStatus;

  return (
    <Flex vertical gap={16}>
      <Descriptions
        size="small"
        column={1}
        items={[
          {
            key: 'status',
            label: i18nText("settings", "auto.api_exposure_status"),
            children: (
              <Tag color={exposed ? 'green' : 'default'}>
                {exposureLabel}
              </Tag>
            )
          },
          {
            key: 'runtime',
            label: i18nText("settings", "auto.runtime"),
            children: model.runtime_availability
          },
          {
            key: 'namespace',
            label: i18nText("settings", "auto.acl_namespace"),
            children: model.acl_namespace
          }
        ]}
      />
    </Flex>
  );
}
