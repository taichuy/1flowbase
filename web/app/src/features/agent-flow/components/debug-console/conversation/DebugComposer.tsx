import { SendOutlined } from '@ant-design/icons';
import { Sender } from '@ant-design/x';
import { Flex, Tooltip } from 'antd';
import type { ReactNode } from 'react';
import { i18nText } from '../../../../../shared/i18n/text';

export function DebugComposer({
  value,
  disabled,
  submitting,
  stopping,
  footerActions,
  onChange,
  onStop,
  onSubmit
}: {
  value: string;
  disabled: boolean;
  submitting: boolean;
  stopping: boolean;
  footerActions?: ReactNode;
  onChange: (value: string) => void;
  onStop: () => void;
  onSubmit: (value: string) => void;
}) {
  const showStop = submitting || stopping;

  function handleSubmit(message: string) {
    if (disabled || submitting || stopping) {
      return;
    }

    onSubmit(message);
    onChange('');
  }

  return (
    <div className="agent-flow-editor__debug-composer">
      <Sender
        autoSize={{ minRows: 3, maxRows: 6 }}
        disabled={disabled}
        footer={(_, { components: { LoadingButton, SendButton } }) => (
          <Flex align="center" justify="end">
            <Flex align="center" gap="small">
              {footerActions}
              {showStop ? (
                <Tooltip
                  title={
                    stopping
                      ? i18nText('agentFlow', 'auto.terminating_debug_run')
                      : i18nText('agentFlow', 'auto.terminate_debugging_run')
                  }
                >
                  <LoadingButton disabled={stopping} loading={stopping} />
                </Tooltip>
              ) : (
                <Tooltip
                  title={
                    value
                      ? i18nText('agentFlow', 'auto.send_debug_message')
                      : i18nText('agentFlow', 'auto.chat_with_bots')
                  }
                >
                  <SendButton
                    aria-label={i18nText('agentFlow', 'auto.send_debug_message')}
                    className="agent-flow-editor__debug-composer-send"
                    color="primary"
                    icon={<SendOutlined />}
                    shape="default"
                    style={{
                      borderColor: 'transparent',
                      color: 'var(--color-primary)',
                      opacity: 1
                    }}
                    variant="text"
                  />
                </Tooltip>
              )}
            </Flex>
          </Flex>
        )}
        loading={showStop}
        placeholder={i18nText('agentFlow', 'auto.chat_with_bots')}
        submitType="enter"
        suffix={false}
        value={value}
        onCancel={onStop}
        onChange={onChange}
        onSubmit={handleSubmit}
      />
    </div>
  );
}
