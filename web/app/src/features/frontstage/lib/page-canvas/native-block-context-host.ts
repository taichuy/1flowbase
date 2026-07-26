import type {
  BlockContextOutputs,
  BlockRuntimeDiagnostic
} from '@1flowbase/page-protocol';
import {
  createNativeBlockContextCapabilities,
  type JsBlockHostEffectHandler,
  type JsBlockHostInterfaceEffect,
  type NativeBlockContextApiCallObservation,
  type NativeBlockContextEventInput
} from '@1flowbase/page-runtime';

export interface FrontstageNativeBlockContextHost {
  interface: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect>;
  emitEvent?(event: NativeBlockContextEventInput): void;
  observeApiCall?(observation: NativeBlockContextApiCallObservation): void;
  reportDiagnostic?(diagnostic: BlockRuntimeDiagnostic): void;
}

export function createFrontstageNativeBlockContextCapabilities(input: {
  host: FrontstageNativeBlockContextHost;
  pageId: string;
  tabId: string;
  blockId: string;
  instanceEpoch: string;
  isCurrentInstance(): boolean;
  outputs: BlockContextOutputs;
}) {
  return createNativeBlockContextCapabilities({
    requestId: `native:${input.blockId}:${input.instanceEpoch}`,
    instanceEpoch: input.instanceEpoch,
    isCurrentInstance: input.isCurrentInstance,
    interfaceHandler: input.host.interface,
    outputs: input.outputs,
    emitEvent: input.host.emitEvent,
    observeApiCall: input.host.observeApiCall,
    reportDiagnostic: ({ error, capability }) =>
      input.host.reportDiagnostic?.({
        pageId: input.pageId,
        tabId: input.tabId,
        blockId: input.blockId,
        phase: capability === 'events' ? 'event' : 'interface',
        code: error.code,
        message: error.message,
        sourceLocation: error.sourceLocation
      })
  });
}
