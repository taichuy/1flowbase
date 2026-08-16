export type FrontstageAssistantCapability =
  | 'list_page_blocks'
  | 'inspect_block_render'
  | 'search_block_render'
  | 'read_block_render_fragment'
  | 'click_block_element'
  | 'recompile_block';

export interface FrontstageAssistantExecution {
  result: unknown;
  is_error: boolean;
}

export interface FrontstageAssistantRuntime {
  execute(
    capability: FrontstageAssistantCapability,
    arguments_: Record<string, unknown>
  ): Promise<FrontstageAssistantExecution>;
}

let activeRuntime: FrontstageAssistantRuntime | null = null;
const runtimeListeners = new Set<() => void>();

function emitRuntimeChange(): void {
  runtimeListeners.forEach((listener) => listener());
}

export function registerFrontstageAssistantRuntime(
  runtime: FrontstageAssistantRuntime
): () => void {
  activeRuntime = runtime;
  emitRuntimeChange();
  return () => {
    if (activeRuntime !== runtime) return;
    activeRuntime = null;
    emitRuntimeChange();
  };
}

export function getFrontstageAssistantRuntime(): FrontstageAssistantRuntime | null {
  return activeRuntime;
}

export function subscribeFrontstageAssistantRuntime(
  listener: () => void
): () => void {
  runtimeListeners.add(listener);
  return () => runtimeListeners.delete(listener);
}
