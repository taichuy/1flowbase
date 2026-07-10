import type { RestrictedBlockLoaderLimits } from '../../lib/restricted-block-loader';

export const DESIGN_MODE_PERMISSION = 'frontstage.page.design';
export const DEFAULT_JS_BLOCK_TRIAL_LIMITS: RestrictedBlockLoaderLimits = {
  timeoutMs: 1000,
  maxRenderDepth: 8,
  maxRenderNodes: 250,
  maxEventChainDepth: 4,
  allowedActions: [],
  allowedEvents: [],
  allowedQueries: [],
  allowedDataOperations: []
};
