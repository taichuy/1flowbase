export interface SyncSlice {
  autosaveStatus: 'idle' | 'saving' | 'saved' | 'error';
  isRestoringVersion: boolean;
  isDirty: boolean;
  lastChangeKind: 'layout' | 'logical' | null;
  lastChangeSummary: string | null;
}
