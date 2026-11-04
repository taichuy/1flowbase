import { Tooltip } from 'antd';
import { i18nText } from '../../../../../../shared/i18n/text';
import { DebugComposer } from '../DebugComposer';
import './read-only-conversation-input.css';

const noAction = () => {};

/** History viewers use the same composer as the assistant, without editing or sending. */
export function ReadOnlyConversationInput({
  model,
  reasoning_effort
}: {
  model?: string | null;
  reasoning_effort?: string | null;
}) {
  return (
    <div className="application-run-detail__model-summary">
      <DebugComposer
        ariaLabel={i18nText('agentFlow', 'auto.read_only_conversation_input')}
        disabled
        submitting={false}
        stopping={false}
        value=""
        footerActions={
          <div className="read-only-conversation-input__metadata">
            <Tooltip title={i18nText('applications', 'auto.requested_model')}>
              <span className="application-run-detail__model-name">
                {model || '—'}
              </span>
            </Tooltip>
            <Tooltip title={i18nText('applications', 'auto.reasoning_effort')}>
              <span className="application-run-detail__reasoning-effort">
                {reasoning_effort || '—'}
              </span>
            </Tooltip>
          </div>
        }
        onChange={noAction}
        onStop={noAction}
        onSubmit={noAction}
      />
    </div>
  );
}
