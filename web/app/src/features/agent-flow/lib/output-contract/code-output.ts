import type { FlowNodeOutputDocument } from '@1flowbase/flow-schema';

export function codeOutputSelector(key: string) {
  return ['result', key];
}

export function normalizeCodeOutput(
  output: FlowNodeOutputDocument
): FlowNodeOutputDocument {
  return {
    ...output,
    title: output.key,
    selector: output.selector ?? codeOutputSelector(output.key)
  };
}
