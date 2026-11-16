import { ApiClientError } from '@1flowbase/api-client';
import { useEffect, useReducer, useRef } from 'react';
import { useAuthStore } from '../../../../state/auth-store';
import {
  fetchRecordPermissions,
  replaceRecordPermissions,
  type RecordPermissionEditorData,
  type RecordPermissionGrant,
  type RecordPermissionMode
} from '../../api/identity-access/record-permissions';

interface Session {
  data: RecordPermissionEditorData | null;
  mode: RecordPermissionMode;
  grants: RecordPermissionGrant[];
  loading: boolean;
  saving: boolean;
  error: 'failed' | 'conflict' | null;
  reload: number;
}
type Action =
  | { type: 'loaded'; data: RecordPermissionEditorData }
  | { type: 'load-failed' }
  | { type: 'reload' }
  | { type: 'mode'; mode: RecordPermissionMode }
  | { type: 'roles'; roleIds: string[] }
  | { type: 'action'; role_id: string; action_code: string; checked: boolean }
  | { type: 'saving' }
  | { type: 'saved' }
  | { type: 'save-failed'; error: 'failed' | 'conflict' };

function transition(state: Session, action: Action): Session {
  switch (action.type) {
    case 'loaded':
      return {
        ...state,
        data: action.data,
        mode: action.data.mode,
        grants: action.data.grants,
        loading: false
      };
    case 'load-failed':
      return { ...state, loading: false, error: 'failed' };
    case 'reload':
      return {
        ...state,
        data: null,
        loading: true,
        error: null,
        reload: state.reload + 1
      };
    case 'mode':
      return { ...state, mode: action.mode };
    case 'roles':
      return {
        ...state,
        grants: action.roleIds.map(
          (role_id) =>
            state.grants.find((grant) => grant.role_id === role_id) ?? {
              role_id,
              actions: []
            }
        )
      };
    case 'action':
      return {
        ...state,
        grants: state.grants.map((grant) =>
          grant.role_id === action.role_id
            ? {
                ...grant,
                actions: action.checked
                  ? [...grant.actions, action.action_code]
                  : grant.actions.filter((code) => code !== action.action_code)
              }
            : grant
        )
      };
    case 'saving':
      return { ...state, saving: true, error: null };
    case 'saved':
      return { ...state, saving: false };
    case 'save-failed':
      return { ...state, saving: false, error: action.error };
  }
}

/** The keyed editor owns this draft and every asynchronous publication into it. */
export function useRecordPermissionSession({
  pluginCode,
  resourceCode,
  recordId,
  onSaved
}: {
  pluginCode: string;
  resourceCode: string;
  recordId: string;
  onSaved: () => void;
}) {
  const [state, dispatch] = useReducer(transition, {
    data: null,
    mode: 'inherit',
    grants: [],
    loading: true,
    saving: false,
    error: null,
    reload: 0
  });
  const request = useRef<AbortController | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    request.current = controller;
    fetchRecordPermissions(
      {
        plugin_code: pluginCode,
        resource_code: resourceCode,
        record_id: recordId
      },
      undefined,
      controller.signal
    )
      .then((data) => {
        if (!controller.signal.aborted) dispatch({ type: 'loaded', data });
      })
      .catch(() => {
        if (!controller.signal.aborted) dispatch({ type: 'load-failed' });
      });
    // Aborting publication does not cancel or roll back an accepted backend write.
    return () => controller.abort();
  }, [pluginCode, resourceCode, recordId, state.reload]);

  async function save() {
    const controller = request.current;
    if (
      !controller ||
      controller.signal.aborted ||
      !state.data ||
      state.loading ||
      state.saving ||
      state.error === 'conflict'
    )
      return;
    dispatch({ type: 'saving' });
    try {
      await replaceRecordPermissions(
        {
          plugin_code: pluginCode,
          resource_code: resourceCode,
          record_id: recordId
        },
        {
          mode: state.mode,
          grants: state.mode === 'inherit' ? [] : state.grants,
          expected_revision: state.data.revision
        },
        useAuthStore.getState().csrfToken
      );
      if (controller.signal.aborted) return;
      dispatch({ type: 'saved' });
      onSaved();
    } catch (cause) {
      if (controller.signal.aborted) return;
      dispatch({
        type: 'save-failed',
        error:
          cause instanceof ApiClientError && cause.status === 409
            ? 'conflict'
            : 'failed'
      });
    }
  }

  return {
    ...state,
    save,
    reload: () => dispatch({ type: 'reload' }),
    setMode: (mode: RecordPermissionMode) => dispatch({ type: 'mode', mode }),
    selectRoles: (roleIds: string[]) => dispatch({ type: 'roles', roleIds }),
    toggleAction: (role_id: string, action_code: string, checked: boolean) =>
      dispatch({ type: 'action', role_id, action_code, checked })
  };
}
