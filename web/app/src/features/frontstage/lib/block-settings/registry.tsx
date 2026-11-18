import { useState, type ReactNode } from 'react';
import {
  BlockSettingsContext,
  createBlockSettingsRegistry
} from './registry-store';

/** Each canvas slot owns its configuration menu; registrations never cross blocks. */
export function BlockSettingsProvider({ children }: { children: ReactNode }) {
  const [registry] = useState(createBlockSettingsRegistry);
  return (
    <BlockSettingsContext.Provider value={registry}>
      {children}
    </BlockSettingsContext.Provider>
  );
}
