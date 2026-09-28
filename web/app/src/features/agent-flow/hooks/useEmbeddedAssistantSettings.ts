import { useCallback, useEffect, useRef, useState } from 'react';
import {
  getConsoleAssistantSettings,
  type ConsoleAssistantSettings
} from '@1flowbase/api-client';

export function useEmbeddedAssistantSettings({
  open,
  workspaceId,
  saving
}: {
  open: boolean;
  workspaceId: string | undefined;
  saving: boolean;
}) {
  const [settings, updateSettings] = useState<ConsoleAssistantSettings | null>(
    null
  );
  const [refreshError, setRefreshError] = useState(false);
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
      const requestGeneration = ++generation.current;
      try {
        const next = await getConsoleAssistantSettings();
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
      }
    };
    void refresh();
    const onFocus = () => void refresh();
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onFocus);
    return () => {
      disposed = true;
      generation.current += 1;
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onFocus);
    };
  }, [open, workspaceId]);

  return { settings, setSettings, invalidateSettingsRead, refreshError };
}
