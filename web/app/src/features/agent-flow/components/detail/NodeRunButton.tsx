import { CaretRightOutlined } from '@ant-design/icons';
import { Button, Tooltip } from 'antd';
import { i18nText } from '../../../../shared/i18n/text';

export function NodeRunButton({
  disabled = false,
  onRunNode,
  loading = false
}: {
  disabled?: boolean;
  onRunNode?: (() => void) | undefined;
  loading?: boolean;
}) {
  return (
    <Tooltip title={i18nText('agentFlow', 'auto.trial_run')}>
      <Button
        aria-label={i18nText('agentFlow', 'auto.run_current_node')}
        disabled={!onRunNode || disabled || loading}
        icon={<CaretRightOutlined />}
        loading={loading}
        type="text"
        onClick={() => onRunNode?.()}
      />
    </Tooltip>
  );
}
