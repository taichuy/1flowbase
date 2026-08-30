import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';

export function ConnectorAddIcon({ className }: { className?: string }) {
  return (
    <PlusOutlined
      aria-hidden="true"
      className={`agent-flow-connector-add-icon${className ? ` ${className}` : ''}`}
      data-testid="agent-flow-connector-add-icon"
    />
  );
}
