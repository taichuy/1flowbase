import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

export interface ViewportSlice {
  viewport: FlowAuthoringDocument['editor']['viewport'];
  controlMode: 'pointer' | 'hand';
  isFittingView: boolean;
}
