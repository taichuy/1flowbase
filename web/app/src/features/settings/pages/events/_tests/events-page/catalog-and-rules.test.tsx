import {
  access,
  api,
  clipboard,
  custom,
  declarations,
  documented,
  renderPage,
  schema
} from './fixtures';
import type { ApiEventOperationDetails } from '@1flowbase/api-client';
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

describe('event center configuration and observations', () => {
  test('catalog has business grouping and management columns and keeps source ownership in read-only details', async () => {
    api.listEventCatalog.mockResolvedValue([
      custom,
      {
        ...custom,
        id: null,
        name: 'Core ready',
        contract_id: 'core.ready',
        origin: 'core',
        enabled: null,
        owner_id: 'core.owner',
        scope: { kind: 'system' },
        source: {
          selection: { source_id: 'core.source', scope: { kind: 'system' } },
          point_id: 'core.point'
        }
      }
    ]);
    renderPage();
    await screen.findByRole('button', { name: 'Core ready' });
    expect(
      screen.getAllByRole('columnheader').map((node) => node.textContent)
    ).toEqual([
      'Name',
      'Business group',
      'Event ID',
      'Source category',
      'Event type',
      'Enabled',
      'Supported trigger phases',
      'Updated at',
      'Actions'
    ]);
    expect(screen.queryByText('Workspace · workspace')).not.toBeInTheDocument();
    expect(screen.queryByText('core.owner')).not.toBeInTheDocument();
    expect(screen.queryByText('core.point')).not.toBeInTheDocument();
    const row = screen.getByRole('row', { name: /Core ready/ });
    expect(within(row).getByRole('switch', { name: 'Enabled' })).toBeChecked();
    expect(within(row).getByRole('switch', { name: 'Enabled' })).toBeDisabled();
    expect(
      within(row).queryByRole('button', { name: 'Edit' })
    ).not.toBeInTheDocument();
    await act(async () => {
      fireEvent.click(within(row).getByRole('button', { name: 'Details' }));
    });
    const drawer = await screen.findByRole('dialog');
    const schemaEditor = await within(drawer).findByTestId('schema-json-editor');
    expect(schemaEditor).toHaveAttribute('data-readonly', 'true');
    expect(JSON.parse(schemaEditor.textContent ?? 'null')).toEqual(schema);
    const scopeRow = within(drawer)
      .getByText('Scope', {
        selector: '.ant-descriptions-item-label'
      })
      .closest('tr')!;
    expect(within(scopeRow).getByText('System')).toBeInTheDocument();
    expect(
      within(drawer).queryByText('Workspace · workspace')
    ).not.toBeInTheDocument();
    expect(within(drawer).getByText('core.owner')).toBeInTheDocument();
    expect(within(drawer).getByText('core.point')).toBeInTheDocument();
    expect(
      within(drawer).getByRole('switch', { name: 'Enabled' })
    ).toBeChecked();
    expect(
      within(drawer).getByRole('switch', { name: 'Enabled' })
    ).toBeDisabled();
    expect(
      within(drawer).queryByText('Contract version')
    ).not.toBeInTheDocument();
  });

  test.each(['core', 'plugin'] as const)(
    '%s events show an enabled disabled switch and never issue updates',
    async (origin) => {
      api.listEventCatalog.mockResolvedValue([
        {
          ...custom,
          id: null,
          origin,
          enabled: null,
          owner_id: `${origin}.owner`
        }
      ]);
      renderPage();
      const row = await screen.findByRole('row', { name: /Order/ });
      const toggle = within(row).getByRole('switch', { name: 'Enabled' });
      expect(toggle).toBeChecked();
      expect(toggle).toBeDisabled();
      fireEvent.click(toggle);
      expect(api.updateEventDefinition).not.toHaveBeenCalled();
    }
  );

  test('same-contract subtype leaves and user definitions remain distinct rows', async () => {
    const source = {
      source_id: 'business.facts',
      scope: { kind: 'system' as const }
    };
    api.listEventCatalog.mockResolvedValue([
      ...['reserved', 'settled'].map((kind) => ({
        ...custom,
        id: null,
        name: kind,
        origin: 'plugin',
        group_path: ['billing', kind],
        source: {
          point_id: 'business.facts',
          selection: { ...source, filters: { kind } }
        }
      })),
      custom,
      { ...custom, id: 'another-definition', name: 'Another rule' }
    ]);
    renderPage();
    for (const name of ['reserved', 'settled', 'Order', 'Another rule'])
      expect(await screen.findByRole('button', { name })).toBeInTheDocument();
    expect(screen.getAllByRole('row')).toHaveLength(5);
    fireEvent.click(screen.getByRole('button', { name: 'settled' }));
    expect(
      within(screen.getByRole('dialog')).getByText('settled', {
        selector: '.ant-descriptions-item-content'
      })
    ).toBeInTheDocument();
  });
  test('toggles a custom event both ways, preserves its definition and waits for the server', async () => {
    let resolveUpdate!: (value: object) => void;
    api.updateEventDefinition.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveUpdate = resolve;
        })
    );
    renderPage();
    const row = await screen.findByRole('row', { name: /Order/ });
    const toggle = within(row).getByRole('switch', { name: 'Enabled' });
    expect(toggle).not.toBeChecked();
    expect(toggle).toBeEnabled();
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenCalledWith(
        'definition',
        {
          name: 'Order',
          description: 'New order',
          interface_event: custom.interface_event,
          event_kind: 'interface',
          enabled: true
        },
        'csrf'
      )
    );
    expect(toggle).toBeDisabled();
    expect(toggle).not.toBeChecked();
    api.listEventCatalog.mockResolvedValue([{ ...custom, enabled: true }]);
    await act(async () => resolveUpdate({ id: 'definition' }));
    await waitFor(() => expect(toggle).toBeChecked());
    await waitFor(() => expect(toggle).toBeEnabled());
    api.listEventCatalog.mockResolvedValue([custom]);
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenLastCalledWith(
        'definition',
        {
          name: 'Order',
          description: 'New order',
          interface_event: custom.interface_event,
          event_kind: 'interface',
          enabled: false
        },
        'csrf'
      )
    );
    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  test('a failed toggle keeps the server state and shows an error', async () => {
    api.updateEventDefinition.mockRejectedValueOnce(new Error('update failed'));
    renderPage();
    const row = await screen.findByRole('row', { name: /Order/ });
    const toggle = within(row).getByRole('switch', { name: 'Enabled' });
    fireEvent.click(toggle);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The event could not be saved.'
    );
    expect(toggle).not.toBeChecked();
    expect(toggle).toBeEnabled();
  });

  test('custom event switches respect update permissions', async () => {
    access.operations.delete('events.definitions.update');
    renderPage();
    const row = await screen.findByRole('row', { name: /Order/ });
    const toggle = within(row).getByRole('switch', { name: 'Enabled' });
    expect(toggle).not.toBeChecked();
    expect(toggle).toBeDisabled();
    fireEvent.click(toggle);
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });

  test.each([schema, {}, true, false, null])(
    'previews and copies the complete read-only schema %j',
    async (payload_schema) => {
      api.listEventCatalog.mockResolvedValue([
        {
          ...custom,
          origin: 'core',
          enabled: null,
          owner_id: 'core.owner',
          id: null,
          payload_schema
        }
      ]);
      renderPage();
      const row = await screen.findByRole('row', { name: /Order/ });
      fireEvent.click(within(row).getByRole('button', { name: 'Details' }));
      const editor = await screen.findByTestId('schema-json-editor');
      expect(JSON.parse(editor.textContent ?? '')).toEqual(payload_schema);
      expect(editor).toHaveAttribute('data-readonly', 'true');
      expect(
        screen.queryByRole('button', { name: 'Save' })
      ).not.toBeInTheDocument();
      fireEvent.click(
        screen.getByRole('button', { name: 'Copy Payload schema JSON' })
      );
      await waitFor(() =>
        expect(clipboard.copy).toHaveBeenCalledWith(
          JSON.stringify(payload_schema, null, 2)
        )
      );
      fireEvent.click(
        screen.getByRole('button', {
          name: 'Zoom in to view Payload schema JSON'
        })
      );
      const enlarged = await screen.findByRole('dialog', {
        name: 'Payload schema JSON'
      });
      expect(
        JSON.parse(
          within(enlarged).getByTestId('schema-json-editor').textContent ?? ''
        )
      ).toEqual(payload_schema);
    }
  );

  test('opens event details with a single direct row action click', async () => {
    api.listEventCatalog.mockResolvedValue([
      {
        ...custom,
        origin: 'core',
        enabled: null,
        owner_id: 'core.owner',
        id: null
      }
    ]);
    renderPage();
    const view = await screen.findByRole('button', { name: 'Details' });
    expect(
      screen.queryByRole('button', { name: 'Actions' })
    ).not.toBeInTheDocument();
    fireEvent.click(view);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });
  test('shared column settings change the visible fields and persist the selection', async () => {
    access.operations.add('patch_me_meta');
    const view = renderPage();
    await screen.findByRole('button', { name: 'Order' });
    fireEvent.click(
      screen.getByRole('button', { name: 'Field configuration' })
    );
    const fields = await screen.findByRole('dialog', {
      name: 'Customize visible fields'
    });
    fireEvent.click(
      within(fields).getByRole('checkbox', { name: 'Event type' })
    );
    await waitFor(() =>
      expect(
        screen.queryByRole('columnheader', { name: 'Event type' })
      ).not.toBeInTheDocument()
    );
    view.unmount();
    renderPage();
    await screen.findByRole('button', { name: 'Order' });
    expect(
      screen.queryByRole('columnheader', { name: 'Event type' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: 'Event ID' })
    ).toBeInTheDocument();
  });
  test.each(['before', 'completion'] as const)(
    'creates an interface rule for %s with inherited schema and no free-form contract',
    async (phase) => {
      api.listEventCatalog.mockResolvedValue([custom, ...declarations]);
      api.listApiEventOperations.mockResolvedValue([documented]);
      renderPage();
      fireEvent.click(
        await screen.findByRole('button', { name: 'Create event' })
      );
      const drawer = screen.getByRole('dialog');
      expect(
        within(drawer).queryByLabelText('Event ID')
      ).not.toBeInTheDocument();
      expect(
        within(drawer).queryByRole('button', { name: 'Add schema field' })
      ).not.toBeInTheDocument();
      fireEvent.change(within(drawer).getByLabelText('Name'), {
        target: { value: 'Request' }
      });
      fireEvent.mouseDown(
        within(drawer).getByRole('combobox', { name: 'Interface / operation' })
      );
      fireEvent.click(
        await screen.findByText('POST /api/console/orders · Create order')
      );
      await waitFor(() =>
        expect(api.getApiEventOperation).toHaveBeenCalledWith('create_order')
      );
      await waitFor(() =>
        expect(
          within(drawer).getByRole('combobox', { name: 'Interface phase' })
        ).toBeEnabled()
      );
      fireEvent.mouseDown(
        within(drawer).getByRole('combobox', { name: 'Interface phase' })
      );
      fireEvent.click(
        await screen.findByText(
          phase === 'before' ? 'Before request' : 'After completion'
        )
      );
      const editor = await within(drawer).findByTestId('schema-json-editor');
      expect(editor).toHaveAttribute('data-readonly', 'true');
      expect(JSON.parse(editor.textContent ?? '')).toEqual(
        declarations.find((item) => item.interface_event?.phase === phase)
          ?.payload_schema
      );
      if (phase === 'completion') {
        const results = within(drawer).getByRole('combobox', {
          name: 'Terminal results'
        });
        for (const label of ['Completed', 'Failed', 'Rejected', 'Cancelled']) {
          fireEvent.mouseDown(results);
          fireEvent.click(
            await screen.findByText(label, {
              selector: '.ant-select-item-option-content'
            })
          );
        }
        fireEvent.keyDown(results, { key: 'Escape' });
      }
      fireEvent.click(within(drawer).getByRole('switch', { name: 'Enabled' }));
      await waitFor(() =>
        expect(
          within(drawer).getByRole('button', { name: 'Save' })
        ).toBeEnabled()
      );
      fireEvent.click(within(drawer).getByRole('button', { name: 'Save' }));
      await waitFor(() =>
        expect(api.createEventDefinition).toHaveBeenCalledWith(
          {
            name: 'Request',
            description: '',
            enabled: false,
            event_kind: 'interface',
            interface_event: {
              operation_id: 'create_order',
              phase,
              ...(phase === 'completion'
                ? { results: ['completed', 'failed', 'rejected', 'cancelled'] }
                : {})
            }
          },
          'csrf'
        )
      );
      expect(api.createEventDefinition.mock.calls[0][0]).not.toHaveProperty(
        'payload_schema'
      );
      expect(api.createEventDefinition.mock.calls[0][0]).not.toHaveProperty(
        'contract_id'
      );
    }
  );
  test('new rules cannot invent interfaces when the declaration catalog is empty', async () => {
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Create event' })
    );
    expect(
      within(screen.getByRole('dialog')).getByRole('button', { name: 'Save' })
    ).toBeDisabled();
    expect(api.createEventDefinition).not.toHaveBeenCalled();
  });
  test('custom event details stay read-only even when editing is authorized', async () => {
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Details' }));
    expect(
      JSON.parse(
        (await screen.findByTestId('schema-json-editor')).textContent ?? ''
      )
    ).toEqual(schema);
    expect(
      screen.queryByRole('button', { name: 'Save' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Add schema field' })
    ).not.toBeInTheDocument();
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });
  test('editing a disabled interface rule inherits schema and sends the binding', async () => {
    api.listEventCatalog.mockResolvedValue([custom, ...declarations]);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    const drawer = screen.getByRole('dialog');
    expect(within(drawer).queryByLabelText('Event ID')).not.toBeInTheDocument();
    expect(
      within(drawer).queryByLabelText('Contract version')
    ).not.toBeInTheDocument();
    fireEvent.change(within(drawer).getByLabelText('Name'), {
      target: { value: 'Changed order' }
    });
    await waitFor(() =>
      expect(within(drawer).getByRole('button', { name: 'Save' })).toBeEnabled()
    );
    fireEvent.click(within(drawer).getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenCalledWith(
        'definition',
        {
          name: 'Changed order',
          description: 'New order',
          enabled: false,
          event_kind: 'interface',
          interface_event: custom.interface_event
        },
        'csrf'
      )
    );
  });
  test('late save completion cannot close a newer event draft', async () => {
    let finishPrevious!: (value: { id: string }) => void;
    api.updateEventDefinition.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishPrevious = resolve;
        })
    );
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    const previousDrawer = screen.getByRole('dialog');
    fireEvent.change(within(previousDrawer).getByLabelText('Name'), {
      target: { value: 'Saved previous order' }
    });
    fireEvent.click(
      within(previousDrawer).getByRole('button', { name: 'Save' })
    );
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenCalledWith(
        'definition',
        expect.objectContaining({ name: 'Saved previous order' }),
        'csrf'
      )
    );
    // The drawer close control remains available while its form is saving.
    fireEvent.click(
      within(previousDrawer).getByLabelText('Close', { selector: 'button' })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    fireEvent.click(screen.getByRole('button', { name: 'Create event' }));
    fireEvent.change(
      within(screen.getByRole('dialog')).getByLabelText('Name'),
      {
        target: { value: 'New unsaved event' }
      }
    );
    await act(async () => finishPrevious({ id: 'definition' }));
    await waitFor(() => expect(screen.getByRole('dialog')).toBeVisible());
    expect(
      within(screen.getByRole('dialog')).getByLabelText('Name')
    ).toHaveValue('New unsaved event');
    expect(api.createEventDefinition).not.toHaveBeenCalled();
  });

  test('complete API discovery supports method path and name search and shows genuine phase restrictions', async () => {
    const restricted: ApiEventOperationDetails = {
      ...documented,
      operation_id: 'download_report',
      method: 'GET',
      path: '/api/console/reports/{id}',
      name: 'Download report',
      completion: { status: 'unavailable', reason: 'binary_response' },
      completion_schema: null
    };
    api.listApiEventOperations.mockResolvedValue([documented, restricted]);
    let resolveDetails!: (value: ApiEventOperationDetails) => void;
    const pendingDetails = new Promise<ApiEventOperationDetails>((resolve) => {
      resolveDetails = resolve;
    });
    api.getApiEventOperation.mockImplementation((id: string) =>
      id === restricted.operation_id
        ? pendingDetails
        : Promise.resolve(documented)
    );
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Create event' })
    );
    const drawer = screen.getByRole('dialog');
    const select = within(drawer).getByRole('combobox', {
      name: 'Interface / operation'
    });
    fireEvent.mouseDown(select);
    fireEvent.change(select, { target: { value: 'reports' } });
    fireEvent.click(
      await screen.findByText('GET /api/console/reports/{id} · Download report', {
        selector: '.ant-select-item-option-content'
      })
    );
    expect(
      screen.queryByText('interfaces / api-server')
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(api.getApiEventOperation).toHaveBeenCalledWith(
        restricted.operation_id
      )
    );
    expect(
      within(drawer).getByRole('combobox', { name: 'Interface phase' })
    ).toBeDisabled();
    await act(async () => {
      resolveDetails(restricted);
      await pendingDetails;
    });
    await waitFor(() =>
      expect(
        within(drawer).getByRole('combobox', { name: 'Interface phase' })
      ).toBeEnabled()
    );
    fireEvent.mouseDown(
      within(drawer).getByRole('combobox', { name: 'Interface phase' })
    );
    fireEvent.click(await screen.findByText('After completion'));
    expect(
      await within(drawer).findByText(
        'This phase returns binary content without an event result contract.'
      )
    ).toBeInTheDocument();
    expect(within(drawer).getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(api.createEventDefinition).not.toHaveBeenCalled();
  });
  test('existing legacy rules retain their stored schema even after the source catalog changes', async () => {
    api.listEventCatalog.mockResolvedValue([custom]);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    const drawer = screen.getByRole('dialog');
    expect(
      await within(drawer).findByText(
        'This existing rule keeps its original interface binding and payload schema.'
      )
    ).toBeInTheDocument();
    expect(
      JSON.parse(
        (await within(drawer).findByTestId('schema-json-editor')).textContent ??
          ''
      )
    ).toEqual(custom.payload_schema);
    expect(api.listApiEventOperations).not.toHaveBeenCalled();
    fireEvent.change(within(drawer).getByLabelText('Name'), {
      target: { value: 'Preserved legacy' }
    });
    fireEvent.click(within(drawer).getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(api.updateEventDefinition).toHaveBeenCalledWith(
        'definition',
        {
          name: 'Preserved legacy',
          description: custom.description,
          enabled: false,
          event_kind: 'interface',
          interface_event: custom.interface_event
        },
        'csrf'
      )
    );
  });
  test('registered source contracts are read-only and do not fabricate supported trigger phases', async () => {
    api.listEventCatalog.mockResolvedValue([
      {
        ...custom,
        id: null,
        origin: 'plugin',
        enabled: null,
        owner_id: 'plugin.owner',
        name: 'Plugin event',
        supported_phases: []
      }
    ]);
    renderPage();
    expect(
      await screen.findByText('No source adapter available')
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Plugin event' }));
    const drawer = screen.getByRole('dialog');
    expect(
      within(drawer).queryByRole('button', { name: 'Save' })
    ).not.toBeInTheDocument();
    expect(within(drawer).getByText('plugin.owner')).toBeInTheDocument();
    expect(
      within(drawer).getByText('Workspace · workspace')
    ).toBeInTheDocument();
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });
  test('shows a missing schema as literal null without inventing an accept-all schema', async () => {
    api.listEventCatalog.mockResolvedValue([
      {
        ...custom,
        id: null,
        origin: 'core',
        name: 'Source without schema',
        payload_schema: null,
        supported_phases: []
      }
    ]);
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Source without schema' })
    );
    const drawer = screen.getByRole('dialog');
    expect(
      await within(drawer).findByTestId('schema-json-editor')
    ).toHaveTextContent('null');
    expect(
      within(drawer).queryByText(
        'This source has not declared a payload schema.'
      )
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: 'Save' })
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: 'Add schema field' })
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByText(/accepts every payload/)
    ).not.toBeInTheDocument();
    expect(
      within(drawer).getByRole('button', {
        name: 'Copy Payload schema JSON'
      })
    ).toBeInTheDocument();
    expect(api.createEventDefinition).not.toHaveBeenCalled();
    expect(api.updateEventDefinition).not.toHaveBeenCalled();
  });
  test('a catalog-only observer receives no create/edit or runtime actions', async () => {
    access.operations = new Set(['events.catalog.list']);
    renderPage();
    expect(
      await screen.findByRole('button', { name: 'Order' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Create event' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Edit' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Current queues' })
    ).not.toBeInTheDocument();
  });
});
