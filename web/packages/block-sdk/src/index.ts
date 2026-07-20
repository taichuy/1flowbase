import type {
  BlockContext,
  BlockContextRecord,
  BlockUiSchema
} from '@1flowbase/page-protocol';

export type { BlockContext } from '@1flowbase/page-protocol';

export interface BlockResult<
  TOutputs extends BlockContextRecord = BlockContextRecord
> {
  view: BlockUiSchema;
  outputs: TOutputs;
}

export type BlockMain<
  TInputs extends BlockContextRecord = BlockContextRecord,
  TOutputs extends BlockContextRecord = BlockContextRecord
> = (
  ctx: BlockContext<TInputs>
) => BlockResult<TOutputs> | Promise<BlockResult<TOutputs>>;

export interface BlockModule<
  TInputs extends BlockContextRecord = BlockContextRecord,
  TOutputs extends BlockContextRecord = BlockContextRecord
> {
  readonly main: BlockMain<TInputs, TOutputs>;
}

export function isBlockModule(value: unknown): value is BlockModule {
  if (!isPlainRecord(value)) {
    return false;
  }

  const keys = Reflect.ownKeys(value);
  if (keys.length !== 1 || keys[0] !== 'main') {
    return false;
  }

  const descriptor = Object.getOwnPropertyDescriptor(value, 'main');
  return Boolean(
    descriptor &&
    'value' in descriptor &&
    typeof descriptor.value === 'function'
  );
}

export function isBlockResult(value: unknown): value is BlockResult {
  if (!isPlainRecord(value)) {
    return false;
  }

  const view = Object.getOwnPropertyDescriptor(value, 'view');
  const outputs = Object.getOwnPropertyDescriptor(value, 'outputs');
  return Boolean(
    view &&
    'value' in view &&
    outputs &&
    'value' in outputs &&
    isPlainRecord(outputs.value)
  );
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }

  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
