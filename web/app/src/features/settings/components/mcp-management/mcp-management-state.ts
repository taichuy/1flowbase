import type {
  ConsoleMcpInstance,
  ConsoleMcpTool,
  ConsoleMcpToolBinding
} from '@1flowbase/api-client';
import type { SetStateAction } from 'react';

export type McpDirectoryEditorMode = 'group' | 'binding';

export interface McpInstancesState {
  editingInstance: ConsoleMcpInstance | null;
  editingBinding: ConsoleMcpToolBinding | null;
  instanceModalOpen: boolean;
  directoryModalOpen: boolean;
  directoryEditorMode: McpDirectoryEditorMode;
  requestedInstanceId: string;
  selectedDirectoryKey: string;
}

export type McpInstancesAction =
  | {
      type: 'setEditingInstance';
      value: SetStateAction<ConsoleMcpInstance | null>;
    }
  | {
      type: 'setEditingBinding';
      value: SetStateAction<ConsoleMcpToolBinding | null>;
    }
  | { type: 'setInstanceModalOpen'; value: SetStateAction<boolean> }
  | { type: 'setDirectoryModalOpen'; value: SetStateAction<boolean> }
  | {
      type: 'setDirectoryEditorMode';
      value: SetStateAction<McpDirectoryEditorMode>;
    }
  | { type: 'setRequestedInstanceId'; value: SetStateAction<string> }
  | { type: 'setSelectedDirectoryKey'; value: SetStateAction<string> };

export function createInitialMcpInstancesState(
  requestedInstanceId: string
): McpInstancesState {
  return {
    editingInstance: null,
    editingBinding: null,
    instanceModalOpen: false,
    directoryModalOpen: false,
    directoryEditorMode: 'group',
    requestedInstanceId,
    selectedDirectoryKey: ''
  };
}

export interface McpToolsState {
  modalOpen: boolean;
  editingTool: ConsoleMcpTool | null;
  step: string;
  keyword: string;
  executionTargetKind: string | undefined;
  interfaceId: string | undefined;
  riskLevel: string | undefined;
  status: string | undefined;
  desIdRequired: boolean | undefined;
  exportingCatalog: boolean;
}

export type McpToolsAction =
  | { type: 'setModalOpen'; value: SetStateAction<boolean> }
  | {
      type: 'setEditingTool';
      value: SetStateAction<ConsoleMcpTool | null>;
    }
  | { type: 'setStep'; value: SetStateAction<string> }
  | { type: 'setKeyword'; value: SetStateAction<string> }
  | {
      type: 'setExecutionTargetKind';
      value: SetStateAction<string | undefined>;
    }
  | {
      type: 'setInterfaceId';
      value: SetStateAction<string | undefined>;
    }
  | { type: 'setRiskLevel'; value: SetStateAction<string | undefined> }
  | { type: 'setStatus'; value: SetStateAction<string | undefined> }
  | {
      type: 'setDesIdRequired';
      value: SetStateAction<boolean | undefined>;
    }
  | { type: 'setExportingCatalog'; value: SetStateAction<boolean> };

export const initialMcpToolsState: McpToolsState = {
  modalOpen: false,
  editingTool: null,
  step: 'basic',
  keyword: '',
  executionTargetKind: undefined,
  interfaceId: undefined,
  riskLevel: undefined,
  status: undefined,
  desIdRequired: undefined,
  exportingCatalog: false
};

function resolveSetState<T>(value: SetStateAction<T>, current: T): T {
  return typeof value === 'function'
    ? (value as (previous: T) => T)(current)
    : value;
}

export function mcpInstancesReducer(
  state: McpInstancesState,
  action: McpInstancesAction
): McpInstancesState {
  switch (action.type) {
    case 'setEditingInstance':
      return {
        ...state,
        editingInstance: resolveSetState(action.value, state.editingInstance)
      };
    case 'setEditingBinding':
      return {
        ...state,
        editingBinding: resolveSetState(action.value, state.editingBinding)
      };
    case 'setInstanceModalOpen':
      return {
        ...state,
        instanceModalOpen: resolveSetState(
          action.value,
          state.instanceModalOpen
        )
      };
    case 'setDirectoryModalOpen':
      return {
        ...state,
        directoryModalOpen: resolveSetState(
          action.value,
          state.directoryModalOpen
        )
      };
    case 'setDirectoryEditorMode':
      return {
        ...state,
        directoryEditorMode: resolveSetState(
          action.value,
          state.directoryEditorMode
        )
      };
    case 'setRequestedInstanceId':
      return {
        ...state,
        requestedInstanceId: resolveSetState(
          action.value,
          state.requestedInstanceId
        )
      };
    case 'setSelectedDirectoryKey':
      return {
        ...state,
        selectedDirectoryKey: resolveSetState(
          action.value,
          state.selectedDirectoryKey
        )
      };
  }
}

export function mcpToolsReducer(
  state: McpToolsState,
  action: McpToolsAction
): McpToolsState {
  switch (action.type) {
    case 'setModalOpen':
      return {
        ...state,
        modalOpen: resolveSetState(action.value, state.modalOpen)
      };
    case 'setEditingTool':
      return {
        ...state,
        editingTool: resolveSetState(action.value, state.editingTool)
      };
    case 'setStep':
      return { ...state, step: resolveSetState(action.value, state.step) };
    case 'setKeyword':
      return {
        ...state,
        keyword: resolveSetState(action.value, state.keyword)
      };
    case 'setExecutionTargetKind':
      return {
        ...state,
        executionTargetKind: resolveSetState(
          action.value,
          state.executionTargetKind
        )
      };
    case 'setInterfaceId':
      return {
        ...state,
        interfaceId: resolveSetState(action.value, state.interfaceId)
      };
    case 'setRiskLevel':
      return {
        ...state,
        riskLevel: resolveSetState(action.value, state.riskLevel)
      };
    case 'setStatus':
      return { ...state, status: resolveSetState(action.value, state.status) };
    case 'setDesIdRequired':
      return {
        ...state,
        desIdRequired: resolveSetState(action.value, state.desIdRequired)
      };
    case 'setExportingCatalog':
      return {
        ...state,
        exportingCatalog: resolveSetState(action.value, state.exportingCatalog)
      };
  }
}
