import Skeleton from 'antd/es/skeleton';

export function BlockUiLoadingShell() {
  return (
    <div
      aria-busy="true"
      data-testid="block-ui-loading-shell"
      style={{ width: '100%', padding: '8px 4px', opacity: 0.85 }}
    >
      <Skeleton
        title={{ width: '30%' }}
        paragraph={{ rows: 4, width: ['92%', '80%', '64%', '44%'] }}
      />
    </div>
  );
}
