import { Skeleton } from 'antd';

export function BlockUiLoadingShell() {
  return (
    <div
      aria-busy="true"
      data-testid="block-ui-loading-shell"
      style={{ width: '100%' }}
    >
      <Skeleton
        active
        title={{ width: '32%' }}
        paragraph={{ rows: 3, width: ['94%', '78%', '58%'] }}
      />
    </div>
  );
}
