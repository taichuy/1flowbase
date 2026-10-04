import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode
} from 'react';

interface ChartResource {
  dispose(): void;
}
interface ChartResourceScope {
  register(resource: ChartResource): () => void;
}
const ChartResourceContext = createContext<ChartResourceScope | null>(null);

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

export function useEChartResourceScope() {
  return useContext(ChartResourceContext);
}
