import { useCallback, useEffect, useRef, useState } from 'react';
import {
  getConsoleAssistantSettings,
  updateConsoleAssistantSettings,
  type ConsoleAssistantSettings
} from '@1flowbase/api-client';

export function useEmbeddedAssistantSettings({
  open,
  workspaceId,
  saving,
  csrfToken
}: {
  open: boolean;
  workspaceId: string | undefined;
  saving: boolean;
  csrfToken: string | null;
}) {
  const [settings, updateSettings] = useState<ConsoleAssistantSettings | null>(
    null
  );
  const [refreshError, setRefreshError] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const refreshRef = useRef<() => Promise<void>>(async () => {});
  const refreshSettings = useCallback(() => refreshRef.current(), []);
  const generation = useRef(0);
  const savingRef = useRef(saving);
  savingRef.current = saving;
  const invalidateSettingsRead = useCallback(() => {
    generation.current += 1;
  }, []);
  const setSettings = useCallback((next: ConsoleAssistantSettings | null) => {
    generation.current += 1;
    updateSettings(next);
    setRefreshError(false);
  }, []);

  useEffect(() => {
    setSettings(null);
  }, [workspaceId, setSettings]);

  useEffect(() => {
    if (!open) return;
    let disposed = false;
    let pending = false;
    const refresh = async () => {
      if (
        disposed ||
        pending ||
        savingRef.current ||
        document.visibilityState === 'hidden'
      )
        return;
      pending = true;
      setRefreshing(true);
      const requestGeneration = ++generation.current;
      try {
        let next = await getConsoleAssistantSettings();
        if (disposed || requestGeneration !== generation.current) return;
        // Always expose the fresh catalog, including when preference persistence fails.
        updateSettings(next);
        const { preference, run_capabilities } = next;
        const selected = run_capabilities.model_selection_enabled
          ? run_capabilities.models.find(
              (model) => model.id === preference.model
            )
          : undefined;
        const replacement = run_capabilities.model_selection_enabled
          ? (selected ?? run_capabilities.models[0])
          : undefined;
        const modelChanged = Boolean(preference.model && !selected);
        const effortInvalid = Boolean(
          preference.reasoning_effort &&
          (!run_capabilities.reasoning_effort_enabled ||
            !selected?.reasoning_efforts.includes(preference.reasoning_effort))
        );
        if (modelChanged || effortInvalid) {
          if (!csrfToken)
            throw new Error('assistant preference requires a console session');
          const model = preference.model ? (replacement?.id ?? null) : null;
          const reasoning_effort =
            model && run_capabilities.reasoning_effort_enabled
              ? preference.reasoning_effort &&
                replacement?.reasoning_efforts.includes(
                  preference.reasoning_effort
                )
                ? preference.reasoning_effort
                : (replacement?.default_reasoning_effort ??
                  replacement?.reasoning_efforts[0] ??
                  null)
              : null;
          next = await updateConsoleAssistantSettings(
            {
              ...preference,
              model,
              reasoning_effort
            },
            csrfToken
          );
        }
        if (!disposed && requestGeneration === generation.current) {
          updateSettings(next);
          setRefreshError(false);
        }
      } catch {
        if (!disposed && requestGeneration === generation.current) {
          setRefreshError(true);
        }
      } finally {
        pending = false;
        if (!disposed) setRefreshing(false);
      }
    };
    refreshRef.current = refresh;
    void refresh();
    const onFocus = () => void refresh();
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onFocus);
    return () => {
      disposed = true;
      refreshRef.current = async () => {};
      setRefreshing(false);
      generation.current += 1;
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onFocus);
    };
  }, [open, workspaceId, csrfToken]);

  return {
    settings,
    setSettings,
    invalidateSettingsRead,
    refreshError,
    refreshSettings,
    refreshing
  };
}
