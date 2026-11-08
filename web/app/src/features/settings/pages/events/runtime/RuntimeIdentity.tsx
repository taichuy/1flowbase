import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import { App, Button, Tooltip } from 'antd';
import { useTranslation } from 'react-i18next';
import { useClipboardCopy } from '../../../../../shared/ui/clipboard/use-clipboard-copy';

export function RuntimeIdentity({
  value,
  label,
  full = false
}: {
  value: string | null | undefined;
  label: string;
  full?: boolean;
}) {
  const { t } = useTranslation('settingsEvents');
  const { message } = App.useApp();
  const { copy, copied } = useClipboardCopy();
  if (value == null || value === '') return <>—</>;
  return (
    <span
      className={`event-runtime-identity${full ? ' event-runtime-identity--full' : ''}`}
    >
      <Tooltip title={full ? undefined : value}>
        <span className="event-runtime-identity__value">{value}</span>
      </Tooltip>
      <Button
        type="text"
        size="small"
        aria-label={t('runtime_copy', { label })}
        title={t('runtime_copy', { label })}
        icon={copied ? <CheckOutlined /> : <CopyOutlined />}
        onClick={(event) => {
          event.stopPropagation();
          void copy(value).catch(() => message.error(t('runtime_copy_failed')));
        }}
      />
      <span className="event-runtime-visually-hidden" role="status">
        {copied ? t('runtime_copied') : ''}
      </span>
    </span>
  );
}
