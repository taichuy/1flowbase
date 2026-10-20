import {
  setupEmbeddedAssistantFixture,
  renderSettled,
  domEvents,
  attachConsoleAssistantRunWebSocket,
  cancelConsoleFlowRun,
  createConsoleAssistantConversation,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantLegacySnapshotMessages,
  getConsoleAssistantRunActivity,
  getConsoleAssistantSettings,
  listConsoleAssistantConversations,
  startConsoleAssistantRunWebSocket,
  startConsoleAssistantRunStream,
  subscribeConsoleAssistantConversationsWebSocket,
  ASSISTANT_WINDOW_SIZE_STORAGE_KEY,
  innerWidthSpy,
  findReadyHistoryButton
} from './assistant-fixture';
import { act, screen, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import { AssistantRunTimeline } from '../../components/embedded-assistant/AssistantRunActivityPanel';
import * as runtimeApi from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';

setupEmbeddedAssistantFixture();

export {
  ASSISTANT_WINDOW_SIZE_STORAGE_KEY,
  AppProviders,
  AssistantRunTimeline,
  EmbeddedAgentAssistant,
  act,
  attachConsoleAssistantRunWebSocket,
  cancelConsoleFlowRun,
  createConsoleAssistantConversation,
  domEvents,
  expect,
  findReadyHistoryButton,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantLegacySnapshotMessages,
  getConsoleAssistantRunActivity,
  getConsoleAssistantSettings,
  i18nText,
  innerWidthSpy,
  listConsoleAssistantConversations,
  renderSettled,
  runtimeApi,
  screen,
  startConsoleAssistantRunStream,
  startConsoleAssistantRunWebSocket,
  subscribeConsoleAssistantConversationsWebSocket,
  test,
  vi,
  waitFor
};
