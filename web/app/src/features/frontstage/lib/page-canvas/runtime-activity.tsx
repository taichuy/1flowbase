import { createContext } from 'react';

/** Session visibility is independent of the lifetime of its native surface. */
export const FrontstageRuntimeActivityContext = createContext(true);
