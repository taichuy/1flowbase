import { createContext, useContext } from 'react';

export interface ChartResource {
  dispose(): void;
}
export interface ChartResourceScope {
  register(resource: ChartResource): () => void;
}
export const ChartResourceContext = createContext<ChartResourceScope | null>(
  null
);

export function useEChartResourceScope() {
  return useContext(ChartResourceContext);
}
