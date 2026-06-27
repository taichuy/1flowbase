import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { i18nText } from '../../../../shared/i18n/text';
import { DataModelFormDrawer } from '../../components/data-models/DataModelFormDrawer';
import {
  SLOW_SETTINGS_PAGE_TEST_TIMEOUT,
  cleanupDataModelsPageTest,
  contactsModel,
  dataModelsApi,
  findDataModelsNavigation,
  openContactsDataModelEditor,
  openDataModelEditorByTitle,
  renderApp,
  setupDataModelsPageTest
} from './support';

beforeEach(setupDataModelsPageTest);
afterEach(cleanupDataModelsPageTest);

describe('Settings data models page', () => {
  test(
    'shows data source navigation, defaults, and the Data Model table',
    async () => {
      renderApp('/settings/data-models');

      expect(await findDataModelsNavigation()).toBeInTheDocument();
      expect(await screen.findByText('主数据源')).toBeInTheDocument();
      expect(await screen.findByText('HubSpot')).toBeInTheDocument();
      const hubSpotRow = screen
        .getAllByRole('row')
        .find((row) => within(row).queryByText('HubSpot'));
      expect(hubSpotRow).toBeInstanceOf(HTMLElement);
      expect(
        within(hubSpotRow as HTMLElement).getByLabelText('HubSpot 启用')
      ).toBeChecked();
      fireEvent.click(
        within(hubSpotRow as HTMLElement).getByRole('button', { name: '配置' })
      );
      expect(await screen.findByText('数据源管理')).toBeInTheDocument();
      expect(
        screen.queryByText(
          '管理内建主数据源和外部数据源的默认建模状态、API 暴露策略与 Data Model 访问面。'
        )
      ).not.toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: /返\s*回/ })
      ).toBeInTheDocument();
      expect(screen.getByLabelText('默认 Data Model 状态')).toBeInTheDocument();
      expect(screen.getByLabelText('默认 API 暴露状态')).toBeInTheDocument();
      expect(
        screen.getByRole('heading', { name: 'HubSpot' })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: '新建数据表' })
      ).toBeInTheDocument();
      expect(screen.getByText('数据表')).toBeInTheDocument();
      expect(await screen.findByText('Contacts')).toBeInTheDocument();
      expect(screen.getByText('contacts')).toBeInTheDocument();
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test(
    'shows built-in user and role metadata in the main data source editor',
    async () => {
      renderApp('/settings/data-models');

      expect(await findDataModelsNavigation()).toBeInTheDocument();
      expect(await screen.findByText('主数据源')).toBeInTheDocument();
      const mainSourceRow = screen
        .getAllByRole('row')
        .find((row) => within(row).queryByText('主数据源'));
      expect(mainSourceRow).toBeInstanceOf(HTMLElement);
      fireEvent.click(
        within(mainSourceRow as HTMLElement).getByRole('button', {
          name: '配置'
        })
      );

      expect(await screen.findByText('Attachments')).toBeInTheDocument();
      const attachmentsRow = screen
        .getAllByRole('row')
        .find((row) => within(row).queryByText('attachments'));
      const usersRow = screen
        .getAllByRole('row')
        .find((row) => within(row).queryByText('users'));
      const rolesRow = screen
        .getAllByRole('row')
        .find((row) => within(row).queryByText('roles'));
      expect(attachmentsRow).toBeInstanceOf(HTMLElement);
      expect(usersRow).toBeInstanceOf(HTMLElement);
      expect(rolesRow).toBeInstanceOf(HTMLElement);
      expect(
        within(attachmentsRow as HTMLElement).queryByRole('button', {
          name: '删除数据表 Attachments'
        })
      ).not.toBeInTheDocument();
      expect(
        within(usersRow as HTMLElement).queryByRole('button', {
          name: '删除数据表 用户'
        })
      ).not.toBeInTheDocument();
      expect(
        within(rolesRow as HTMLElement).queryByRole('button', {
          name: '删除数据表 角色'
        })
      ).not.toBeInTheDocument();
      expect(
        within(usersRow as HTMLElement).getByText('用户')
      ).toBeInTheDocument();
      expect(
        within(usersRow as HTMLElement).getByText('7')
      ).toBeInTheDocument();
      expect(
        within(rolesRow as HTMLElement).getByText('角色')
      ).toBeInTheDocument();
      expect(
        within(rolesRow as HTMLElement).getByText('6')
      ).toBeInTheDocument();
      expect(screen.getByLabelText('默认 Data Model 状态')).toBeEnabled();
      expect(screen.getByLabelText('默认 API 暴露状态')).toBeEnabled();

      fireEvent.click(
        within(rolesRow as HTMLElement).getByRole('button', { name: '编辑' })
      );
      expect(await screen.findByText('编辑 角色')).toBeInTheDocument();
      const editorDialog = await screen.findByRole('region', {
        name: 'Data Model 详情'
      });
      expect(
        within(editorDialog).getByRole('tab', { name: '字段' })
      ).toBeInTheDocument();
      expect(within(editorDialog).getByText('角色标识')).toBeInTheDocument();
      expect(
        within(editorDialog).getByText('默认成员角色')
      ).toBeInTheDocument();
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test(
    'selects a Data Model and exposes detail tabs with safe status controls',
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
      expect(within(detailSummary).getByText('contacts')).toBeInTheDocument();
      expect(
        within(detailSummary).getAllByTestId('data-model-summary-item')
      ).toHaveLength(6);
      expect(within(detailSummary).getByText('表 ID：')).toBeInTheDocument();
      expect(
        within(detailSummary).queryByText('状态：')
      ).not.toBeInTheDocument();
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
      const statusSelect = within(detailActions).getByRole('combobox', {
        name: /状态/
      });
      expect(statusSelect).toBeInTheDocument();
      const statusLabel = within(detailActions).getByTestId(
        'data-model-status-label'
      );
      expect(statusLabel).toHaveTextContent('状态：');
      expect(
        within(statusLabel).getByLabelText('Data Model 状态说明')
      ).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '关系' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '权限' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'API' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '记录预览' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: 'Advisor' })).toBeInTheDocument();

      fireEvent.mouseDown(statusSelect);
      expect(
        await screen.findByRole('option', { name: 'draft' })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('option', { name: 'published' })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('option', { name: 'disabled' })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('option', { name: 'broken' })
      ).toBeInTheDocument();
      expect(
        within(editorDialog).getByLabelText('Data Model 状态说明')
      ).toBeInTheDocument();
      expect(
        within(editorDialog).getByText(
          /broken: 当前定义、运行依赖或外部资源异常/
        )
      ).toBeInTheDocument();

      fireEvent.click(screen.getByRole('tab', { name: 'API' }));
      expect(
        await screen.findByText(i18nText('settings', 'auto.api_exposed_ready'))
      ).toBeInTheDocument();
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
      expect(
        screen.getByText(i18nText('settings', 'auto.runtime'))
      ).toBeInTheDocument();
      expect(
        screen.queryByRole('combobox', {
          name: i18nText('settings', 'auto.api_exposed_ready')
        })
      ).not.toBeInTheDocument();
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test('shows editable grants, record preview, and Advisor severities', async () => {
    renderApp('/settings/data-models?source=source-1');

    await openContactsDataModelEditor();
    fireEvent.click(screen.getByRole('tab', { name: '权限' }));
    expect(await screen.findByText('owner')).toBeInTheDocument();
    expect(screen.getByText('scope_all')).toBeInTheDocument();
    expect(screen.getByText('system_all')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '保存权限' }));
    await waitFor(() =>
      expect(dataModelsApi.updateSettingsDataModelScopeGrant).toHaveBeenCalled()
    );

    fireEvent.click(screen.getByRole('tab', { name: '记录预览' }));
    expect(await screen.findByText('person@example.com')).toBeInTheDocument();
    expect(
      dataModelsApi.fetchSettingsDataModelRecordPreview
    ).toHaveBeenCalledWith('contacts');

    fireEvent.click(screen.getByRole('tab', { name: 'Advisor' }));
    const advisorTab = await screen.findByTestId('data-model-advisor-tab');
    expect(within(advisorTab).getByText('blocking')).toBeInTheDocument();
    expect(within(advisorTab).getByText('high')).toBeInTheDocument();
    expect(within(advisorTab).getByText('info')).toBeInTheDocument();
  }, 20_000);

  test('creates Data Models from the data source section', async () => {
    renderApp('/settings/data-models?source=source-1');

    await screen.findByText(
      'Contacts',
      {},
      { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
    );
    fireEvent.click(screen.getByRole('button', { name: '新建数据表' }));
    const createDialog = await screen.findByRole('dialog', {
      name: '新建 Data Model'
    });
    expect(createDialog).toBeInTheDocument();
    expect(within(createDialog).getByLabelText('Code说明')).toBeInTheDocument();
    expect(
      within(createDialog).getByText(/Code: Data Model 的稳定标识/)
    ).toBeInTheDocument();
    expect(within(createDialog).getByLabelText('标题说明')).toBeInTheDocument();
    expect(
      within(createDialog).getByText(/标题: 管理台展示名称/)
    ).toBeInTheDocument();
    expect(within(createDialog).getByLabelText('状态说明')).toBeInTheDocument();
    expect(
      within(createDialog).getByText(/disabled: 已停用，不进入运行面/)
    ).toBeInTheDocument();
    const titleInput = within(createDialog).getByLabelText('标题');
    const codeInput = within(createDialog).getByLabelText('Code');
    expect(
      titleInput.compareDocumentPosition(codeInput) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

    fireEvent.change(screen.getByLabelText('Code'), {
      target: { value: 'companies' }
    });
    fireEvent.change(screen.getByLabelText('标题'), {
      target: { value: 'Companies' }
    });
    fireEvent.change(screen.getByLabelText('表 ID'), {
      target: { value: 'crm.companies' }
    });
    fireEvent.click(screen.getByRole('button', { name: '创建' }));

    await waitFor(() =>
      expect(dataModelsApi.createSettingsDataModel).toHaveBeenCalledWith(
        expect.objectContaining({
          scope_kind: 'workspace',
          code: 'companies',
          title: 'Companies',
          status: 'draft',
          data_source_instance_id: 'source-1',
          external_resource_key: 'crm.companies',
          external_table_id: 'crm.companies'
        }),
        'csrf-123'
      )
    );
    await waitFor(() =>
      expect(
        screen.queryByRole('dialog', { name: '新建 Data Model' })
      ).not.toBeInTheDocument()
    );
  }, 20_000);

  test('exposes Data Model editing from the detail drawer', async () => {
    renderApp('/settings/data-models?source=source-1');

    await screen.findByText(
      'Contacts',
      {},
      { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
    );
    const contactsRow = screen
      .getAllByRole('row')
      .find((row) => within(row).queryByText('Contacts'));
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
    expect(within(editorDialog).getByText('crm.contacts')).toBeInTheDocument();
    fireEvent.click(
      within(detailActions).getByRole('button', { name: /编\s*辑/ })
    );
    expect(await screen.findByDisplayValue('Contacts')).toBeInTheDocument();
    expect(screen.getByLabelText('Code')).toBeDisabled();
  }, 20_000);

  test('submits Data Model edits from the form drawer', async () => {
    const onUpdate = vi.fn();

    render(
      <DataModelFormDrawer
        open
        mode="edit"
        model={contactsModel}
        source={null}
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
    fireEvent.change(within(editDialog).getByDisplayValue('Contacts'), {
      target: { value: 'Customer Contacts' }
    });
    fireEvent.change(within(editDialog).getByDisplayValue('crm.contacts'), {
      target: { value: 'crm.contacts.v2' }
    });
    fireEvent.click(within(editDialog).getByRole('button', { name: '保存' }));

    await waitFor(() =>
      expect(onUpdate).toHaveBeenCalledWith(
        contactsModel,
        expect.objectContaining({
          title: 'Customer Contacts',
          status: 'published',
          external_table_id: 'crm.contacts.v2'
        })
      )
    );
  });

  test('deletes a Data Model from the table operation column after confirmation', async () => {
    renderApp('/settings/data-models?source=source-1');

    await screen.findByText(
      'Contacts',
      {},
      { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
    );
    const contactsRow = screen
      .getAllByRole('row')
      .find((row) => within(row).queryByText('Contacts'));
    expect(contactsRow).toBeInstanceOf(HTMLElement);

    fireEvent.click(
      within(contactsRow as HTMLElement).getByRole('button', {
        name: '删除数据表 Contacts'
      })
    );

    expect(await screen.findByText('确认删除数据表')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() =>
      expect(dataModelsApi.deleteSettingsDataModel).toHaveBeenCalledWith(
        'model-1',
        'csrf-123'
      )
    );
  }, 20_000);

  test('manages Data Model fields through the field drawer with delete confirmation', async () => {
    renderApp('/settings/data-models?source=source-1');

    const editorDialog = await openContactsDataModelEditor();
    fireEvent.click(
      within(editorDialog).getByRole('button', { name: '新增字段' })
    );
    expect(await screen.findByLabelText('字段 Code')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('字段 Code'), {
      target: { value: 'company_name' }
    });
    fireEvent.change(screen.getByLabelText('字段标题'), {
      target: { value: 'Company Name' }
    });
    fireEvent.change(screen.getByLabelText('外部字段映射 Key'), {
      target: { value: 'properties.company_name' }
    });
    fireEvent.click(screen.getByRole('checkbox', { name: '必填' }));
    fireEvent.click(screen.getByRole('button', { name: '创建字段' }));

    await waitFor(() =>
      expect(dataModelsApi.createSettingsDataModelField).toHaveBeenCalledWith(
        'model-1',
        expect.objectContaining({
          code: 'company_name',
          title: 'Company Name',
          external_field_key: 'properties.company_name',
          field_kind: 'string',
          is_required: true,
          is_unique: false,
          default_value: null,
          display_interface: 'input',
          display_options: {},
          relation_target_model_id: null,
          relation_options: {}
        }),
        'csrf-123'
      )
    );

    fireEvent.click(await screen.findByText('Email'));
    expect(await screen.findByText('编辑字段')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('字段标题'), {
      target: { value: 'Primary Email' }
    });
    fireEvent.click(screen.getByRole('button', { name: '保存字段' }));

    await waitFor(() =>
      expect(dataModelsApi.updateSettingsDataModelField).toHaveBeenCalledWith(
        'model-1',
        'field-1',
        expect.objectContaining({
          title: 'Primary Email',
          is_required: true,
          is_unique: true,
          display_interface: 'input',
          display_options: {},
          relation_options: {}
        }),
        'csrf-123'
      )
    );

    fireEvent.click(await screen.findByText('Email'));
    fireEvent.click(screen.getByRole('button', { name: '删除字段' }));
    expect(await screen.findByText('确认删除字段')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    await waitFor(() =>
      expect(dataModelsApi.deleteSettingsDataModelField).toHaveBeenCalledWith(
        'model-1',
        'field-1',
        'csrf-123'
      )
    );
  }, SLOW_SETTINGS_PAGE_TEST_TIMEOUT * 2);

  test('keeps main source field creation focused on basic field settings', async () => {
    renderApp('/settings/data-models?source=main_source');

    const editorDialog = await openDataModelEditorByTitle('Attachments');
    fireEvent.click(
      within(editorDialog).getByRole('button', { name: '新增字段' })
    );

    expect(await screen.findByLabelText('字段标题')).toBeInTheDocument();
    expect(screen.getByLabelText('字段 Code')).toBeInTheDocument();
    expect(screen.getByLabelText('字段类型')).toBeInTheDocument();
    expect(screen.getByLabelText('默认值')).toBeInTheDocument();
    expect(screen.queryByLabelText('外部字段映射 Key')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('目标数据表')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('关系配置 JSON')).not.toBeInTheDocument();
    expect(
      screen.queryByLabelText('显示控件配置 JSON')
    ).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('字段标题'), {
      target: { value: '状态' }
    });
    fireEvent.change(screen.getByLabelText('字段 Code'), {
      target: { value: 'status' }
    });
    fireEvent.click(screen.getByRole('button', { name: '创建字段' }));

    await waitFor(() =>
      expect(dataModelsApi.createSettingsDataModelField).toHaveBeenCalledWith(
        'model-attachments',
        expect.objectContaining({
          code: 'status',
          title: '状态',
          external_field_key: null,
          field_kind: 'string',
          default_value: null,
          display_interface: 'input',
          display_options: {},
          relation_target_model_id: null,
          relation_options: {}
        }),
        'csrf-123'
      )
    );
  }, 20_000);

  test('reveals enum, relation, external, and advanced field settings only when relevant', async () => {
    renderApp('/settings/data-models?source=source-1');

    const editorDialog = await openContactsDataModelEditor();
    fireEvent.click(
      within(editorDialog).getByRole('button', { name: '新增字段' })
    );

    expect(
      await screen.findByLabelText('外部字段映射 Key')
    ).toBeInTheDocument();
    expect(screen.queryByLabelText('显示格式')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('目标数据表')).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('字段标题'), {
      target: { value: '状态' }
    });
    fireEvent.change(screen.getByLabelText('字段 Code'), {
      target: { value: 'status' }
    });
    fireEvent.change(screen.getByLabelText('外部字段映射 Key'), {
      target: { value: 'properties.status' }
    });
    fireEvent.mouseDown(screen.getByLabelText('字段类型'));
    fireEvent.click(await screen.findByText('枚举'));
    expect(await screen.findByLabelText('显示格式')).toBeInTheDocument();
    expect(screen.getByLabelText('选项 1 显示值')).toBeInTheDocument();
    expect(screen.getByLabelText('选项 1 存储值')).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByLabelText('字段类型'));
    fireEvent.click(await screen.findByText('多对一关系'));
    expect(await screen.findByLabelText('目标数据表')).toBeInTheDocument();

    fireEvent.click(screen.getByText('高级显示设置'));
    expect(await screen.findByLabelText('显示控件')).toBeInTheDocument();
    expect(screen.getByLabelText('显示控件配置 JSON')).toBeInTheDocument();
    expect(screen.getByLabelText('关系配置 JSON')).toBeInTheDocument();
  }, 20_000);

  test(
    'creates enum fields with display format and label-value options',
    async () => {
      renderApp('/settings/data-models?source=source-1');

      const editorDialog = await openContactsDataModelEditor();
      fireEvent.click(
        within(editorDialog).getByRole('button', { name: '新增字段' })
      );

      fireEvent.change(await screen.findByLabelText('字段标题'), {
        target: { value: '状态' }
      });
      fireEvent.change(screen.getByLabelText('字段 Code'), {
        target: { value: 'status' }
      });
      fireEvent.change(screen.getByLabelText('外部字段映射 Key'), {
        target: { value: 'properties.status' }
      });
      expect(
        screen.queryByText('外部数据源里的字段路径，例如 properties.email。')
      ).not.toBeInTheDocument();
      fireEvent.mouseDown(screen.getByLabelText('字段类型'));
      fireEvent.click(await screen.findByText('枚举'));

      expect(await screen.findByLabelText('显示格式')).toBeInTheDocument();
      expect(screen.queryByLabelText('枚举选项说明')).not.toBeInTheDocument();
      expect(screen.getByLabelText('存储值说明')).toBeInTheDocument();
      expect(screen.getByLabelText('显示值说明')).toBeInTheDocument();
      expect(
        screen.queryByText(
          '显示值用于界面展示，存储值会写入数据库和 API payload。'
        )
      ).not.toBeInTheDocument();
      expect(
        screen
          .getByText('存储值')
          .compareDocumentPosition(screen.getByText('显示值')) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      expect(
        screen
          .getByRole('button', { name: '添加选项' })
          .compareDocumentPosition(screen.getByText('规则')) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      expect(
        screen
          .getByText('规则')
          .compareDocumentPosition(screen.getByText('默认值')) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      expect(screen.getByLabelText('选项 1 显示值')).toBeInTheDocument();
      expect(
        (screen.getByLabelText('选项 1 存储值') as HTMLInputElement).value
      ).toMatch(/^enum_[a-z0-9]{8}$/);

      fireEvent.mouseDown(screen.getByLabelText('显示格式'));
      fireEvent.click(await screen.findByText('多选下拉'));
      fireEvent.change(screen.getByLabelText('选项 1 显示值'), {
        target: { value: '草稿' }
      });
      fireEvent.change(screen.getByLabelText('选项 1 存储值'), {
        target: { value: 'draft' }
      });
      fireEvent.click(screen.getByRole('button', { name: '添加选项' }));
      expect(
        ((await screen.findByLabelText('选项 2 存储值')) as HTMLInputElement)
          .value
      ).toMatch(/^enum_[a-z0-9]{8}$/);
      fireEvent.change(await screen.findByLabelText('选项 2 显示值'), {
        target: { value: '已支付' }
      });
      fireEvent.change(screen.getByLabelText('选项 2 存储值'), {
        target: { value: 'paid' }
      });
      fireEvent.mouseDown(screen.getByLabelText('默认值'));
      fireEvent.click(await screen.findByText('草稿'));
      fireEvent.click(await screen.findByText('已支付'));
      fireEvent.click(screen.getByRole('button', { name: '创建字段' }));

      await waitFor(() =>
        expect(dataModelsApi.createSettingsDataModelField).toHaveBeenCalledWith(
          'model-1',
          expect.objectContaining({
            code: 'status',
            title: '状态',
            field_kind: 'enum',
            default_value: ['draft', 'paid'],
            display_interface: 'multi_select',
            display_options: {
              options: [
                { label: '草稿', value: 'draft' },
                { label: '已支付', value: 'paid' }
              ]
            }
          }),
          'csrf-123'
        )
      );
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT * 2
  );

});
