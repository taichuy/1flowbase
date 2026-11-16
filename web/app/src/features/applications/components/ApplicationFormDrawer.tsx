import { Alert, Button, Flex, Form, Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import { ResizableDrawer } from '../../../shared/ui/resizable-drawer/ResizableDrawer';
import { ApplicationFormFields } from './application-form/ApplicationFormFields';
import {
  useApplicationFormController,
  type ApplicationFormIntent,
  type ApplicationFormValues
} from './application-form/use-application-form-controller';

export type { ApplicationFormIntent } from './application-form/use-application-form-controller';

const APPLICATION_FORM_ID = 'application-form';

export function ApplicationFormDrawer({
  open,
  csrfToken,
  intent,
  onClose
}: {
  open: boolean;
  csrfToken: string;
  intent: ApplicationFormIntent;
  onClose: () => void;
}) {
  const { t } = useTranslation('applications');
  const {
    form,
    isEdit,
    isEvent,
    eventQuery,
    catalogQuery,
    applicationTypeOption,
    workflowTriggerTypeOption,
    showWorkflow,
    triggerType,
    extension,
    extensionContract,
    timezoneOptions,
    mutation,
    loading,
    triggerLoadFailed
  } = useApplicationFormController({ open, csrfToken, intent, onClose });

  return (
    <ResizableDrawer
      open={open}
      title={
        isEdit
          ? t('auto.edit_application_information')
          : t('auto.new_application')
      }
      destroyOnClose
      resizeLabel={t('auto.resize_application_drawer')}
      footer={
        loading ? null : (
          <Flex justify="flex-end">
            <Button
              type="primary"
              htmlType="submit"
              form={APPLICATION_FORM_ID}
              disabled={triggerLoadFailed || (isEvent && eventQuery.isPending)}
              loading={mutation.isPending}
            >
              {isEdit ? t('auto.save_changes') : t('auto.create_application')}
            </Button>
          </Flex>
        )
      }
      onClose={onClose}
    >
      {loading ? (
        <Typography.Text type="secondary">{t('auto.loading')}</Typography.Text>
      ) : (
        <Form<ApplicationFormValues>
          id={APPLICATION_FORM_ID}
          form={form}
          layout="vertical"
          onFinish={(values) => mutation.mutate(values)}
        >
          {mutation.isError ? (
            <Alert
              type="error"
              showIcon
              title={
                isEdit
                  ? t('auto.save_failed')
                  : t('auto.create_application_failed')
              }
              description={formatMutationError(mutation.error)}
            />
          ) : null}

          <ApplicationFormFields
            form={form}
            isEdit={isEdit}
            catalogQuery={catalogQuery}
            applicationTypeOption={applicationTypeOption}
            workflowTriggerTypeOption={workflowTriggerTypeOption}
            showWorkflow={showWorkflow}
            triggerType={triggerType}
            extension={extension}
            extensionContract={extensionContract}
            timezoneOptions={timezoneOptions}
            triggerLoadFailed={triggerLoadFailed}
          />
        </Form>
      )}
    </ResizableDrawer>
  );
}

function formatMutationError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
