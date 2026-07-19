import type { FlowNodeType } from '@1flowbase/flow-schema';

import type { CanvasNodeSchema } from '../../../shared/schema-ui/v1/contracts/canvas-node-schema';
import {
  buildCommonConfigBlocks,
  buildCommonLastRunBlocks,
  buildNodeCardBlocks,
  buildNodeDetailHeaderBlocks,
  buildNodeRuntimeSlots
} from './node-schema-fragments';
import { getNodeDefinitionMeta } from '../lib/node-definitions';

export function resolveAgentFlowNodeSchema(nodeType: FlowNodeType): CanvasNodeSchema {
  const meta = getNodeDefinitionMeta(nodeType);

  return {
    schemaVersion: '1.0.0',
    nodeType,
    capabilities: [
      'help',
      'locate',
      'duplicate',
      ...(meta?.canEnterContainer ? ['enter_container'] : [])
    ],
    card: {
      blocks: buildNodeCardBlocks(nodeType)
    },
    detail: {
      header: {
        blocks: buildNodeDetailHeaderBlocks()
      },
      tabs: {
        config: {
          blocks: buildCommonConfigBlocks(nodeType)
        },
        lastRun: {
          blocks: buildCommonLastRunBlocks()
        }
      }
    },
    runtimeSlots: buildNodeRuntimeSlots()
  };
}

