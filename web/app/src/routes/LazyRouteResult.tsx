import { lazy, Suspense } from 'react';
import type { ResultProps } from 'antd';

const LazyAntdResult = lazy(() => import('antd/es/result'));

export function LazyRouteResult(props: ResultProps) {
  return (
    <Suspense fallback={null}>
      <LazyAntdResult {...props} />
    </Suspense>
  );
}
