import { useEffect, useState } from 'react';

import { Button, Drawer, Form, Input, Switch } from 'antd';

import type {
  CreateSettingsDataModelInput,
  SettingsDataModel,
  SettingsDataSource,
  UpdateSettingsDataModelInput
} from '../../api/data-models';
import { DataModelFieldLabel } from './DataModelHelpTooltip';
import {
  dataModelCodeHelp,
  dataModelStatusHelp,
  dataModelTitleHelp
} from './data-model-help-text';
import { i18nText } from '../../../../shared/i18n/text';

interface DataModelFormValues {
  code: string;
  title: string;
}

function isApiOpen(status: SettingsDataModel['status'] | undefined) {
  return status === 'published';
}

function apiOpenStatus(apiOpen: boolean): SettingsDataModel['status'] {
  return apiOpen ? 'published' : 'draft';
}

export function DataModelFormDrawer({
  open,
  mode,
  model,
  source,
  saving,
  onClose,
  onCreate,
  onUpdate
}: {
  open: boolean;
  mode: 'create' | 'edit';
  model: SettingsDataModel | null;
  source: SettingsDataSource | null;
  saving: boolean;
  onClose: () => void;
  onCreate: (input: CreateSettingsDataModelInput) => void;
  onUpdate: (
    model: SettingsDataModel,
    input: UpdateSettingsDataModelInput
  ) => void;
}) {
  const [form] = Form.useForm<DataModelFormValues>();
  const [apiOpen, setApiOpen] = useState(true);
  useEffect(() => {
    if (!open) {
      return;
    }

    if (mode === 'edit' && model) {
      setApiOpen(isApiOpen(model.status));
      form.setFieldsValue({
        code: model.code,
        title: model.title
      });
      return;
    }

    setApiOpen(isApiOpen(source?.default_data_model_status ?? 'published'));
    form.setFieldsValue({
      code: '',
      title: ''
    });
  }, [form, mode, model, open, source]);

  const handleSubmit = async () => {
    const values = await form.validateFields();

    if (mode === 'edit' && model) {
      onUpdate(model, { status: apiOpenStatus(apiOpen) });
      onClose();
      return;
    }

    onCreate({
      scope_kind: 'workspace',
      code: values.code,
      title: values.title,
      status: apiOpenStatus(apiOpen)
    });
    onClose();
  };

  return (
    <Drawer
      title={
        mode === 'create'
          ? i18nText('settings', 'auto.new_data_model')
          : i18nText('settings', 'auto.edit_data_model')
      }
      open={open}
      size={520}
      onClose={onClose}
      extra={
        <Button
          type="primary"
          aria-label={
            mode === 'create'
              ? i18nText('settings', 'auto.create')
              : i18nText('settings', 'auto.save')
          }
          loading={saving}
          onClick={handleSubmit}
        >
          {mode === 'create'
            ? i18nText('settings', 'auto.create')
            : i18nText('settings', 'auto.save')}
        </Button>
      }
    >
      <Form
        form={form}
        layout="vertical"
      >
        <Form.Item
          name="title"
          label={
            <DataModelFieldLabel
              label={i18nText('settings', 'auto.title')}
              title={dataModelTitleHelp}
            />
          }
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.enter_title')
            }
          ]}
        >
          <Input
            aria-label={i18nText('settings', 'auto.title')}
            disabled={mode === 'edit'}
          />
        </Form.Item>
        <Form.Item
          name="code"
          label={<DataModelFieldLabel label="Code" title={dataModelCodeHelp} />}
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.enter_data_model_code')
            }
          ]}
        >
          <Input aria-label="Code" disabled={mode === 'edit'} />
        </Form.Item>
        <Form.Item
          label={
            <DataModelFieldLabel
              label={i18nText('settings', 'auto.open_api')}
              title={dataModelStatusHelp}
            />
          }
        >
          <Switch
            aria-label={i18nText('settings', 'auto.open_api')}
            checked={apiOpen}
            checkedChildren={i18nText('settings', 'auto.open')}
            unCheckedChildren={i18nText('settings', 'auto.closed')}
            disabled={saving}
            onChange={setApiOpen}
          />
        </Form.Item>
      </Form>
    </Drawer>
  );
}
