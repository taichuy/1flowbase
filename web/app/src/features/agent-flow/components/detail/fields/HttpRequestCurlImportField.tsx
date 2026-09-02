import ImportOutlined from '@ant-design/icons/es/icons/ImportOutlined';
import { App, Button, Modal, Input } from 'antd';
import { useState } from 'react';
import type { FlowBinding } from '@1flowbase/flow-schema';

import { parseHttpRequestCurlCommand } from '../../../lib/http-request/curl';
import type { HttpRequestBodyType } from '../../../lib/http-request/contract';
import { toNamedBinding } from './HttpRequestKeyValuesField';
import { i18nText } from '../../../../../shared/i18n/text';

function toTemplatedEntries(entries: Array<{ name: string; value: string }>) {
  return entries.map((entry) => ({
    name: entry.name,
    value: { kind: 'templated_text' as const, value: entry.value }
  }));
}

function toBodyBinding(value: string): FlowBinding {
  return { kind: 'templated_text', value };
}

export function HttpRequestCurlImportField({
  onMethodChange,
  onUrlChange,
  onBodyTypeChange,
  onParamsChange,
  onHeadersChange,
  onBodyChange
}: {
  onMethodChange: (value: string) => void;
  onUrlChange: (value: string) => void;
  onBodyTypeChange: (value: HttpRequestBodyType) => void;
  onParamsChange: (value: FlowBinding) => void;
  onHeadersChange: (value: FlowBinding) => void;
  onBodyChange: (value: FlowBinding) => void;
}) {
  const { message } = App.useApp();
  const [open, setOpen] = useState(false);
  const [command, setCommand] = useState('');

  function handleImport() {
    const parsed = parseHttpRequestCurlCommand(command);

    if (!parsed.url) {
      message.error(i18nText('agentFlow', 'auto.curl_command_missing_url'));
      return;
    }

    onMethodChange(parsed.method);
    onUrlChange(parsed.url);
    onBodyTypeChange(parsed.bodyType);
    onParamsChange(toNamedBinding(toTemplatedEntries(parsed.params)));
    onHeadersChange(toNamedBinding(toTemplatedEntries(parsed.headers)));
    onBodyChange(toBodyBinding(parsed.body));
    setOpen(false);
  }

  return (
    <>
      <div className="agent-flow-http-request-curl-import">
        <Button
          aria-label={i18nText('agentFlow', 'auto.import_curl')}
          icon={<ImportOutlined />}
          onClick={() => setOpen(true)}
        >
          {i18nText('agentFlow', 'auto.import_curl')}
        </Button>
      </div>
      <Modal
        destroyOnHidden
        okText={i18nText('agentFlow', 'auto.import_request')}
        open={open}
        title={i18nText('agentFlow', 'auto.import_curl')}
        onCancel={() => setOpen(false)}
        onOk={handleImport}
      >
        <Input.TextArea
          aria-label={i18nText('agentFlow', 'auto.curl_command')}
          autoSize={{ minRows: 5, maxRows: 10 }}
          value={command}
          onChange={(event) => setCommand(event.target.value)}
        />
      </Modal>
    </>
  );
}
