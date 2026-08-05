import { Sender } from '@ant-design/x';
import { type ReactNode } from 'react';
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
        autoSize={{ minRows: 1, maxRows: 4 }}
        disabled={disabled}
        footer={(_, { components: { LoadingButton, SendButton } }) => (
          <div className="agent-flow-editor__debug-composer-footer">
            {footerActions ? (
              <div className="agent-flow-editor__debug-composer-footer-actions">
                {footerActions}
              </div>
            ) : null}
            <div className="agent-flow-editor__debug-composer-actions">
              {showStop ? (
                <LoadingButton
                  aria-label={
                    stopping
                      ? i18nText('agentFlow', 'auto.terminating_debug_run')
                      : i18nText('agentFlow', 'auto.terminate_debugging_run')
                  }
                  className="agent-flow-editor__debug-composer-submit agent-flow-editor__debug-composer-stop"
                  disabled={stopping}
                  loading={stopping}
                />
              ) : (
                <SendButton
                  aria-label={i18nText('agentFlow', 'auto.send_debug_message')}
                  className="agent-flow-editor__debug-composer-submit"
                />
              )}
            </div>
          </div>
        )}
        loading={showStop}
        placeholder={i18nText('agentFlow', 'auto.chat_with_bots')}
        rootClassName="agent-flow-editor__debug-composer-sender"
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
