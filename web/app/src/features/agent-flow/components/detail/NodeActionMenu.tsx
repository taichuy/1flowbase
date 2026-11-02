import MoreOutlined from '@ant-design/icons/es/icons/MoreOutlined';
import { Button, Dropdown } from 'antd';
import { i18nText } from '../../../../shared/i18n/text';

export function NodeActionMenu({
  readOnly = false,
  onLocate,
  onCopy,
  onDelete
}: {
  readOnly?: boolean;
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
            disabled: readOnly,
            label: i18nText('agentFlow', 'auto.copy_node'),
            onClick: onCopy
          },
          {
            key: 'delete',
            disabled: readOnly,
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
