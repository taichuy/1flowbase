import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Descriptions, Select, Space, Tag, Typography } from 'antd';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { ConsoleWorkflowExtensionAccessPolicy } from '@1flowbase/api-client';
import { useAuthStore } from '../../../state/auth-store';
import type { ApplicationDetail } from '../api/applications';
import {
  applicationApiMappingQueryKey,
  applicationApiPublicationQueryKey,
  fetchApplicationApiMapping,
  fetchApplicationApiPublication,
  publishApplicationApiVersion,
  saveApplicationApiMapping,
  unpublishApplicationApiVersion
} from '../api/public-api';

export function WorkflowExtensionApiPage({
  application
}: {
  application: ApplicationDetail;
}) {
  const { t } = useTranslation('applications');
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const mappingQuery = useQuery({
    queryKey: applicationApiMappingQueryKey(application.id),
    queryFn: () => fetchApplicationApiMapping(application.id)
  });
  const publicationQuery = useQuery({
    queryKey: applicationApiPublicationQueryKey(application.id),
    queryFn: () => fetchApplicationApiPublication(application.id),
    retry: false
  });
  const extension = mappingQuery.data?.extension ?? null;
  const [selectedAccessPolicy, setSelectedAccessPolicy] = useState<
    ConsoleWorkflowExtensionAccessPolicy | undefined
  >();
  const accessPolicy = selectedAccessPolicy ?? extension?.access_policy;
  const invalidate = () =>
    Promise.all([
      queryClient.invalidateQueries({
        queryKey: applicationApiMappingQueryKey(application.id)
      }),
      queryClient.invalidateQueries({
        queryKey: applicationApiPublicationQueryKey(application.id)
      })
    ]);
  const saveMutation = useMutation({
    mutationFn: async () => {
      if (!mappingQuery.data || !extension || !accessPolicy) return;
      await saveApplicationApiMapping(
        application.id,
        {
          ...mappingQuery.data,
          extension: { ...extension, access_policy: accessPolicy }
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      setSelectedAccessPolicy(undefined);
      await invalidate();
    }
  });
  const publishMutation = useMutation({
    mutationFn: async () => {
      const mapping =
        mappingQuery.data ?? (await fetchApplicationApiMapping(application.id));
      return publishApplicationApiVersion(application.id, mapping, csrfToken);
    },
    onSuccess: invalidate
  });
  const unpublishMutation = useMutation({
    mutationFn: () => unpublishApplicationApiVersion(application.id, csrfToken),
    onSuccess: invalidate
  });
  const publication = publicationQuery.data ?? null;
  const operation = publication?.operation ?? null;

  if (mappingQuery.isError) {
    return <Alert type="error" showIcon message={t('auto.trigger_load_failed')} />;
  }

  return (
    <Space direction="vertical" size="large" className="application-api-page">
      <Space wrap>
        <Typography.Title level={4} style={{ margin: 0 }}>
          {t('auto.workflow_extension_api')}
        </Typography.Title>
        <Tag color={publication?.api_enabled ? 'success' : 'default'}>
          {publication?.api_enabled
            ? t('auto.publication_published')
            : t('auto.publication_draft')}
        </Tag>
        <Button
          type={publication?.api_enabled ? 'default' : 'primary'}
          loading={publishMutation.isPending || unpublishMutation.isPending}
          onClick={() =>
            publication?.api_enabled
              ? unpublishMutation.mutate()
              : publishMutation.mutate()
          }
        >
          {publication?.api_enabled
            ? t('auto.revert_to_draft')
            : t('auto.publish_application')}
        </Button>
      </Space>

      <Descriptions
        bordered
        column={1}
        items={[
          {
            key: 'route',
            label: t('auto.call_path_template'),
            children: extension ? `/api/ex/${extension.slug}` : '—'
          },
          {
            key: 'method',
            label: t('auto.http_method'),
            children: extension?.method ?? '—'
          },
          {
            key: 'response',
            label: t('auto.response_mode'),
            children: extension?.response_mode ?? '—'
          },
          {
            key: 'access',
            label: t('auto.access_policy'),
            children: (
              <Space>
                <Select<ConsoleWorkflowExtensionAccessPolicy>
                  value={accessPolicy}
                  style={{ minWidth: 220 }}
                  onChange={setSelectedAccessPolicy}
                  options={[
                    {
                      value: 'user_api_key',
                      label: t('auto.access_policy_user_api_key')
                    },
                    {
                      value: 'public',
                      label: t('auto.access_policy_public')
                    }
                  ]}
                />
                <Button
                  disabled={!selectedAccessPolicy}
                  loading={saveMutation.isPending}
                  onClick={() => saveMutation.mutate()}
                >
                  {t('auto.save_changes')}
                </Button>
              </Space>
            )
          }
        ]}
      />

      {operation ? (
        <Descriptions
          title={t('auto.operation_schema')}
          bordered
          column={1}
          items={[
            {
              key: 'identity',
              label: t('auto.operation_identity'),
              children: operation.interface_id
            },
            {
              key: 'inputs',
              label: t('auto.request_parameters'),
              children: schemaFieldSummary(operation.parameter_schema)
            },
            {
              key: 'result',
              label: t('auto.response_fields'),
              children: schemaFieldSummary(operation.result_schema)
            }
          ]}
        />
      ) : null}
    </Space>
  );
}

function schemaFieldSummary(schema: Record<string, unknown>) {
  const names: string[] = [];
  collectPropertyNames(schema, '', names);
  return names.length > 0 ? names.join(' · ') : '—';
}

function collectPropertyNames(
  schema: Record<string, unknown>,
  prefix: string,
  names: string[]
) {
  const properties = schema.properties;
  if (!properties || typeof properties !== 'object') return;
  for (const [name, value] of Object.entries(properties)) {
    const path = prefix ? `${prefix}.${name}` : name;
    if (value && typeof value === 'object' && 'properties' in value) {
      collectPropertyNames(value as Record<string, unknown>, path, names);
    } else {
      names.push(path);
    }
  }
}
