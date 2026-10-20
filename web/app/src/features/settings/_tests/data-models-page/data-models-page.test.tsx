import { describe, test, expect, vi } from 'vitest';
import './data-models-page.fixtures';
import { DataModelFormDrawer } from '../../components/data-models/DataModelFormDrawer';
import {
  SLOW_SETTINGS_PAGE_TEST_TIMEOUT,
  contactsModel,
  createAppView,
  dataModelsApi,
  generalTemplateSystemFields,
  openContactsDataModelEditor,
  openDataModelEditorByTitle,
  orderedTreeTemplateSystemFields,
  renderApp
} from './support';
import { dataModelStatusHelp } from '../../components/data-models/data-model-help-text';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { i18nText } from '../../../../shared/i18n/text';

describe('Settings data models page', () => {
  test('hides selected data source implementation metadata in model management', async () => {
    render(createAppView('/settings/data-models?source=main'));

    expect(
      await screen.findByRole('heading', { name: '主数据源' })
    ).toBeInTheDocument();
    expect(
      screen.queryByText(i18nText('settings', 'auto.id_label'))
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(i18nText('settings', 'auto.source_type'))
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(i18nText('settings', 'auto.catalog_label'))
    ).not.toBeInTheDocument();
  });

  test('AC-002 loads main compatible templates when opening create and submits general identity', async () => {
    renderApp('/settings/data-models?source=main');

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: '新建数据表' },
        {
          timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT
        }
      )
    );
    await waitFor(() =>
      expect(
        dataModelsApi.fetchSettingsCompatibleDataModelTemplates
      ).toHaveBeenCalledWith('main')
    );
    const createDialog = await screen.findByRole('dialog', {
      name: '新建 Data Model'
    });
    expect(document.querySelector('.schema-form-drawer')).toBeInTheDocument();
    expect(
      within(createDialog).getByRole('button', { name: '取消' })
    ).toBeInTheDocument();
    expect(within(createDialog).getByText('普通表')).toBeInTheDocument();
    fireEvent.change(within(createDialog).getByLabelText('标题'), {
      target: { value: 'Orders' }
    });
    fireEvent.change(within(createDialog).getByLabelText('Code'), {
      target: { value: 'orders' }
    });
    fireEvent.click(within(createDialog).getByRole('button', { name: '创建' }));

    await waitFor(() =>
      expect(dataModelsApi.createSettingsDataModel).toHaveBeenCalledWith(
        expect.objectContaining({
          template_provider: 'core',
          template_code: 'general',
          template_version: 'v1'
        }),
        'csrf-123'
      )
    );
  });

  test(
    'keeps runtime log model structural management read-only by backend capability',
    async () => {
      renderApp('/settings/data-models?source=main');

      const editorDialog = await openDataModelEditorByTitle('Runtime Logs');
      expect(
        within(editorDialog).getByRole('button', { name: '新增字段' })
      ).toBeDisabled();
      expect(within(editorDialog).getByText('Level')).toBeInTheDocument();
      const levelRow = within(editorDialog)
        .getAllByRole('row')
        .find((row) => within(row).queryByText('Level'));
      expect(levelRow).toBeInstanceOf(HTMLElement);
      expect(
        within(levelRow as HTMLElement).getByRole('button', { name: '编辑' })
      ).toBeEnabled();
      fireEvent.click(
        within(levelRow as HTMLElement).getByRole('button', { name: '编辑' })
      );
      expect(await screen.findByText('编辑字段')).toBeInTheDocument();
      expect(screen.getByLabelText('字段标题')).toBeEnabled();
      expect(screen.getByLabelText('说明')).toBeEnabled();
      expect(screen.getByLabelText('字段 Code')).toBeDisabled();
      expect(screen.getByLabelText('字段类型')).toBeDisabled();
      expect(screen.getByRole('button', { name: '删除字段' })).toBeDisabled();
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test(
    'selects a Data Model and exposes detail tabs with status metadata',
    async () => {
      renderApp('/settings/data-models?source=source-1');

      const editorDialog = await openContactsDataModelEditor();
      expect(
        await screen.findByRole('tab', { name: '字段' })
      ).toBeInTheDocument();
      expect(
        within(editorDialog).getByTestId('data-model-detail-summary')
      ).toBeInTheDocument();
      const detailSummary = within(editorDialog).getByTestId(
        'data-model-detail-summary'
      );
      expect(within(detailSummary).getByText('标题：')).toBeInTheDocument();
      expect(within(detailSummary).getByText('Code：')).toBeInTheDocument();
      expect(within(detailSummary).getByText('Contacts')).toBeInTheDocument();
      expect(within(detailSummary).getAllByText('contacts')).not.toHaveLength(
        0
      );
      const summaryRows = within(detailSummary).getAllByTestId(
        'data-model-summary-row'
      );
      expect(summaryRows).toHaveLength(5);
      expect(within(summaryRows[0]).getByText('标题：')).toBeInTheDocument();
      expect(within(summaryRows[0]).getByText('来源：')).toBeInTheDocument();
      expect(within(summaryRows[1]).getByText('Code：')).toBeInTheDocument();
      expect(
        within(summaryRows[1]).getByText('开放 API：')
      ).toBeInTheDocument();
      expect(within(summaryRows[1]).getByText('开放')).toBeInTheDocument();
      expect(
        within(summaryRows[2]).getByText('分类表类型：')
      ).toBeInTheDocument();
      expect(within(summaryRows[3]).getByText('数据源：')).toBeInTheDocument();
      expect(within(summaryRows[4]).getByText('说明：')).toBeInTheDocument();
      expect(
        within(summaryRows[4]).getByText('CRM contact records')
      ).toBeInTheDocument();
      expect(
        within(summaryRows[4]).getByTestId('data-model-summary-item')
      ).toHaveClass('data-model-panel__meta-card--full-row');
      expect(
        within(detailSummary).queryByText('状态：')
      ).not.toBeInTheDocument();
      expect(
        within(summaryRows[4]).queryByText('published')
      ).not.toBeInTheDocument();
      expect(within(detailSummary).getByText('available')).toBeInTheDocument();
      expect(
        within(detailSummary).getByText('远端资源标识：')
      ).toBeInTheDocument();
      const detailActions = within(editorDialog).getByTestId(
        'data-model-detail-actions'
      );
      const tabs = within(editorDialog).getByRole('tab', { name: '字段' });
      expect(detailActions).toBeInTheDocument();
      expect(
        detailActions.compareDocumentPosition(tabs) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      expect(
        within(detailActions).getByRole('button', {
          name: /编\s*辑/
        })
      ).toBeInTheDocument();
      expect(
        within(detailActions).queryByRole('combobox', {
          name: /状态/
        })
      ).not.toBeInTheDocument();
      expect(
        within(detailActions).queryByText('状态：')
      ).not.toBeInTheDocument();
      expect(
        within(editorDialog).queryByRole('combobox', {
          name: /状态/
        })
      ).not.toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '关系' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '权限' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'API' })).toBeInTheDocument();
      expect(
        screen.queryByRole('tab', { name: '记录预览' })
      ).not.toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '风险提示' })).toBeInTheDocument();

      fireEvent.click(screen.getByRole('tab', { name: 'API' }));
      expect(
        within(editorDialog).queryByTestId('data-model-api-exposure-status')
      ).not.toBeInTheDocument();
      expect(
        screen.queryByText('published_not_exposed')
      ).not.toBeInTheDocument();
      expect(
        screen.queryByRole('button', {
          name: '请求 API 暴露'
        })
      ).not.toBeInTheDocument();
      expect(
        screen.queryByRole('button', {
          name: '关闭 API 暴露'
        })
      ).not.toBeInTheDocument();
      expect(screen.getAllByText('开放 API').length).toBeGreaterThanOrEqual(1);
      expect(within(editorDialog).getByText('available')).toBeInTheDocument();
      const apiFieldsTable = await within(editorDialog).findByTestId(
        'data-model-api-fields-table'
      );
      expect(within(apiFieldsTable).getByText('暴露字段')).toBeInTheDocument();
      const emailRow = within(apiFieldsTable)
        .getAllByRole('row')
        .find((row) => within(row).queryByText('Email'));
      expect(emailRow).toBeInstanceOf(HTMLElement);
      expect(
        within(emailRow as HTMLElement).getByText('email')
      ).toBeInTheDocument();
      expect(
        within(emailRow as HTMLElement).getByText('string')
      ).toBeInTheDocument();
      const emailRequiredSwitch = await within(
        emailRow as HTMLElement
      ).findByRole('switch', {
        name: 'Email API 创建必填'
      });
      expect(emailRequiredSwitch).toBeChecked();
      fireEvent.click(emailRequiredSwitch);
      await waitFor(() =>
        expect(dataModelsApi.updateSettingsDataModelField).toHaveBeenCalledWith(
          'model-1',
          'field-1',
          expect.objectContaining({
            title: 'Email',
            description: 'Primary contact email',
            is_required: true,
            api_required: false,
            is_unique: true,
            default_value: null,
            display_interface: 'input',
            display_options: {},
            relation_options: {}
          }),
          'csrf-123'
        )
      );
      const remoteIdRow = within(apiFieldsTable)
        .getAllByRole('row')
        .find((row) => within(row).queryByText('Remote ID'));
      expect(remoteIdRow).toBeInstanceOf(HTMLElement);
      expect(
        within(remoteIdRow as HTMLElement).getByText('系统生成/只读')
      ).toBeInTheDocument();
      expect(
        within(remoteIdRow as HTMLElement).queryByRole('switch', {
          name: 'Remote ID API 创建必填'
        })
      ).not.toBeInTheDocument();
      expect(
        dataModelsApi.fetchSettingsDataModelOpenApiDocument
      ).toHaveBeenCalledWith('model-1');
      expect(
        screen.queryByRole('combobox', {
          name: i18nText('settings', 'auto.api_exposed_fields')
        })
      ).not.toBeInTheDocument();
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test(
    'keeps field contract warning visible when the Data Model OpenAPI contract fails to load',
    async () => {
      dataModelsApi.fetchSettingsDataModelOpenApiDocument.mockRejectedValueOnce(
        new Error('openapi unavailable')
      );
      renderApp('/settings/data-models?source=source-1');

      const editorDialog = await openContactsDataModelEditor();
      fireEvent.click(screen.getByRole('tab', { name: 'API' }));

      expect(
        await screen.findByText(
          i18nText('settings', 'auto.openapi_contract_load_failed')
        )
      ).toBeInTheDocument();
      expect(
        within(editorDialog).queryByTestId('data-model-api-exposure-status')
      ).not.toBeInTheDocument();

      const apiFieldsTable = await within(editorDialog).findByTestId(
        'data-model-api-fields-table'
      );
      const emailRow = within(apiFieldsTable)
        .getAllByRole('row')
        .find((row) => within(row).queryByText('Email'));
      expect(emailRow).toBeInstanceOf(HTMLElement);
      expect(
        within(emailRow as HTMLElement).getAllByText(
          i18nText('settings', 'auto.api_contract_unavailable')
        )
      ).toHaveLength(3);
      expect(
        within(emailRow as HTMLElement).queryByRole('switch', {
          name: 'Email API 创建必填'
        })
      ).not.toBeInTheDocument();
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test(
    'uses the shared native resizable drawer for Data Model details',
    async () => {
      renderApp('/settings/data-models?source=source-1');

      const editorDialog = await openContactsDataModelEditor();
      const keyboardResizeHandle = screen.getByRole('separator', {
        name: '调整 Data Model 详情宽度'
      });
      const drawerWrapper = editorDialog.closest('.ant-drawer-content-wrapper');
      const nativeResizeHandle = document.querySelector(
        '.data-model-panel__detail-drawer.resizable-drawer .ant-drawer-resizable-dragger-right'
      );

      expect(drawerWrapper).toBeInstanceOf(HTMLElement);
      expect(drawerWrapper).toHaveStyle({ width: '980px' });
      expect(drawerWrapper).toHaveStyle({
        minWidth: 'min(720px, calc(100vw - 48px))',
        maxWidth: 'calc(100vw - 48px)'
      });
      expect(nativeResizeHandle).toBeInstanceOf(HTMLElement);
      expect(keyboardResizeHandle).toHaveAttribute('aria-valuemin', '720');
      expect(keyboardResizeHandle).toHaveAttribute('aria-valuemax', '1280');

      fireEvent.mouseDown(nativeResizeHandle as HTMLElement, { clientX: 44 });
      fireEvent.mouseMove(document, { clientX: 144 });
      fireEvent.mouseUp(document);

      expect(drawerWrapper).toHaveStyle({ width: '880px' });
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test(
    'shows draft lifecycle status without model-level API exposure in the table and API tab',
    async () => {
      renderApp('/settings/data-models?source=source-1');

      expect(
        await screen.findByText(
          'Draft Orders',
          {},
          { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
        )
      ).toBeInTheDocument();
      const draftRow = screen
        .getAllByRole('row')
        .find((row) => within(row).queryByText('Draft Orders'));
      expect(draftRow).toBeInstanceOf(HTMLElement);
      expect(
        within(draftRow as HTMLElement).getByText('关闭 API')
      ).toBeInTheDocument();
      expect(
        within(draftRow as HTMLElement).queryByText(
          i18nText('settings', 'auto.draft')
        )
      ).not.toBeInTheDocument();

      const editorDialog = await openDataModelEditorByTitle('Draft Orders');
      fireEvent.click(screen.getByRole('tab', { name: 'API' }));
      expect(
        within(editorDialog).queryByTestId('data-model-api-exposure-status')
      ).not.toBeInTheDocument();
      expect(
        screen.queryByRole('button', {
          name: '请求 API 暴露'
        })
      ).not.toBeInTheDocument();
      expect(
        screen.queryByRole('button', {
          name: '关闭 API 暴露'
        })
      ).not.toBeInTheDocument();
      expect(
        dataModelsApi.fetchSettingsDataModelOpenApiDocument
      ).not.toHaveBeenCalledWith('model-draft-orders');
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test('shows editable grants and localized advisor findings without record preview', async () => {
    renderApp('/settings/data-models?source=source-1');

    await openContactsDataModelEditor();
    fireEvent.click(screen.getByRole('tab', { name: '权限' }));
    expect(await screen.findByText('owner')).toBeInTheDocument();
    expect(screen.getByText('scope_all')).toBeInTheDocument();
    expect(screen.getByText('system_all')).toBeInTheDocument();

    const openPermissionDropdown = async (name: string) => {
      const trigger = await screen.findByRole('combobox', { name });
      fireEvent.mouseDown(trigger);
      await waitFor(() =>
        expect(
          document.querySelector(
            '.ant-select-dropdown:not(.ant-select-dropdown-hidden)'
          )
        ).toBeInstanceOf(HTMLElement)
      );
      return document.querySelector(
        '.ant-select-dropdown:not(.ant-select-dropdown-hidden)'
      ) as HTMLElement;
    };

    const workspaceDropdown = await openPermissionDropdown('权限 grant-owner');
    expect(
      within(workspaceDropdown).getByText('owner', {
        selector: '.ant-select-item-option-content'
      })
    ).toBeInTheDocument();
    expect(
      within(workspaceDropdown).getByText('scope_all', {
        selector: '.ant-select-item-option-content'
      })
    ).toBeInTheDocument();
    expect(
      within(workspaceDropdown).queryByText('system_all', {
        selector: '.ant-select-item-option-content'
      })
    ).not.toBeInTheDocument();
    const systemDropdown = await openPermissionDropdown('权限 grant-system');
    expect(
      within(systemDropdown).getByText('system_all', {
        selector: '.ant-select-item-option-content'
      })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '保存权限' }));
    await waitFor(() =>
      expect(dataModelsApi.updateSettingsDataModelScopeGrant).toHaveBeenCalled()
    );

    expect(
      dataModelsApi.fetchSettingsDataModelRecordPreview
    ).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('tab', { name: '风险提示' }));
    const advisorTab = await screen.findByTestId('data-model-advisor-tab');
    expect(within(advisorTab).getByText('阻塞')).toBeInTheDocument();
    expect(within(advisorTab).getByText('高')).toBeInTheDocument();
    expect(within(advisorTab).getByText('提示')).toBeInTheDocument();
    expect(
      within(advisorTab).getByText('外部数据源缺少作用域过滤')
    ).toBeInTheDocument();
    expect(
      within(advisorTab).getByText('受保护模型暴露尝试')
    ).toBeInTheDocument();
    expect(within(advisorTab).getByText('字段映射提示')).toBeInTheDocument();
    expect(
      within(advisorTab).getByText('请启用作用域过滤。')
    ).toBeInTheDocument();
    expect(
      within(advisorTab).queryByText('protected_model_exposure_attempt')
    ).not.toBeInTheDocument();
  }, 20_000);

  test('exposes Data Model editing from the detail drawer', async () => {
    renderApp('/settings/data-models?source=source-1');

    await screen.findAllByText(
      'Contacts',
      {},
      { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
    );
    const contactsRow = screen
      .getAllByRole('row')
      .find(
        (row) =>
          within(row).queryByText('Contacts') &&
          within(row).queryByRole('button', { name: '编辑' })
      );
    expect(contactsRow).toBeInstanceOf(HTMLElement);

    fireEvent.click(
      within(contactsRow as HTMLElement).getByRole('button', { name: '编辑' })
    );
    expect(await screen.findByText('编辑 Contacts')).toBeInTheDocument();
    const editorDialog = await screen.findByRole('region', {
      name: 'Data Model 详情'
    });
    const detailActions = within(editorDialog).getByTestId(
      'data-model-detail-actions'
    );
    expect(
      within(editorDialog).getByRole('tab', { name: '字段' })
    ).toBeInTheDocument();
    expect(within(editorDialog).getByText('分类表类型：')).toBeInTheDocument();
    expect(within(editorDialog).getByText('General data model')).toBeVisible();
    expect(
      within(editorDialog).queryByText('模板提供方：')
    ).not.toBeInTheDocument();
    expect(
      within(editorDialog).queryByText('模板代码：')
    ).not.toBeInTheDocument();
    expect(
      within(editorDialog).queryByText('模板版本：')
    ).not.toBeInTheDocument();
    expect(within(editorDialog).getAllByText('contacts')).not.toHaveLength(0);
    fireEvent.click(
      within(detailActions).getByRole('button', { name: /编\s*辑/ })
    );
    expect(await screen.findByDisplayValue('Contacts')).toBeInTheDocument();
    expect(screen.getByLabelText('Code')).toBeDisabled();
    expect(screen.queryByLabelText('Data Model 模板')).not.toBeInTheDocument();
  }, 20_000);

  test('submits Data Model edits from the form drawer', async () => {
    const onUpdate = vi.fn();

    render(
      <DataModelFormDrawer
        open
        mode="edit"
        model={contactsModel}
        source={null}
        compatibleTemplates={[]}
        templatesLoading={false}
        templatesError={null}
        saving={false}
        onClose={vi.fn()}
        onCreate={vi.fn()}
        onUpdate={onUpdate}
      />
    );

    const editDialog = await screen.findByRole(
      'dialog',
      {
        name: '编辑 Data Model'
      },
      { timeout: 5000 }
    );
    const titleInput = within(editDialog).getByLabelText('标题');
    expect(titleInput).toBeEnabled();
    fireEvent.change(titleInput, { target: { value: 'Customer Contacts' } });
    const descriptionInput = within(editDialog).getByLabelText('说明');
    expect(descriptionInput).toBeEnabled();
    fireEvent.change(descriptionInput, {
      target: { value: 'Customer contact records' }
    });
    expect(
      within(editDialog).queryByLabelText('表 ID')
    ).not.toBeInTheDocument();
    const apiSwitch = within(editDialog).getByRole('switch', {
      name: '开放 API'
    });
    expect(apiSwitch).toBeChecked();
    expect(
      within(editDialog).queryByLabelText('数据源')
    ).not.toBeInTheDocument();
    fireEvent.click(apiSwitch);
    fireEvent.click(within(editDialog).getByRole('button', { name: '保存' }));

    await waitFor(() =>
      expect(onUpdate).toHaveBeenCalledWith(contactsModel, {
        title: 'Customer Contacts',
        description: 'Customer contact records',
        status: 'draft'
      })
    );
  });

  test('submits Data Model descriptions when creating from the form drawer', async () => {
    const onCreate = vi.fn();

    render(
      <DataModelFormDrawer
        open
        mode="create"
        model={null}
        source={null}
        compatibleTemplates={[
          {
            template_provider: 'core',
            template_code: 'general',
            template_version: 'v1',
            summary: '普通表',
            description: '适用于通用记录。',
            system_fields: generalTemplateSystemFields
          }
        ]}
        templatesLoading={false}
        templatesError={null}
        saving={false}
        onClose={vi.fn()}
        onCreate={onCreate}
        onUpdate={vi.fn()}
      />
    );

    const createDialog = await screen.findByRole('dialog', {
      name: '新建 Data Model'
    });
    fireEvent.change(within(createDialog).getByLabelText('标题'), {
      target: { value: 'Orders' }
    });
    fireEvent.change(within(createDialog).getByLabelText('Code'), {
      target: { value: 'orders' }
    });
    fireEvent.change(within(createDialog).getByLabelText('说明'), {
      target: { value: 'Workspace order records' }
    });
    fireEvent.click(within(createDialog).getByRole('button', { name: '创建' }));

    await waitFor(() =>
      expect(onCreate).toHaveBeenCalledWith({
        scope_kind: 'workspace',
        template_provider: 'core',
        template_code: 'general',
        template_version: 'v1',
        code: 'orders',
        title: 'Orders',
        description: 'Workspace order records',
        status: 'published'
      })
    );
  });

  test('AC-002 selects only backend-compatible main templates and submits the selected identity', async () => {
    const onCreate = vi.fn();

    render(
      <DataModelFormDrawer
        open
        mode="create"
        model={null}
        source={null}
        compatibleTemplates={[
          {
            template_provider: 'core',
            template_code: 'general',
            template_version: 'v1',
            summary: '普通表',
            description: '适用于通用记录。',
            system_fields: generalTemplateSystemFields
          },
          {
            template_provider: 'core',
            template_code: 'ordered_tree',
            template_version: 'v1',
            summary: 'Ordered tree data model',
            description: 'Core main-source ordered-tree Data Model template.',
            system_fields: orderedTreeTemplateSystemFields
          }
        ]}
        templatesLoading={false}
        templatesError={null}
        saving={false}
        onClose={vi.fn()}
        onCreate={onCreate}
        onUpdate={vi.fn()}
      />
    );

    const createDialog = await screen.findByRole('dialog', {
      name: '新建 Data Model'
    });
    expect(within(createDialog).getByText('普通表')).toBeInTheDocument();
    const templateSelect = within(createDialog).getByRole('combobox', {
      name: 'Data Model 模板'
    });
    expect(templateSelect.closest('.ant-select')).toHaveStyle({
      width: '100%'
    });
    const apiOpenRow = createDialog.querySelector(
      '.schema-form-drawer__inline-switch'
    );
    expect(apiOpenRow).not.toBeNull();
    expect(
      within(apiOpenRow as HTMLElement).getByText('开放 API')
    ).toBeVisible();
    expect(
      within(apiOpenRow as HTMLElement).getByRole('switch', {
        name: '开放 API'
      })
    ).toBeVisible();
    expect(
      within(apiOpenRow as HTMLElement).queryByText(dataModelStatusHelp)
    ).not.toBeInTheDocument();
    const apiOpenDescription =
      within(createDialog).getByText(dataModelStatusHelp);
    expect(apiOpenDescription).toBeVisible();
    expect(
      (apiOpenRow as HTMLElement).compareDocumentPosition(apiOpenDescription) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      within(createDialog).getByRole('separator', {
        name: '调整 Data Model 抽屉宽度'
      })
    ).toBeInTheDocument();
    const defaultFieldsTable = within(createDialog).getByRole('table');
    expect(
      within(defaultFieldsTable)
        .getAllByRole('columnheader')
        .map((header) => header.textContent)
    ).toEqual(['Code', '类型', '必填']);
    const orderedFormElements = [
      within(createDialog).getByRole('combobox', {
        name: 'Data Model 模板'
      }),
      within(createDialog).getByLabelText('标题'),
      within(createDialog).getByLabelText('Code'),
      within(createDialog).getByLabelText('说明'),
      within(createDialog).getByRole('switch', { name: '开放 API' }),
      defaultFieldsTable
    ];
    orderedFormElements.slice(0, -1).forEach((element, index) => {
      expect(
        element.compareDocumentPosition(orderedFormElements[index + 1]) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    });
    expect(within(defaultFieldsTable).getAllByRole('row')).toHaveLength(7);
    expect(within(defaultFieldsTable).getByText('id')).toBeInTheDocument();
    expect(screen.queryByText('适用于通用记录。')).not.toBeInTheDocument();
    fireEvent.mouseDown(
      within(createDialog).getByRole('combobox', {
        name: 'Data Model 模板'
      })
    );
    expect(
      await screen.findByRole('option', { name: '树状表' })
    ).toHaveAccessibleName('树状表');
    fireEvent.click(await screen.findByText('树状表'));
    expect(within(defaultFieldsTable).getAllByRole('row')).toHaveLength(9);
    expect(
      within(defaultFieldsTable).getByText('parent_id')
    ).toBeInTheDocument();
    expect(
      within(defaultFieldsTable).getByText('sibling_rank')
    ).toBeInTheDocument();
    expect(
      screen.queryByText('Core main-source ordered-tree Data Model template.')
    ).not.toBeInTheDocument();
    fireEvent.change(within(createDialog).getByLabelText('标题'), {
      target: { value: 'Categories' }
    });
    fireEvent.change(within(createDialog).getByLabelText('Code'), {
      target: { value: 'categories' }
    });
    fireEvent.click(within(createDialog).getByRole('button', { name: '创建' }));

    await waitFor(() =>
      expect(onCreate).toHaveBeenCalledWith(
        expect.objectContaining({
          template_provider: 'core',
          template_code: 'ordered_tree',
          template_version: 'v1'
        })
      )
    );
  });
});
