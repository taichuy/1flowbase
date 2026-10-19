import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test } from 'vitest';

import { i18nText } from '../../../../shared/i18n/text';
import { SLOW_SETTINGS_PAGE_TEST_TIMEOUT, cleanupDataModelsPageTest, dataModelsApi, findDataModelsNavigation, renderApp, setDataModelsPageBreakpoint, setupDataModelsPageTest } from './support';

beforeEach(setupDataModelsPageTest);
afterEach(cleanupDataModelsPageTest);

describe('Settings data models page', () => {
  test('AC-002 presents main and other data sources in one table', async () => {
    renderApp('/settings/data-models');

    await waitFor(
      () => expect(dataModelsApi.fetchSettingsDataSources).toHaveBeenCalled(),
      { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
    );

    expect(
      await screen.findByText('主数据源', undefined, {
        timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT
      })
    ).toBeInTheDocument();
    const table = screen.getByRole('table', { name: '数据源列表' });
    expect(within(table).getByText('HubSpot')).toBeInTheDocument();
    expect(
      within(table).getByRole('columnheader', { name: '数据源名称' })
    ).toBeInTheDocument();
    expect(
      within(table).getByRole('columnheader', { name: '类型' })
    ).toBeInTheDocument();
    expect(
      within(table).getByRole('columnheader', { name: '状态' })
    ).toBeInTheDocument();
    expect(
      within(table).getByRole('columnheader', { name: '已启用' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: '主数据源' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: '外部连接' })
    ).not.toBeInTheDocument();
  });

  test('AC-004/005 runtime extension data sources map discovered resources without generic table-id input', async () => {
    renderApp('/settings/data-models?source=source-1');

    expect(
      await screen.findByRole(
        'heading',
        { name: '远端资源' },
        { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '新建数据表' })
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText('表 ID')).not.toBeInTheDocument();
  });

  test('AC-004 shows preview row fields and values in a closable resource drawer', async () => {
    dataModelsApi.previewSettingsDataSourceResource.mockResolvedValueOnce({
      preview_session_id: 'preview-1',
      expires_at: '2026-07-13T09:30:00Z',
      output: {
        rows: [
          {
            contact_id: 'contact-1',
            full_name: 'Ada Lovelace',
            active: true,
            profile: { tier: 'gold' }
          }
        ],
        next_cursor: null
      }
    });

    renderApp('/settings/data-models?source=source-1');

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: /预览/ },
        { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
      )
    );

    const previewDrawer = await screen.findByRole('dialog');
    expect(
      within(previewDrawer).getByText('Contacts 预览')
    ).toBeInTheDocument();
    expect(
      within(previewDrawer).getAllByText('contact_id').length
    ).toBeGreaterThan(0);
    expect(within(previewDrawer).getByText('contact-1')).toBeInTheDocument();
    expect(
      within(previewDrawer).getAllByText('full_name').length
    ).toBeGreaterThan(0);
    expect(within(previewDrawer).getByText('Ada Lovelace')).toBeInTheDocument();
    expect(within(previewDrawer).getByText('tier')).toBeInTheDocument();
    expect(within(previewDrawer).getByText('gold')).toBeInTheDocument();

    fireEvent.click(
      within(previewDrawer).getByRole('button', { name: '关闭' })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  });

  test('AC-004 shows an explicit empty state for a resource preview without rows', async () => {
    dataModelsApi.previewSettingsDataSourceResource.mockResolvedValueOnce({
      preview_session_id: 'preview-empty',
      expires_at: '2026-07-13T09:30:00Z',
      output: { rows: [], next_cursor: null }
    });

    renderApp('/settings/data-models?source=source-1');

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: /预览/ },
        { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
      )
    );

    const previewDrawer = await screen.findByRole('dialog');
    expect(
      within(previewDrawer).getByText('此资源没有可预览的数据')
    ).toBeInTheDocument();
    expect(
      within(previewDrawer).getByText('预览已返回 0 条记录')
    ).toBeInTheDocument();
  });

  test('AC-004 keeps preview request errors visible without opening stale rows', async () => {
    dataModelsApi.previewSettingsDataSourceResource.mockRejectedValueOnce(
      new Error('preview request failed')
    );

    renderApp('/settings/data-models?source=source-1');

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: /预览/ },
        { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
      )
    );

    expect(
      await screen.findByText('preview request failed')
    ).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  test('AC-004 presents preview rows as a vertical record list on mobile', async () => {
    setDataModelsPageBreakpoint({ md: false });
    dataModelsApi.previewSettingsDataSourceResource.mockResolvedValueOnce({
      preview_session_id: 'preview-mobile',
      expires_at: '2026-07-13T09:30:00Z',
      output: {
        rows: [{ contact_id: 'mobile-contact-1', full_name: 'Grace Hopper' }],
        next_cursor: null
      }
    });

    renderApp('/settings/data-models?source=source-1');
    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: /预览/ },
        { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
      )
    );

    const previewDrawer = await screen.findByRole('dialog');
    expect(
      within(previewDrawer).getByLabelText('移动端预览记录')
    ).toBeInTheDocument();
    expect(within(previewDrawer).getByText('第 1 条记录')).toBeInTheDocument();
    expect(within(previewDrawer).getByText('Grace Hopper')).toBeInTheDocument();
  });

  test.each([
    { layout: 'desktop', breakpoint: { md: true } },
    { layout: 'mobile', breakpoint: { md: false } }
  ])(
    'AC-004 caps rows, fields, keys, and scalar values on $layout',
    async ({ breakpoint }) => {
      setDataModelsPageBreakpoint(breakpoint);
      const longFieldKey = `long_field_${'k'.repeat(90)}_FIELD_KEY_TAIL`;
      const nestedLongFieldKey = `nested_field_${'n'.repeat(90)}_NESTED_KEY_TAIL`;
      const longValue = `long-value-${'v'.repeat(260)}-VALUE_TAIL`;
      const nestedLongValue = `nested-value-${'z'.repeat(260)}-NESTED_VALUE_TAIL`;
      const rows = Array.from({ length: 22 }, (_, rowIndex) =>
        Object.fromEntries([
          [longFieldKey, rowIndex === 0 ? longValue : `value-${rowIndex}`],
          [
            'nested',
            rowIndex === 0
              ? { [nestedLongFieldKey]: nestedLongValue }
              : { tier: 'gold' }
          ],
          ...Array.from({ length: 20 }, (_value, fieldIndex) => [
            `field_${fieldIndex}`,
            fieldIndex === 0 ? `row-${rowIndex}` : fieldIndex
          ])
        ])
      );
      dataModelsApi.previewSettingsDataSourceResource.mockResolvedValueOnce({
        preview_session_id: `preview-${breakpoint.md ? 'desktop' : 'mobile'}`,
        expires_at: '2026-07-13T09:30:00Z',
        output: { rows, next_cursor: null }
      });

      renderApp('/settings/data-models?source=source-1');
      fireEvent.click(
        await screen.findByRole(
          'button',
          { name: /预览/ },
          { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
        )
      );

      const previewDrawer = await screen.findByRole('dialog');
      expect(
        within(previewDrawer).getByText('另有 2 个字段未显示')
      ).toBeInTheDocument();
      expect(
        within(previewDrawer).getByText('另有 2 条记录未显示')
      ).toBeInTheDocument();
      expect(within(previewDrawer).getByText('row-19')).toBeInTheDocument();
      expect(
        within(previewDrawer).queryByText('row-20')
      ).not.toBeInTheDocument();
      expect(
        within(previewDrawer).queryByText(/FIELD_KEY_TAIL/)
      ).not.toBeInTheDocument();
      expect(
        within(previewDrawer).queryByText(/NESTED_KEY_TAIL/)
      ).not.toBeInTheDocument();
      expect(
        within(previewDrawer).queryByText(/VALUE_TAIL/)
      ).not.toBeInTheDocument();
      expect(
        within(previewDrawer).queryByText(/NESTED_VALUE_TAIL/)
      ).not.toBeInTheDocument();
      expect(
        within(previewDrawer).getAllByRole('button', {
          name: '复制完整值'
        }).length
      ).toBeGreaterThanOrEqual(2);
      expect(
        within(previewDrawer).getAllByText(
          `已省略 ${longValue.length - 240} 个字符`
        ).length
      ).toBeGreaterThan(0);
      expect(
        within(previewDrawer).getAllByText(
          `已省略 ${nestedLongValue.length - 240} 个字符`
        ).length
      ).toBeGreaterThan(0);
      expect(
        within(previewDrawer).getAllByText(
          `已省略 ${longFieldKey.length - 80} 个字符`
        ).length
      ).toBeGreaterThan(0);
      expect(
        within(previewDrawer).getAllByText(
          `已省略 ${nestedLongFieldKey.length - 80} 个字符`
        ).length
      ).toBeGreaterThan(0);
    }
  );

  test('AC-003 keeps data source config and secret after rejection, then closes after retry succeeds', async () => {
    dataModelsApi.fetchSettingsDataSourceCatalog.mockResolvedValueOnce({
      entries: [
        {
          installation_id: 'installation-1',
          source_code: 'hubspot',
          plugin_id: 'hubspot@1.0.0',
          plugin_version: '1.0.0',
          display_name: 'HubSpot',
          protocol: 'stdio_json',
          config_schema: [
            {
              key: 'endpoint',
              label: 'Endpoint',
              description: null,
              field_type: 'string',
              control: 'input',
              send_mode: 'value',
              required: true,
              placeholder: null,
              default_value: null,
              options: []
            },
            {
              key: 'api_key',
              label: 'API key',
              description: null,
              field_type: 'string',
              control: 'password',
              send_mode: 'secret_ref',
              required: true,
              placeholder: null,
              default_value: null,
              options: []
            }
          ]
        }
      ]
    });
    dataModelsApi.createSettingsDataSource
      .mockRejectedValueOnce(new Error('data source rejected'))
      .mockResolvedValueOnce({ id: 'source-new' });

    renderApp('/settings/data-models');

    await findDataModelsNavigation();
    await screen.findByText('主数据源', undefined, {
      timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT
    });

    const addDataSourceButton = await screen.findByRole(
      'button',
      { name: '新增数据源' },
      { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
    );
    await waitFor(() => expect(addDataSourceButton).toBeEnabled());
    fireEvent.click(addDataSourceButton);
    const createDrawer = await screen.findByRole('dialog');
    expect(within(createDrawer).getByText('新增数据源')).toBeInTheDocument();
    fireEvent.change(within(createDrawer).getByLabelText('数据源名称'), {
      target: { value: 'CRM primary' }
    });
    fireEvent.change(within(createDrawer).getByLabelText('Endpoint'), {
      target: { value: 'https://crm.example.com' }
    });
    fireEvent.change(within(createDrawer).getByLabelText('API key'), {
      target: { value: 'secret-value' }
    });

    fireEvent.click(screen.getByLabelText('创建'));

    expect(await screen.findByText('data source rejected')).toBeInTheDocument();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByLabelText('数据源名称')).toHaveValue('CRM primary');
    expect(screen.getByLabelText('Endpoint')).toHaveValue(
      'https://crm.example.com'
    );
    expect(screen.getByLabelText('API key')).toHaveValue('secret-value');

    fireEvent.click(screen.getByLabelText('创建'));
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  });

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
        within(hubSpotRow as HTMLElement).getByLabelText('HubSpot 已启用')
      ).toBeChecked();
      fireEvent.click(
        within(hubSpotRow as HTMLElement).getByRole('button', { name: '查看' })
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
      expect(screen.getByLabelText('新建默认开放 API')).toBeInTheDocument();
      expect(
        screen.queryByLabelText('默认 API 暴露状态')
      ).not.toBeInTheDocument();
      expect(
        screen.getByRole('heading', { name: 'HubSpot' })
      ).toBeInTheDocument();
      expect(
        screen.queryByRole('button', { name: '新建数据表' })
      ).not.toBeInTheDocument();
      expect(screen.getByText('数据表')).toBeInTheDocument();
      expect(
        screen.getByRole('columnheader', { name: 'API 状态' })
      ).toBeInTheDocument();
      expect(
        screen.queryByRole('columnheader', { name: '状态' })
      ).not.toBeInTheDocument();
      await waitFor(
        () => {
          const currentModelsTable = screen
            .getByRole('columnheader', { name: 'API 状态' })
            .closest('table');
          expect(currentModelsTable).toBeInstanceOf(HTMLTableElement);
          expect(
            within(currentModelsTable as HTMLTableElement).getByText('Contacts')
          ).toBeInTheDocument();
        },
        { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
      );
      const modelsTable = screen
        .getByRole('columnheader', { name: 'API 状态' })
        .closest('table');
      expect(modelsTable).toBeInstanceOf(HTMLTableElement);
      expect(
        within(modelsTable as HTMLTableElement).getAllByText('contacts').length
      ).toBeGreaterThan(0);
      const contactsRow = within(modelsTable as HTMLTableElement)
        .getAllByRole('row')
        .find((row) => within(row).queryByText('Contacts'));
      expect(contactsRow).toBeInstanceOf(HTMLElement);
      expect(
        within(contactsRow as HTMLElement).getByText('开放 API')
      ).toBeInTheDocument();
      expect(
        within(contactsRow as HTMLElement).queryByText(
          i18nText('settings', 'auto.published')
        )
      ).not.toBeInTheDocument();
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

  test(
    'uses backend capabilities for built-in model management entrances',
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
          name: '查看'
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
      const runtimeLogsRow = screen
        .getAllByRole('row')
        .find((row) => within(row).queryByText('runtime_logs'));
      expect(attachmentsRow).toBeInstanceOf(HTMLElement);
      expect(usersRow).toBeInstanceOf(HTMLElement);
      expect(rolesRow).toBeInstanceOf(HTMLElement);
      expect(runtimeLogsRow).toBeInstanceOf(HTMLElement);
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
        within(runtimeLogsRow as HTMLElement).queryByRole('button', {
          name: '删除数据表 Runtime Logs'
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
      expect(screen.getByLabelText('新建默认开放 API')).toBeEnabled();
      expect(
        screen.queryByLabelText('默认 API 暴露状态')
      ).not.toBeInTheDocument();

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
      const summaryRows = within(editorDialog).getAllByTestId(
        'data-model-summary-row'
      );
      expect(
        within(summaryRows[2]).getByText('分类表类型：')
      ).toBeInTheDocument();
      expect(within(summaryRows[3]).getByText('物理表：')).toBeInTheDocument();
      expect(within(summaryRows[4]).getByText('说明：')).toBeInTheDocument();
      expect(
        within(summaryRows[4]).getByTestId('data-model-summary-item')
      ).toHaveClass('data-model-panel__meta-card--full-row');

      const detailActions = within(editorDialog).getByTestId(
        'data-model-detail-actions'
      );
      fireEvent.click(
        within(detailActions).getByRole('button', { name: /编\s*辑/ })
      );
      expect(await screen.findByText('编辑 Data Model')).toBeInTheDocument();
      const apiSwitch = screen.getByRole('switch', { name: '开放 API' });
      expect(apiSwitch).toBeEnabled();
      expect(apiSwitch).toBeChecked();
      expect(screen.getByLabelText('标题')).toBeEnabled();
      expect(screen.queryByLabelText('数据源')).not.toBeInTheDocument();
      fireEvent.click(screen.getByRole('button', { name: '保存' }));
      await waitFor(() =>
        expect(dataModelsApi.updateSettingsDataModel).toHaveBeenCalledWith(
          'model-roles',
          { title: '角色', description: null, status: 'published' },
          'csrf-123'
        )
      );
    },
    SLOW_SETTINGS_PAGE_TEST_TIMEOUT
  );

});
