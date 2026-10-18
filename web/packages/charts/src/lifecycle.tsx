import { useEffect, useState, type ReactNode } from 'react';

import {
  ChartResourceContext,
  type ChartResource,
  type ChartResourceScope
} from './lifecycle-context';

/** Place outside Activity: visibility suspends a chart; its owner releases it. */
export function EChartResourceBoundary({ children }: { children: ReactNode }) {
  const [resources] = useState(() => new Set<ChartResource>());
  const [scope] = useState<ChartResourceScope>(() => ({
    register(resource) {
      resources.add(resource);
      return () => {
        resources.delete(resource);
      };
    }
  }));
  useEffect(
    () => () => {
      for (const resource of [...resources]) resource.dispose();
      resources.clear();
    },
    [resources]
  );
  return <ChartResourceContext value={scope}>{children}</ChartResourceContext>;
}
