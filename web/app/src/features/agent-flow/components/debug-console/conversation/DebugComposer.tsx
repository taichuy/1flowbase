import { ArrowUpOutlined, CloseCircleOutlined } from '@ant-design/icons';
import { Button, Input } from 'antd';
import { useState, type ReactNode } from 'react';
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
  const [isComposing, setIsComposing] = useState(false);
  const showStop = submitting || stopping;

  function handleSubmit() {
    if (disabled || submitting || stopping) {
      return;
    }

    onSubmit(value);
    onChange('');
  }

  return (
    <div className="agent-flow-editor__debug-composer">
      <div
        className={[
          'agent-flow-editor__debug-composer-box',
          footerActions && 'agent-flow-editor__debug-composer-box--with-footer'
        ]
          .filter(Boolean)
          .join(' ')}
      >
        <Input.TextArea
          autoSize={{ minRows: 1, maxRows: 4 }}
          variant="borderless"
          placeholder={i18nText('agentFlow', 'auto.chat_with_bots')}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onCompositionStart={() => setIsComposing(true)}
          onCompositionEnd={() => setIsComposing(false)}
          onKeyDown={(event) => {
            // 中文输入法组合态期间不能把 Enter 误判成发送。
            if (
              event.key !== 'Enter' ||
              event.shiftKey ||
              isComposing ||
              event.nativeEvent.isComposing
            ) {
              return;
            }

            event.preventDefault();

            handleSubmit();
          }}
        />
        <div className="agent-flow-editor__debug-composer-footer">
          {footerActions ? (
            <div className="agent-flow-editor__debug-composer-footer-actions">
              {footerActions}
            </div>
          ) : null}
          <div className="agent-flow-editor__debug-composer-actions">
          {showStop ? (
            <Button
              aria-label={
                stopping
                  ? i18nText('agentFlow', 'auto.terminating_debug_run')
                  : i18nText('agentFlow', 'auto.terminate_debugging_run')
              }
              className="agent-flow-editor__debug-composer-submit agent-flow-editor__debug-composer-stop"
              disabled={stopping}
              icon={<CloseCircleOutlined />}
              loading={stopping}
              shape="circle"
              onClick={onStop}
            />
          ) : (
            <Button
              aria-label={i18nText('agentFlow', 'auto.send_debug_message')}
              className="agent-flow-editor__debug-composer-submit"
              disabled={disabled}
              icon={<ArrowUpOutlined />}
              shape="circle"
              type="primary"
              onClick={handleSubmit}
            />
          )}
          </div>
        </div>
      </div>
    </div>
  );
}
