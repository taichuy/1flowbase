import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import { App, Button, Tooltip } from 'antd';
import { copyTextToClipboard } from '../../../../../../shared/ui/clipboard/copy-text';
import { i18nText } from '../../../../../../shared/i18n/text';

export function MessageCopyButton({
  text,
  label = i18nText('agentFlow', 'auto.copy')
}: {
  text: string;
  label?: string;
}) {
  const { message } = App.useApp();

  async function handleCopy() {
    try {
      await copyTextToClipboard(text);
      message.success(i18nText('agentFlow', 'auto.copied'));
    } catch {
      message.error(i18nText('agentFlow', 'auto.copy_failed'));
    }
  }

  return (
    <Tooltip title={label}>
      <Button
        aria-label={label}
        disabled={!text}
        icon={<CopyOutlined />}
        size="small"
        onClick={() => {
          void handleCopy();
        }}
      />
    </Tooltip>
  );
}
