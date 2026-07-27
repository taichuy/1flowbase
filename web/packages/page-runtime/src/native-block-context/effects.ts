export type BlockHostApiMethod =
  | 'GET'
  | 'POST'
  | 'PUT'
  | 'PATCH'
  | 'DELETE'
  | 'HEAD'
  | 'OPTIONS';

export interface BlockHostInterfaceEffect {
  type: 'interface';
  requestId: string;
  effectId?: string;
  method: BlockHostApiMethod | string;
  path: string;
  operation?: 'call' | 'stream_open' | 'stream_next' | 'stream_cancel';
  streamId?: string;
  request?: unknown;
}

export type BlockHostEffectHandler<Effect> = (
  effect: Effect
) => unknown | Promise<unknown>;
