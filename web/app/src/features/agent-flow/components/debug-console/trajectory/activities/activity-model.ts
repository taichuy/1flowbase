import { createContext } from 'react';

export type ActivityCategory = 'tools' | 'rounds' | 'agents';
// The original detail renderer remains the owner of node I/O and source relations.
export const TraceActivityDetailContext = createContext(false);
