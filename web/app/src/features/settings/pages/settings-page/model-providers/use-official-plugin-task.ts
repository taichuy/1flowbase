import { useEffect, useEffectEvent, useState } from 'react';

import { useQuery } from '@tanstack/react-query';

import { fetchSettingsPluginTask } from '../../../api/plugins';
import {
  isTaskSucceeded,
  isTaskTerminal,
  type OfficialInstallState
} from './shared';

export function useOfficialPluginTask({
  onSettled
}: {
  onSettled: (status: 'success' | 'failed') => Promise<void>;
}) {
  const [officialInstallState, setOfficialInstallState] =
    useState<OfficialInstallState>({
      pluginId: null,
      taskId: null,
      status: 'idle'
    });

  const pluginTaskQuery = useQuery({
    queryKey: ['settings', 'plugins', 'task', officialInstallState.taskId],
    queryFn: () => fetchSettingsPluginTask(officialInstallState.taskId!),
    enabled: Boolean(officialInstallState.taskId)
  });

  const refetchOfficialInstallTask = useEffectEvent(() => {
    void pluginTaskQuery.refetch();
  });

  useEffect(() => {
    if (
      !officialInstallState.taskId ||
      pluginTaskQuery.fetchStatus === 'fetching'
    ) {
      return;
    }

    const task = pluginTaskQuery.data;
    if (task?.finished_at || isTaskTerminal(task?.status)) {
      return;
    }

    const timer = window.setTimeout(() => {
      refetchOfficialInstallTask();
    }, 1000);

    return () => {
      window.clearTimeout(timer);
    };
  }, [
    officialInstallState.taskId,
    pluginTaskQuery.data,
    pluginTaskQuery.fetchStatus,
    refetchOfficialInstallTask
  ]);

  useEffect(() => {
    const task = pluginTaskQuery.data;
    if (!task || !officialInstallState.taskId) {
      return;
    }

    if (!task.finished_at && !isTaskTerminal(task.status)) {
      return;
    }

    const status = isTaskSucceeded(task.status) ? 'success' : 'failed';
    setOfficialInstallState((current) => ({
      pluginId: current.pluginId,
      taskId: null,
      status
    }));
    void onSettled(status);
  }, [officialInstallState.taskId, onSettled, pluginTaskQuery.data]);

  return {
    officialInstallState,
    setOfficialInstallState,
    pluginTaskQuery
  };
}
