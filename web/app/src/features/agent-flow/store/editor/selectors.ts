import type { FlowEditorState } from '../../../flow-editor/store';

export const selectWorkingDocument = (state: FlowEditorState) =>
  state.workingDocument;

export const selectLastSavedDocument = (state: FlowEditorState) =>
  state.lastSavedDocument;

export const selectSelectedNodeId = (state: FlowEditorState) =>
  state.selectedNodeId;

export const selectActiveContainerId = (state: FlowEditorState) =>
  state.activeContainerPath.at(-1) ?? null;

export const selectAutosaveStatus = (state: FlowEditorState) =>
  state.autosaveStatus;

export const selectVersions = (state: FlowEditorState) => state.versions;

export const selectUserProtectionLimit = (state: FlowEditorState) =>
  state.userProtectionLimit;
