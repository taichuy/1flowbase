import { queryOptions } from '@tanstack/react-query';
import {
  getCommunicationBinding,
  getCommunicationOptions,
  fetchConsoleFileTables
} from '@1flowbase/api-client';
export {
  getCommunicationBinding,
  putCommunicationBinding,
  listCommunicationConversations,
  listCommunicationMessages,
  listCommunicationFiles,
  retryCommunicationMessage,
  getCommunicationFile,
  listCommunicationConversationOverview,
  listCommunicationMessageOverview,
  listCommunicationChatMessages,
  getCommunicationMessageDetail
} from '@1flowbase/api-client';
export const communicationBindingOptions = (scope: string, id?: string) =>
  queryOptions({
    queryKey: ['communications', scope, 'binding', id],
    queryFn: ({ signal }) => getCommunicationBinding(id!, signal),
    enabled: !!id
  });
export const communicationOptions = (scope: string, application_id?: string) =>
  queryOptions({
    queryKey: ['communications', scope, 'options', application_id],
    queryFn: ({ signal }) => getCommunicationOptions(application_id, signal)
  });
export const communicationFileTablesOptions = (scope: string) =>
  queryOptions({
    queryKey: ['communications', scope, 'file-tables'],
    queryFn: () => fetchConsoleFileTables()
  });
