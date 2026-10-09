import type {
  BlockContext,
  BlockContextRecord
} from '@1flowbase/page-protocol';

export type {
  BlockBinaryInput,
  BlockBinaryResource,
  BlockContextAssets,
  BlockContext,
  BlockContextRecord,
  BlockExternalAssetHandle,
  BlockContextOutputPublishResult,
  BlockContextOutputs,
  BlockContextI18n,
  BlockTranslationOptions
} from '@1flowbase/page-protocol';

export interface BlockComponentProps<
  TInputs extends BlockContextRecord = BlockContextRecord,
  TOutputs extends BlockContextRecord = BlockContextRecord
> {
  readonly ctx: BlockContext<TInputs, TOutputs>;
}

export const blockSdkVersion = '1.0.0';
