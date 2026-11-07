import { useEffect, useRef, useState } from 'react';
import { App } from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstanceDirectory
} from '@1flowbase/api-client';
import {
  fetchSettingsMcpInstanceDirectory,
  replaceSettingsMcpInstanceDirectory,
  settingsMcpCatalogQueryKey
} from '../../../api/mcp-management';
import { i18nText } from '../../../../../shared/i18n/text';
import { notifyWebMcpRegistrationsChanged } from '../../../../webmcp/registration-events';
import {
  directoryPayload,
  reconcileDirectory,
  sameDirectory
} from './directory-draft';

type Session = {
  instanceId: string;
  baseline: ConsoleMcpInstanceDirectory | null;
  current: ConsoleMcpInstanceDirectory | null;
  loading: boolean;
  error: string | null;
};
const empty: Session = {
  instanceId: '',
  baseline: null,
  current: null,
  loading: false,
  error: null
};

export function useDirectoryDraft(csrfToken: string) {
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const [session, setSession] = useState<Session>(empty);
  const current = useRef(session);
  const generation = useRef(0);
  useEffect(
    () => () => {
      ++generation.current;
    },
    []
  );
  function publish(value: Session) {
    current.current = value;
    setSession(value);
  }
  async function load(instanceId: string) {
    const ticket = ++generation.current;
    publish({ ...empty, instanceId, loading: true });
    try {
      const snapshot = await fetchSettingsMcpInstanceDirectory(instanceId);
      if (ticket === generation.current) {
        publish({
          instanceId,
          baseline: snapshot,
          current: snapshot,
          loading: false,
          error: null
        });
        return snapshot;
      }
    } catch (error) {
      if (ticket === generation.current)
        publish({
          ...empty,
          instanceId,
          error: error instanceof Error ? error.message : String(error)
        });
    }
  }
  function update(
    transform: (
      snapshot: ConsoleMcpInstanceDirectory
    ) => ConsoleMcpInstanceDirectory
  ) {
    const state = current.current;
    if (!state.current) return;
    publish({ ...state, current: transform(state.current) });
  }
  function clear() {
    ++generation.current;
    publish(empty);
  }
  const saveDirectoryMutation = useMutation({
    mutationFn: async () => {
      const state = current.current;
      if (!state.baseline || !state.current)
        throw new Error(
          i18nText('settingsMcpManagement', 'auto.directory_not_ready')
        );
      const submitted = state.current;
      const ticket = generation.current;
      const saved = await replaceSettingsMcpInstanceDirectory(
        state.instanceId,
        {
          expected_snapshot: state.baseline,
          ...directoryPayload(submitted)
        },
        csrfToken
      );
      if (ticket !== generation.current) return;
      const latest = current.current;
      publish({
        ...latest,
        baseline: saved,
        current: reconcileDirectory(
          latest.current ?? submitted,
          submitted,
          saved
        ),
        error: null
      });
      queryClient.setQueryData<ConsoleMcpCatalog>(
        settingsMcpCatalogQueryKey,
        (catalog) =>
          catalog
            ? {
                ...catalog,
                groups: [
                  ...catalog.groups.filter(
                    (row) => row.instance_record_id !== saved.instance_record_id
                  ),
                  ...saved.groups
                ],
                bindings: [
                  ...catalog.bindings.filter(
                    (row) => row.instance_record_id !== saved.instance_record_id
                  ),
                  ...saved.bindings
                ]
              }
            : catalog
      );
      notifyWebMcpRegistrationsChanged();
      message.success(i18nText('settings', 'auto.mcp_saved'));
    },
    onError: (error) =>
      message.error(error instanceof Error ? error.message : String(error))
  });
  return {
    snapshot: session.current,
    loading: session.loading,
    error: session.error,
    hasChanges: Boolean(
      session.current &&
      session.baseline &&
      !sameDirectory(session.current, session.baseline)
    ),
    getSnapshot: () => current.current.current,
    load,
    update,
    clear,
    saveDirectoryMutation
  };
}
