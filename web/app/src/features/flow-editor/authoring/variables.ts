import type { FlowNodeOutputDocument } from '@1flowbase/flow-schema';

export interface AuthoringVariableSource {
  id: string;
  label: string;
  outputs: FlowNodeOutputDocument[];
}

export interface AuthoringVariableOption {
  nodeId: string;
  nodeLabel: string;
  outputKey: string;
  outputLabel: string;
  valueType: string;
  value: string[];
  displayLabel: string;
}
