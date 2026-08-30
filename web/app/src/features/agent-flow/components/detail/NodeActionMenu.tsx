import MoreOutlined from '@ant-design/icons/es/icons/MoreOutlined';
import { Button, Dropdown } from 'antd';
import { i18nText } from '../../../../shared/i18n/text';

export function NodeActionMenu({
  onLocate,
  onCopy,
  onDelete
}: {
  onLocate: () => void;
  onCopy: () => void;
  onDelete: () => void;
}) {
  return (
    <Dropdown
      trigger={['click']}
      menu={{
        items: [
          {
            key: 'locate',
            label: i18nText('agentFlow', 'auto.locate_node'),
            onClick: onLocate
          },
          {
            key: 'copy',
            label: i18nText('agentFlow', 'auto.copy_node'),
            onClick: onCopy
          },
          {
            key: 'delete',
            label: i18nText('agentFlow', 'auto.delete_node'),
            danger: true,
            onClick: onDelete
          }
        ]
      }}
    >
      <Button
        aria-label={i18nText('agentFlow', 'auto.more_actions_alt')}
        icon={<MoreOutlined />}
        type="text"
      />
    </Dropdown>
  );
}
