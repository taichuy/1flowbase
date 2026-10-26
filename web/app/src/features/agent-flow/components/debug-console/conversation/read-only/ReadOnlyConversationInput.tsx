import { Tooltip, theme } from 'antd';
import { i18nText } from '../../../../../../shared/i18n/text';
import './read-only-conversation-input.css';

/** The non-interactive input surface shared by conversation history viewers. */
export function ReadOnlyConversationInput({
  model,
  reasoning_effort
}: {
  model?: string | null;
  reasoning_effort?: string | null;
}) {
  const { token } = theme.useToken();
  return (
    <div
      className="application-run-detail__model-summary"
      style={{
        borderRadius: token.borderRadius * 2,
        borderColor: token.colorBorder,
        boxShadow: token.boxShadowTertiary
      }}
    >
      <textarea
        disabled
        value=""
        aria-label={i18nText('agentFlow', 'auto.read_only_conversation_input')}
        className="read-only-conversation-input__field"
      />
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
    </div>
  );
}
