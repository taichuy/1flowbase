import { useMutation, useQueryClient } from '@tanstack/react-query';
import type { RefObject } from 'react';
import type {
  ReplaceConsoleWorkflowEventTriggerInput,
  ConsoleApplicationType,
  ConsoleWorkflowExtensionHttpMethod,
  ConsoleWorkflowExtensionResponseMode,
  ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';
import {
  saveWorkflowEventTrigger,
  workflowEventTriggerQueryKey,
  applicationDetailQueryKey,
  applicationsQueryKey,
  createApplication,
  updateApplication
} from '../../api/applications';
import { saveWorkflowScheduleTrigger } from '../../api/public-api';

export type ApplicationFormIntent =
  | {
      kind: 'create';
      onCreated: (applicationId: string) => void;
    }
  | {
      kind: 'edit';
      applicationId: string;
      onSaved?: () => void;
    };

export interface ApplicationFormValues {
  application_type: ConsoleApplicationType;
  trigger_type: ConsoleWorkflowTriggerType;
  extension_subpath: string;
  extension_http_method: Extract<
    ConsoleWorkflowExtensionHttpMethod,
    'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  >;
  extension_response_mode: ConsoleWorkflowExtensionResponseMode;
  schedule_enabled: boolean;
  schedule_cron: string;
  schedule_timezone?: string;
  schedule_input_payload?: object;
  event: ReplaceConsoleWorkflowEventTriggerInput;
  name: string;
  description: string;
  tag_ids: string[];
}

export function useApplicationFormMutation({
  csrfToken,
  intent,
  isEvent,
  isSchedule,
  pendingCreated,
  onSaved
}: {
  csrfToken: string;
  intent: ApplicationFormIntent;
  isEvent: boolean;
  isSchedule: boolean;
  pendingCreated: RefObject<Awaited<
    ReturnType<typeof createApplication>
  > | null>;
  onSaved: () => void;
}) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (values: ApplicationFormValues) => {
      // Ant Form collects registered fields only. With no Start inputs there are no
      // mapping controls, but the event request still owns an explicit empty mapping.
      const event = {
        ...values.event,
        input_mapping: values.event?.input_mapping ?? {}
      };
      if (intent.kind === 'create') {
        const workflow = values.application_type === 'workflow';
        const created =
          pendingCreated.current ??
          (await createApplication(
            {
              application_type: values.application_type,
              workflow_trigger_type: workflow ? values.trigger_type : null,
              workflow_trigger_config: workflow
                ? values.trigger_type === 'schedule'
                  ? {
                      cron: values.schedule_cron,
                      timezone: values.schedule_timezone ?? '',
                      input_payload: {}
                    }
                  : values.trigger_type === 'extension'
                    ? {
                        subpath: values.extension_subpath,
                        http_method: values.extension_http_method,
                        response_mode: values.extension_response_mode
                      }
                    : null
                : null,
              name: values.name,
              description: values.description,
              icon: null,
              icon_type: null,
              icon_background: null
            },
            csrfToken
          ));
        pendingCreated.current = created;
        if (workflow && values.trigger_type === 'event')
          await saveWorkflowEventTrigger(created.id, event, csrfToken);
        if (values.tag_ids.length > 0) {
          await updateApplication(
            created.id,
            {
              name: created.name,
              description: created.description,
              tag_ids: values.tag_ids
            },
            csrfToken
          );
        }
        return created.id;
      }

      await updateApplication(
        intent.applicationId,
        {
          name: values.name,
          description: values.description,
          tag_ids: values.tag_ids
        },
        csrfToken
      );
      if (isEvent)
        await saveWorkflowEventTrigger(intent.applicationId, event, csrfToken);
      if (isSchedule) {
        await saveWorkflowScheduleTrigger(
          intent.applicationId,
          {
            enabled: values.schedule_enabled,
            cron: values.schedule_cron.trim(),
            timezone: values.schedule_timezone?.trim() ?? '',
            input_payload: values.schedule_input_payload ?? {}
          },
          csrfToken
        );
      }
      return intent.applicationId;
    },
    onSuccess: async (savedApplicationId) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: applicationsQueryKey }),
        queryClient.invalidateQueries({
          queryKey: applicationDetailQueryKey(savedApplicationId)
        }),
        queryClient.invalidateQueries({
          queryKey: workflowEventTriggerQueryKey(savedApplicationId)
        })
      ]);
      onSaved();
      if (intent.kind === 'create') intent.onCreated(savedApplicationId);
      else intent.onSaved?.();
    }
  });
}
