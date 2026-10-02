import { create } from 'zustand';
import type { NavigationDropPosition } from './projection';

type TopbarDrag = {
  workspaceId: string;
  nodeId: string;
  target: {
    nodeId: string;
    position: Exclude<NavigationDropPosition, 'inside'>;
  } | null;
};

// Native dragover protects payload data; the local session supplies the source.
export const useTopbarDragStore = create<{ drag: TopbarDrag | null }>(() => ({
  drag: null
}));
