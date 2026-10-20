import { describe, test, expect, vi } from 'vitest';
import {
  DataModelFormDrawer,
  SLOW_SETTINGS_PAGE_TEST_TIMEOUT,
  type SettingsCompatibleDataModelTemplate,
  act,
  dataModelsApi,
  fireEvent,
  openContactsDataModelEditor,
  openDataModelEditorByTitle,
  render,
  renderApp,
  screen,
  waitFor,
  within
} from './data-models-page.fixtures';

describe('Settings data models page', () => {
  test('AC-002 does not fall back when the compatible template catalog is empty', async () => {
    const onCreate = vi.fn();

    render(
      <DataModelFormDrawer
        open
        mode="create"
        model={null}
        source={null}
        compatibleTemplates={[]}
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
    expect(
      within(createDialog).getByText('当前数据源没有可用的 Data Model 模板。')
    ).toBeInTheDocument();
    expect(
      within(createDialog).getByRole('button', { name: '创建' })
    ).toBeDisabled();
    expect(within(createDialog).getByLabelText('标题')).toBeEnabled();
    expect(
      within(createDialog).getByRole('button', { name: '取消' })
    ).toBeEnabled();
    expect(onCreate).not.toHaveBeenCalled();
  });

  test('AC-013 maps an external resource with the selected backend-compatible plugin template', async () => {
    let resolveCompatibleTemplates!: (
      templates: SettingsCompatibleDataModelTemplate[]
    ) => void;
    const compatibleTemplates = new Promise<
      SettingsCompatibleDataModelTemplate[]
    >((resolve) => {
      resolveCompatibleTemplates = resolve;
    });
    dataModelsApi.fetchSettingsCompatibleDataModelTemplates.mockImplementation(
      async (dataSourceId: string, resourceKey?: string) => {
        if (dataSourceId === 'source-1' && resourceKey === 'contacts') {
          return compatibleTemplates;
        }
        return [];
      }
    );
    renderApp('/settings/data-models?source=source-1');

    const mapButton = await screen.findByRole(
      'button',
      { name: '映射为 Data Model' },
      { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
    );
    fireEvent.click(mapButton);
    const templateSelector = await screen.findByRole('combobox', {
      name: 'Data Model 模板'
    });
    await waitFor(() =>
      expect(
        dataModelsApi.fetchSettingsCompatibleDataModelTemplates
      ).toHaveBeenCalledWith('source-1', 'contacts')
    );
    expect(templateSelector).toBeDisabled();
    expect(templateSelector).toHaveAttribute('aria-busy', 'true');
    expect(screen.queryByText('普通表')).not.toBeInTheDocument();
    expect(
      dataModelsApi.mapSettingsDataSourceResourceToModel
    ).not.toHaveBeenCalled();

    await act(async () => {
      resolveCompatibleTemplates([
        {
          template_provider: 'plugin.crm',
          template_code: 'contact_tree',
          template_version: 'v2',
          summary: '联系人树',
          description: '由 CRM 插件提供的联系人树。',
          system_fields: [
            {
              code: 'id',
              summary: 'Contact identifier',
              description: 'Stable external contact identifier.',
              field_kind: 'string',
              required: true
            }
          ]
        }
      ]);
      await compatibleTemplates;
    });
    await waitFor(() => expect(templateSelector).toBeEnabled());
    expect(templateSelector).toHaveAttribute('aria-busy', 'false');
    fireEvent.mouseDown(templateSelector);
    expect(
      await screen.findByRole('option', { name: '联系人树' })
    ).toHaveAccessibleName('联系人树');
    fireEvent.click(await screen.findByText('联系人树'));
    expect(
      screen.queryByText('由 CRM 插件提供的联系人树。')
    ).not.toBeInTheDocument();
    const confirmButtons = screen.getAllByRole('button', {
      name: '映射为 Data Model'
    });
    fireEvent.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() =>
      expect(
        dataModelsApi.mapSettingsDataSourceResourceToModel
      ).toHaveBeenCalledWith(
        'source-1',
        {
          resource_key: 'contacts',
          template_provider: 'plugin.crm',
          template_code: 'contact_tree',
          template_version: 'v2'
        },
        'csrf-123'
      )
    );
  });

  test('AC-013 keeps external mapping fail closed when the compatible catalog is empty', async () => {
    dataModelsApi.fetchSettingsCompatibleDataModelTemplates.mockResolvedValue(
      []
    );
    renderApp('/settings/data-models?source=source-1');

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: '映射为 Data Model' },
        { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
      )
    );

    expect(
      await screen.findByText('当前数据源没有可用的 Data Model 模板。')
    ).toBeInTheDocument();
    expect(
      screen.getByRole('combobox', { name: 'Data Model 模板' })
    ).toBeDisabled();
    expect(screen.queryByText('普通表')).not.toBeInTheDocument();
    const confirmButtons = screen.getAllByRole('button', {
      name: '映射为 Data Model'
    });
    expect(confirmButtons[confirmButtons.length - 1]).toBeDisabled();
    expect(
      dataModelsApi.mapSettingsDataSourceResourceToModel
    ).not.toHaveBeenCalled();
  });

  test('AC-013 keeps external mapping fail closed when the compatible catalog request fails', async () => {
    dataModelsApi.fetchSettingsCompatibleDataModelTemplates.mockRejectedValue(
      new Error('compatible catalog unavailable')
    );
    renderApp('/settings/data-models?source=source-1');

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: '映射为 Data Model' },
        { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
      )
    );

    expect(
      await screen.findByText('compatible catalog unavailable')
    ).toBeInTheDocument();
    expect(
      screen.getByRole('combobox', { name: 'Data Model 模板' })
    ).toBeDisabled();
    expect(screen.queryByText('普通表')).not.toBeInTheDocument();
    const confirmButtons = screen.getAllByRole('button', {
      name: '映射为 Data Model'
    });
    expect(confirmButtons[confirmButtons.length - 1]).toBeDisabled();
    expect(
      dataModelsApi.mapSettingsDataSourceResourceToModel
    ).not.toHaveBeenCalled();
  });

  test('deletes a Data Model from the table operation column after confirmation', async () => {
    renderApp('/settings/data-models?source=source-1');

    await screen.findAllByText(
      'Contacts',
      {},
      { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
    );
    const contactsRow = screen.getAllByRole('row').find(
      (row) =>
        within(row).queryByText('Contacts') &&
        within(row).queryByRole('button', {
          name: '删除数据表 Contacts'
        })
    );
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

  test(
    'manages Data Model fields through the field drawer with delete confirmation',
    async () => {
      renderApp('/settings/data-models?source=source-1');

      const editorDialog = await openContactsDataModelEditor();
      expect(
        within(editorDialog).getByText('Primary contact email')
      ).toBeInTheDocument();
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
      fireEvent.change(screen.getByLabelText('说明'), {
        target: { value: 'Company name from CRM' }
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
            description: 'Company name from CRM',
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
      fireEvent.change(screen.getByLabelText('说明'), {
        target: { value: 'Primary login email' }
      });
      fireEvent.click(screen.getByRole('button', { name: '保存字段' }));

      await waitFor(() =>
        expect(dataModelsApi.updateSettingsDataModelField).toHaveBeenCalledWith(
          'model-1',
          'field-1',
          expect.objectContaining({
            title: 'Primary Email',
            description: 'Primary login email',
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
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT * 2
  );

  test(
    'keeps system field physical metadata and deletion disabled by capability',
    async () => {
      renderApp('/settings/data-models?source=main');

      const editorDialog = await openDataModelEditorByTitle('Attachments');
      expect(
        within(editorDialog).getByText('附件原始文件名')
      ).toBeInTheDocument();
      fireEvent.click(within(editorDialog).getByText('文件名'));

      expect(await screen.findByText('编辑字段')).toBeInTheDocument();
      expect(screen.getByLabelText('字段标题')).toBeEnabled();
      expect(screen.getByLabelText('说明')).toBeEnabled();
      expect(screen.getByLabelText('字段 Code')).toBeDisabled();
      expect(screen.getByLabelText('字段类型')).toBeDisabled();
      expect(screen.getByRole('checkbox', { name: '必填' })).toBeDisabled();
      expect(screen.getByRole('checkbox', { name: '唯一' })).toBeDisabled();
      expect(screen.getByLabelText('默认值')).toBeDisabled();
      expect(screen.getByRole('button', { name: '删除字段' })).toBeDisabled();

      fireEvent.click(screen.getByRole('button', { name: '高级显示设置' }));
      expect(screen.getByLabelText('显示控件')).toBeEnabled();
      expect(screen.getByLabelText('显示控件配置 JSON')).toBeEnabled();
      fireEvent.change(screen.getByLabelText('说明'), {
        target: { value: '用户可见的附件文件名' }
      });
      fireEvent.click(screen.getByRole('button', { name: '保存字段' }));

      await waitFor(() =>
        expect(dataModelsApi.updateSettingsDataModelField).toHaveBeenCalledWith(
          'model-attachments',
          'attachment-name',
          expect.objectContaining({
            title: '文件名',
            description: '用户可见的附件文件名',
            is_required: true,
            is_unique: false,
            display_interface: 'input',
            display_options: {},
            relation_options: {}
          }),
          'csrf-123'
        )
      );
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test('keeps main source field creation focused on basic field settings', async () => {
    renderApp('/settings/data-models?source=main');

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
      const selectDropdownOption = async (name: string) => {
        await waitFor(() => {
          expect(
            screen.getByText(name, {
              selector: '.ant-select-item-option-content'
            })
          ).toBeInstanceOf(HTMLElement);
        });
        const option = screen.getByText(name, {
          selector: '.ant-select-item-option-content'
        });
        fireEvent.click(option);
      };
      fireEvent.mouseDown(screen.getByLabelText('默认值'));
      await selectDropdownOption('草稿');
      await selectDropdownOption('已支付');
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
