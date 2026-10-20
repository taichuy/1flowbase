import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

import { i18nText } from '../../../../shared/i18n/text';
import type { SettingsCompatibleDataModelTemplate } from '../../api/data-models';
import { dataModelStatusHelp } from '../../components/data-models/data-model-help-text';
import { DataModelFormDrawer } from '../../components/data-models/DataModelFormDrawer';
import {
  SLOW_SETTINGS_PAGE_TEST_TIMEOUT,
  cleanupDataModelsPageTest,
  contactsModel,
  createAppView,
  dataModelsApi,
  generalTemplateSystemFields,
  openContactsDataModelEditor,
  openDataModelEditorByTitle,
  orderedTreeTemplateSystemFields,
  renderApp,
  setupDataModelsPageTest
} from './support';

beforeEach(setupDataModelsPageTest);
afterEach(cleanupDataModelsPageTest);

export {
  DataModelFormDrawer,
  SLOW_SETTINGS_PAGE_TEST_TIMEOUT,
  type SettingsCompatibleDataModelTemplate,
  act,
  contactsModel,
  createAppView,
  dataModelStatusHelp,
  dataModelsApi,
  expect,
  fireEvent,
  generalTemplateSystemFields,
  i18nText,
  openContactsDataModelEditor,
  openDataModelEditorByTitle,
  orderedTreeTemplateSystemFields,
  render,
  renderApp,
  screen,
  test,
  vi,
  waitFor,
  within
};
