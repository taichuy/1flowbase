import {
  CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS,
  CONFIGURED_MODEL_GRID_GAP,
  type ModelProviderInstanceDrawerProps
} from './model-provider-instance-config';
import { useModelProviderInstanceController } from './useModelProviderInstanceController';

import {
  Button,
  Empty,
  Flex,
  Form,
  Input,
  Space,
  Switch,
  Tag,
  Typography
} from 'antd';

import ApiOutlined from '@ant-design/icons/es/icons/ApiOutlined';
import CheckCircleOutlined from '@ant-design/icons/es/icons/CheckCircleOutlined';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';

import { CollapseShell } from '../../../../shared/ui/collapse-shell/CollapseShell';
import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { CachedModelSelect } from './CachedModelSelect';

import { i18nText } from '../../../../shared/i18n/text';
import { ModelProviderConfiguredModelModal } from './ModelProviderConfiguredModelModal';
import { ModelProviderAccountOperationsCard } from './ModelProviderAccountOperationsCard';

export function ModelProviderInstanceDrawer(
  props: ModelProviderInstanceDrawerProps
) {
  if (!props.open) {
    return null;
  }

  return <ModelProviderInstanceDrawerContent {...props} />;
}

function ModelProviderInstanceDrawerContent({
  open,
  mode,
  catalogEntry,
  instance,
  cachedModelCatalog,
  pricingTargets,
  defaultIncludedInMain,
  submitting,
  onClose,
  onSubmit,
  onPreviewModels,
  onRevealSecret,
  onAuthenticate,
  onCreateDraft,
  onDiscardDraft,
  onValidate,
  onRefreshUsage,
  onCountResetCredits,
  onConsumeResetCredit
}: ModelProviderInstanceDrawerProps) {
  const {
    form,
    previewModels,
    configuredModels,
    setConfiguredModels,
    selectedCachedModelId,
    previewingModels,
    usageSnapshot,
    setUsageSnapshot,
    configuredModelEditor,
    setConfiguredModelEditor,
    canDetect,
    nextConfiguredModelKey,
    clearPreviewState,
    openConfiguredModelEditor,
    saveConfiguredModel,
    applyCachedModelSelection,
    title,
    primaryConfigFields,
    advancedConfigFields,
    updateConfiguredModelRow,
    removeConfiguredModelRow,
    refreshUsageSnapshot,
    handlePreviewModels,
    handleSubmit,
    renderConfigField,
    renderAuthenticationCard
  } = useModelProviderInstanceController({
    open,
    mode,
    catalogEntry,
    instance,
    cachedModelCatalog,
    pricingTargets,
    defaultIncludedInMain,
    submitting,
    onClose,
    onSubmit,
    onPreviewModels,
    onRevealSecret,
    onAuthenticate,
    onCreateDraft,
    onDiscardDraft,
    onValidate,
    onRefreshUsage,
    onCountResetCredits,
    onConsumeResetCredit
  });

  return (
    <>
      <ResizableDrawer
        defaultWidth={560}
        maxWidth={1200}
        minWidth={480}
        open={open}
        zIndex={1100}
        title={title}
        onClose={onClose}
        destroyOnClose
        resizeLabel="调整供应商抽屉宽度"
        footer={
          <div style={{ textAlign: 'right' }}>
            <Space>
              <Button
                type="primary"
                loading={submitting}
                onClick={() => {
                  void handleSubmit().catch(() => undefined);
                }}
              >
                {i18nText('settings', 'auto.save')}
              </Button>
              <Button onClick={onClose}>
                {i18nText('settings', 'auto.cancel')}
              </Button>
            </Space>
          </div>
        }
      >
        <Form
          form={form}
          layout="vertical"
          onValuesChange={(changedValues) => {
            if ('config' in changedValues) {
              clearPreviewState();
            }
          }}
        >
          {catalogEntry ? (
            <>
              <div className="model-provider-drawer__card">
                <div
                  className="model-provider-drawer__card-title"
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    width: '100%',
                    flexWrap: 'wrap',
                    gap: 8
                  }}
                >
                  <div
                    style={{ display: 'flex', alignItems: 'center', gap: 8 }}
                  >
                    <ApiOutlined
                      style={{ color: 'var(--ant-color-primary)' }}
                    />
                    <span>{catalogEntry.display_name}</span>
                  </div>
                  <div
                    style={{
                      display: 'flex',
                      gap: 4,
                      flexWrap: 'wrap',
                      fontWeight: 'normal',
                      fontSize: '12px'
                    }}
                  >
                    <Tag color="blue" style={{ margin: 0 }}>
                      {catalogEntry.provider_code}
                    </Tag>
                    <Tag color="cyan" style={{ margin: 0 }}>
                      {catalogEntry.protocol}
                    </Tag>
                    <Tag color="purple" style={{ margin: 0 }}>
                      {i18nText('settings', 'auto.discovery_mode')}
                      {catalogEntry.model_discovery_mode}
                    </Tag>
                    <Tag color="gold" style={{ margin: 0 }}>
                      {i18nText('settings', 'auto.preset_models')}
                      {catalogEntry.predefined_models.length}
                    </Tag>
                  </div>
                </div>
                <div className="model-provider-drawer__card-body">
                  <Flex gap={16} align="flex-start">
                    <div style={{ flex: 1 }}>
                      <Form.Item
                        label={i18nText('settings', 'auto.name')}
                        name="display_name"
                        rules={[
                          {
                            required: true,
                            message: i18nText('settings', 'auto.fill_name')
                          }
                        ]}
                        style={{ marginBottom: 0 }}
                      >
                        <Input
                          placeholder={i18nText(
                            'settings',
                            'auto.example_openai_production'
                          )}
                        />
                      </Form.Item>
                    </div>
                    <div style={{ flex: 'none' }}>
                      <Form.Item
                        label={i18nText(
                          'settings',
                          'auto.inject_main_instance_alt'
                        )}
                        name="included_in_main"
                        valuePropName="checked"
                        style={{ marginBottom: 0 }}
                      >
                        <Switch
                          aria-label={i18nText(
                            'settings',
                            'auto.inject_main_instance_alt'
                          )}
                        />
                      </Form.Item>
                    </div>
                  </Flex>
                </div>
              </div>

              {renderAuthenticationCard()}

              <div className="model-provider-drawer__card">
                <div
                  className="model-provider-drawer__card-title"
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    width: '100%'
                  }}
                >
                  <div
                    style={{ display: 'flex', alignItems: 'center', gap: 8 }}
                  >
                    <CheckCircleOutlined />
                    <span>
                      {i18nText('settings', 'auto.connection_configuration')}
                    </span>
                  </div>
                  {canDetect ? (
                    <div>
                      <Button
                        size="small"
                        loading={previewingModels}
                        onClick={(e) => {
                          e.stopPropagation();
                          void handlePreviewModels().catch(() => undefined);
                        }}
                      >
                        {i18nText('settings', 'auto.detection')}
                      </Button>
                    </div>
                  ) : null}
                </div>
                <div className="model-provider-drawer__card-body">
                  {primaryConfigFields.map(renderConfigField)}
                  {advancedConfigFields.length > 0 ? (
                    <div style={{ marginTop: 12 }}>
                      <CollapseShell
                        variant="compact"
                        items={[
                          {
                            key: 'advanced-config',
                            header: i18nText(
                              'settings',
                              'auto.advanced_configuration_optional'
                            ),
                            children:
                              advancedConfigFields.map(renderConfigField)
                          }
                        ]}
                      />
                    </div>
                  ) : null}
                </div>
              </div>

              {instance ? (
                <ModelProviderAccountOperationsCard
                  catalogEntry={catalogEntry}
                  usageSnapshot={usageSnapshot}
                  onUsageSnapshot={setUsageSnapshot}
                  onRefreshUsage={
                    onRefreshUsage ? refreshUsageSnapshot : undefined
                  }
                  onCountResetCredits={onCountResetCredits}
                  onConsumeResetCredit={onConsumeResetCredit}
                />
              ) : null}

              <div className="model-provider-drawer__card">
                <div className="model-provider-drawer__card-title">
                  <PlusOutlined />
                  <span>
                    {i18nText('settings', 'auto.model_configuration')}
                  </span>
                </div>
                <div className="model-provider-drawer__card-body">
                  <Space
                    orientation="vertical"
                    size={16}
                    style={{ width: '100%' }}
                  >
                    <Flex align="center" gap={12} style={{ width: '100%' }}>
                      <div style={{ flex: 1 }}>
                        <CachedModelSelect
                          modelIds={previewModels.map(
                            (model) => model.model_id
                          )}
                          ariaLabel={i18nText('settings', 'auto.cache_model')}
                          placeholder={i18nText('settings', 'auto.cache_model')}
                          value={selectedCachedModelId}
                          emptyMode="select"
                          style={{ width: '100%' }}
                          onChange={applyCachedModelSelection}
                        />
                      </div>
                      <Button
                        type="dashed"
                        aria-label={i18nText('settings', 'auto.new')}
                        onClick={() => openConfiguredModelEditor()}
                      >
                        {i18nText('settings', 'auto.new')}
                      </Button>
                      {previewModels.length > 0 && (
                        <Button
                          type="primary"
                          onClick={() => {
                            setConfiguredModels((current) => {
                              const existingIds = new Set(
                                current.map((row) => row.model_id.trim())
                              );
                              const newRows = [...current];
                              for (const pm of previewModels) {
                                const id = pm.model_id.trim();
                                if (id && !existingIds.has(id)) {
                                  newRows.push({
                                    key: nextConfiguredModelKey(),
                                    model_id: id,
                                    context_window_input: '',
                                    context_window_error: null,
                                    supports_multimodal: pm.supports_multimodal,
                                    enabled: true,
                                    pricing_provider_code: 'zero',
                                    pricing_model_id: 'any'
                                  });
                                  existingIds.add(id);
                                }
                              }
                              return newRows;
                            });
                          }}
                        >
                          {i18nText('settings', 'auto.import_all')}
                        </Button>
                      )}
                    </Flex>

                    <div className="model-provider-drawer__model-table">
                      <div
                        className="model-provider-drawer__model-header"
                        style={{
                          gridTemplateColumns:
                            CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS,
                          gap: CONFIGURED_MODEL_GRID_GAP,
                          alignItems: 'center'
                        }}
                      >
                        <Typography.Text strong style={{ color: 'inherit' }}>
                          {i18nText('settings', 'auto.model_id_alt')}
                        </Typography.Text>
                        <Typography.Text strong style={{ color: 'inherit' }}>
                          {i18nText('settings', 'auto.context_alt')}
                        </Typography.Text>
                        <Typography.Text strong style={{ color: 'inherit' }}>
                          {i18nText('settings', 'auto.billing_pricing_rules')}
                        </Typography.Text>
                        <Typography.Text
                          strong
                          style={{ textAlign: 'center', color: 'inherit' }}
                        >
                          {i18nText('settings', 'auto.multimodal')}
                        </Typography.Text>
                        <Typography.Text
                          strong
                          style={{ textAlign: 'center', color: 'inherit' }}
                        >
                          {i18nText('settings', 'auto.enabled')}
                        </Typography.Text>
                        <Typography.Text
                          strong
                          style={{ textAlign: 'center', color: 'inherit' }}
                        >
                          {i18nText('settings', 'auto.operation')}
                        </Typography.Text>
                      </div>

                      {configuredModels.length > 0 ? (
                        configuredModels.map((row, index) => (
                          <div
                            key={row.key}
                            className="model-provider-drawer__model-row"
                            style={{
                              gridTemplateColumns:
                                CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS,
                              gap: CONFIGURED_MODEL_GRID_GAP,
                              alignItems: 'start'
                            }}
                          >
                            <Typography.Text
                              ellipsis={{ tooltip: row.model_id }}
                            >
                              {row.model_id}
                            </Typography.Text>
                            <Typography.Text>
                              {row.context_window_input || '—'}
                            </Typography.Text>
                            <Typography.Text
                              ellipsis={{
                                tooltip: `${row.pricing_provider_code} / ${row.pricing_model_id}`
                              }}
                            >
                              {row.pricing_provider_code} /{' '}
                              {row.pricing_model_id}
                            </Typography.Text>
                            <div
                              style={{
                                display: 'flex',
                                justifyContent: 'center',
                                paddingTop: 5
                              }}
                            >
                              <Switch
                                size="small"
                                aria-label={i18nText(
                                  'settings',
                                  'auto.enable_multimodal_model',
                                  { value1: index + 1 }
                                )}
                                checked={row.supports_multimodal}
                                onChange={(checked) => {
                                  updateConfiguredModelRow(row.key, {
                                    supports_multimodal: checked
                                  });
                                }}
                              />
                            </div>
                            <div
                              style={{
                                display: 'flex',
                                justifyContent: 'center',
                                paddingTop: 5
                              }}
                            >
                              <Switch
                                size="small"
                                aria-label={i18nText(
                                  'settings',
                                  'auto.enable_model',
                                  { value1: index + 1 }
                                )}
                                checked={row.enabled}
                                onChange={(checked) => {
                                  updateConfiguredModelRow(row.key, {
                                    enabled: checked
                                  });
                                }}
                              />
                            </div>
                            <div
                              style={{
                                display: 'flex',
                                justifyContent: 'center',
                                gap: 2
                              }}
                            >
                              <Button
                                size="small"
                                type="text"
                                icon={<EditOutlined />}
                                aria-label={i18nText(
                                  'settings',
                                  'auto.edit_model',
                                  { value1: index + 1 }
                                )}
                                onClick={() => openConfiguredModelEditor(row)}
                              />
                              <Button
                                danger
                                size="small"
                                type="text"
                                icon={<DeleteOutlined />}
                                aria-label={i18nText(
                                  'settings',
                                  'auto.delete_model',
                                  { value1: index + 1 }
                                )}
                                className="model-provider-drawer__delete-btn"
                                style={{ height: 'auto', padding: '4px 8px' }}
                                onClick={() =>
                                  removeConfiguredModelRow(row.key)
                                }
                              />
                            </div>
                          </div>
                        ))
                      ) : (
                        <div
                          style={{
                            padding: '32px 16px',
                            textAlign: 'center'
                          }}
                        >
                          <Empty
                            image={Empty.PRESENTED_IMAGE_SIMPLE}
                            description={i18nText(
                              'settings',
                              'auto.text_option'
                            )}
                          />
                        </div>
                      )}
                    </div>
                  </Space>
                </div>
              </div>
            </>
          ) : (
            <Typography.Text type="secondary">
              {i18nText(
                'settings',
                'auto.currently_provider_catalog_available'
              )}
            </Typography.Text>
          )}
        </Form>
      </ResizableDrawer>
      <ModelProviderConfiguredModelModal
        open={configuredModelEditor !== null}
        editing={Boolean(configuredModelEditor?.rowKey)}
        initialValue={configuredModelEditor?.initialValue ?? null}
        modelIds={previewModels.map((model) => model.model_id)}
        reservedModelIds={configuredModels.reduce<string[]>((reserved, row) => {
          if (row.key !== configuredModelEditor?.rowKey) {
            reserved.push(row.model_id);
          }
          return reserved;
        }, [])}
        pricingTargets={pricingTargets}
        onCancel={() => setConfiguredModelEditor(null)}
        onSave={saveConfiguredModel}
      />
    </>
  );
}
