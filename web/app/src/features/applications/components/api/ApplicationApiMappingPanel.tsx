import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Divider,
  Form,
  Modal,
  Select,
  Space,
  Tag,
  Typography
} from 'antd';

import {
  applicationApiMappingQueryKey,
  applicationOperationBindingsQueryKey,
  fetchApplicationApiMapping,
  fetchApplicationOperationBindings,
  saveApplicationApiMapping,
  type ApplicationApiMapping,
  type ApplicationOperationBindingOperation,
  type ApplicationOperationBindingOption,
  type ApplicationOperationBindingUnsupportedReason,
  type ApplicationOperationBindings,
  type ApplicationPublishedOperationBindingStatus
} from '../../api/public-api';

type OperationBindingFormTarget = {
  target_node_id?: string;
};

type OperationBindingFormValues = {
  operation_bindings?: {
    generate?: OperationBindingFormTarget | null;
    count_tokens?: OperationBindingFormTarget | null;
    compact?: {
      responses_compact?: OperationBindingFormTarget | null;
      responses_compaction_v2?: OperationBindingFormTarget | null;
    };
  };
};

function operationBindingFieldName(
  operation: ApplicationOperationBindingOperation
) {
  switch (operation) {
    case 'generate':
      return ['operation_bindings', 'generate', 'target_node_id'];
    case 'count_tokens':
      return ['operation_bindings', 'count_tokens', 'target_node_id'];
    case 'compact.responses_compact':
      return [
        'operation_bindings',
        'compact',
        'responses_compact',
        'target_node_id'
      ];
    case 'compact.responses_compaction_v2':
      return [
        'operation_bindings',
        'compact',
        'responses_compaction_v2',
        'target_node_id'
      ];
  }
}

function targetBindingFromFormValue(
  value: OperationBindingFormTarget | null | undefined
) {
  const targetNodeId = value?.target_node_id?.trim();

  return targetNodeId ? { target_node_id: targetNodeId } : null;
}

function saveableOperationBindings(
  current: ApplicationOperationBindings,
  values: OperationBindingFormValues,
  options: ApplicationOperationBindingOption[]
): ApplicationOperationBindings {
  const editableOperations = new Set(options.map((option) => option.operation));
  const nextBinding = (
    operation: ApplicationOperationBindingOperation,
    existing: ApplicationOperationBindings['generate'],
    value: OperationBindingFormTarget | null | undefined
  ) =>
    editableOperations.has(operation)
      ? targetBindingFromFormValue(value)
      : existing;

  return {
    generate: nextBinding(
      'generate',
      current.generate,
      values.operation_bindings?.generate
    ),
    count_tokens: nextBinding(
      'count_tokens',
      current.count_tokens,
      values.operation_bindings?.count_tokens
    ),
    compact: {
      responses_compact: nextBinding(
        'compact.responses_compact',
        current.compact.responses_compact,
        values.operation_bindings?.compact?.responses_compact
      ),
      responses_compaction_v2: nextBinding(
        'compact.responses_compaction_v2',
        current.compact.responses_compaction_v2,
        values.operation_bindings?.compact?.responses_compaction_v2
      )
    }
  };
}

export function ApplicationApiMappingPanel({
  applicationId,
  csrfToken
}: {
  applicationId: string;
  csrfToken: string;
}) {
  const { t } = useTranslation('applications');
  const [open, setOpen] = useState(false);
  const [form] = Form.useForm<OperationBindingFormValues>();
  const queryClient = useQueryClient();
  const mappingQuery = useQuery({
    queryKey: applicationApiMappingQueryKey(applicationId),
    queryFn: () => fetchApplicationApiMapping(applicationId)
  });
  const operationBindingsQuery = useQuery({
    queryKey: applicationOperationBindingsQueryKey(applicationId),
    queryFn: () => fetchApplicationOperationBindings(applicationId)
  });
  const projection = operationBindingsQuery.data;
  const draft = projection?.draft;
  const canEdit = projection?.editable === true && Boolean(mappingQuery.data);
  const saveMutation = useMutation({
    mutationFn: (mapping: ApplicationApiMapping) =>
      saveApplicationApiMapping(applicationId, mapping, csrfToken),
    onSuccess: (mapping) => {
      queryClient.setQueryData(
        applicationApiMappingQueryKey(applicationId),
        mapping
      );
      void queryClient.invalidateQueries({
        queryKey: applicationOperationBindingsQueryKey(applicationId)
      });
    }
  });

  useEffect(() => {
    if (draft) {
      form.setFieldsValue({ operation_bindings: draft.operation_bindings });
    }
  }, [draft, form]);

  const operationLabel = (operation: ApplicationOperationBindingOperation) => {
    switch (operation) {
      case 'generate':
        return t('auto.operation_binding_generate');
      case 'count_tokens':
        return t('auto.operation_binding_count_tokens');
      case 'compact.responses_compact':
        return t('auto.operation_binding_responses_compact');
      case 'compact.responses_compaction_v2':
        return t('auto.operation_binding_responses_compaction_version_two');
    }

    return operation;
  };
  const statusLabel = (status: ApplicationPublishedOperationBindingStatus) => {
    switch (status) {
      case 'supported':
        return t('auto.operation_binding_status_supported');
      case 'unbound':
        return t('auto.not_bound');
      case 'unsupported':
        return t('auto.operation_binding_status_unsupported');
    }

    return status;
  };
  const unsupportedReasonLabel = (
    reason: ApplicationOperationBindingUnsupportedReason
  ) => {
    switch (reason) {
      case 'compiled_plan_missing':
        return t('auto.operation_binding_reason_compiled_plan_missing');
      case 'compiled_plan_mismatch':
        return t('auto.operation_binding_reason_compiled_plan_mismatch');
      case 'compiled_plan_invalid':
        return t('auto.operation_binding_reason_compiled_plan_invalid');
      case 'target_missing':
        return t('auto.operation_binding_reason_target_missing');
      case 'target_not_llm':
        return t('auto.operation_binding_reason_target_not_llm');
      case 'target_runtime_incomplete':
        return t('auto.operation_binding_reason_target_runtime_incomplete');
      case 'provider_target_unavailable':
        return t('auto.operation_binding_reason_provider_target_unavailable');
      case 'provider_contract_unsupported':
        return t('auto.operation_binding_reason_provider_contract_unsupported');
      case 'provider_manifest_unavailable':
        return t('auto.operation_binding_reason_provider_manifest_unavailable');
      case 'provider_capability_unsupported':
        return t(
          'auto.operation_binding_reason_provider_capability_unsupported'
        );
    }

    return reason;
  };

  return (
    <>
      <Button onClick={() => setOpen(true)}>
        {t('auto.operation_bindings')}
      </Button>
      <Modal
        title={t('auto.operation_bindings')}
        open={open}
        destroyOnHidden
        footer={null}
        width={880}
        onCancel={() => setOpen(false)}
      >
        <Space
          direction="vertical"
          size={16}
          className="application-api-panel__stack"
        >
          {operationBindingsQuery.isLoading ? (
            <Alert
              type="info"
              showIcon
              message={t('auto.operation_binding_loading')}
            />
          ) : null}
          {operationBindingsQuery.isError ? (
            <Alert
              type="error"
              showIcon
              message={t('auto.operation_binding_load_failed')}
            />
          ) : null}
          {mappingQuery.isError ? (
            <Alert
              type="error"
              showIcon
              message={t('auto.operation_binding_mapping_load_failed')}
            />
          ) : null}
          {draft ? (
            <>
              <div>
                <Typography.Title level={5}>
                  {t('auto.draft_operation_bindings')}
                </Typography.Title>
                {!projection?.editable ? (
                  <Typography.Text type="secondary">
                    {t('auto.operation_binding_read_only')}
                  </Typography.Text>
                ) : null}
              </div>
              {draft.options.length === 0 ? (
                <Alert
                  type="info"
                  showIcon
                  message={t('auto.operation_binding_no_options')}
                />
              ) : (
                <Form<OperationBindingFormValues>
                  form={form}
                  layout="vertical"
                  onFinish={(values) => {
                    if (!mappingQuery.data || !projection) {
                      return;
                    }

                    saveMutation.mutate({
                      ...mappingQuery.data,
                      operation_bindings: saveableOperationBindings(
                        projection.draft.operation_bindings,
                        values,
                        projection.draft.options
                      )
                    });
                  }}
                >
                  <div className="application-api-mapping-grid">
                    {draft.options.map((option) => (
                      <div key={option.operation}>
                        <Form.Item
                          name={operationBindingFieldName(option.operation)}
                          label={operationLabel(option.operation)}
                        >
                          <Select
                            allowClear={canEdit}
                            aria-label={operationLabel(option.operation)}
                            aria-disabled={canEdit ? undefined : true}
                            disabled={!canEdit}
                            notFoundContent={null}
                            options={option.targets.map((target) => ({
                              value: target.target_node_id,
                              label: `${target.node_alias} · ${target.target_node_id}`
                            }))}
                          />
                        </Form.Item>
                        {option.targets.length === 0 ? (
                          <Alert
                            type="info"
                            showIcon
                            message={t('auto.operation_binding_no_targets')}
                          />
                        ) : null}
                      </div>
                    ))}
                  </div>
                  {canEdit ? (
                    <Button
                      type="primary"
                      htmlType="submit"
                      loading={saveMutation.isPending}
                    >
                      {t('auto.save_operation_bindings')}
                    </Button>
                  ) : null}
                </Form>
              )}
              <Divider />
              <div>
                <Typography.Title level={5}>
                  {t('auto.published_operation_bindings')}
                </Typography.Title>
                <Typography.Text type="secondary">
                  {t('auto.published_operation_bindings_read_only')}
                </Typography.Text>
              </div>
              {projection.published ? (
                <Descriptions column={1} size="small" bordered>
                  <Descriptions.Item
                    label={t('auto.operation_binding_publication_id')}
                  >
                    <Typography.Text code>
                      {projection.published.publication_id}
                    </Typography.Text>
                  </Descriptions.Item>
                  <Descriptions.Item
                    label={t('auto.operation_binding_compiled_plan_id')}
                  >
                    <Typography.Text code>
                      {projection.published.compiled_plan_id}
                    </Typography.Text>
                  </Descriptions.Item>
                  {projection.published.bindings.length === 0 ? (
                    <Descriptions.Item label={t('auto.operation_bindings')}>
                      <Alert
                        type="info"
                        showIcon
                        message={t('auto.published_operation_binding_empty')}
                      />
                    </Descriptions.Item>
                  ) : (
                    projection.published.bindings.map((binding) => (
                      <Descriptions.Item
                        key={binding.operation}
                        label={operationLabel(binding.operation)}
                      >
                        <Space direction="vertical" size={4}>
                          <Space wrap>
                            <Tag>{statusLabel(binding.status)}</Tag>
                            {binding.target ? (
                              <Typography.Text>
                                {binding.target.node_alias}
                              </Typography.Text>
                            ) : null}
                            {!binding.target && binding.target_node_id ? (
                              <Typography.Text code>
                                {binding.target_node_id}
                              </Typography.Text>
                            ) : null}
                          </Space>
                          {binding.status === 'unsupported' ? (
                            <Typography.Text type="danger">
                              {binding.unsupported_reason
                                ? unsupportedReasonLabel(
                                    binding.unsupported_reason
                                  )
                                : t(
                                    'auto.operation_binding_reason_unavailable'
                                  )}
                            </Typography.Text>
                          ) : null}
                        </Space>
                      </Descriptions.Item>
                    ))
                  )}
                </Descriptions>
              ) : (
                <Alert
                  type="info"
                  showIcon
                  message={t('auto.published_operation_binding_unavailable')}
                />
              )}
            </>
          ) : null}
        </Space>
      </Modal>
    </>
  );
}
