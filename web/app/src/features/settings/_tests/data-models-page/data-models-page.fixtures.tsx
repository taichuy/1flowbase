import { afterEach, beforeEach } from 'vitest';

import { cleanupDataModelsPageTest, setupDataModelsPageTest } from './support';

beforeEach(setupDataModelsPageTest);
afterEach(cleanupDataModelsPageTest);
