import {
  Alert,
  Button,
  Descriptions,
  Form,
  Input,
  Space,
  Spin,
  Typography
} from 'antd';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  decideMcpOAuthAuthorization,
  fetchMcpOAuthAuthorization,
  verifyMcpOAuthApiKey,
  type McpOAuthApproval,
  type McpOAuthAuthorization
} from '../../api/mcp-oauth';
import './mcp-authorization.css';

export function McpAuthorizePage({ requestId }: { requestId?: string }) {
  // Reset all transient credentials if a different authorization request is opened.
  return <AuthorizationForm key={requestId} requestId={requestId} />;
}

function AuthorizationForm({ requestId }: { requestId?: string }) {
  const { t } = useTranslation('auth');
  const [authorization, setAuthorization] =
    useState<McpOAuthAuthorization | null>(null);
  const [approval, setApproval] = useState<McpOAuthApproval | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [loading, setLoading] = useState(Boolean(requestId));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<'request' | 'key' | 'decision' | null>(
    requestId ? null : 'request'
  );
  const active = useRef(true);
  const submitting = useRef(false);

  useEffect(() => {
    active.current = true;
    const controller = new AbortController();
    if (requestId) {
      void fetchMcpOAuthAuthorization(requestId, controller.signal)
        .then((value) => {
          if (active.current && !controller.signal.aborted)
            setAuthorization(value);
        })
        .catch(() => {
          if (active.current && !controller.signal.aborted) setError('request');
        })
        .finally(() => {
          if (active.current && !controller.signal.aborted) setLoading(false);
        });
    }
    return () => {
      active.current = false;
      controller.abort();
    };
  }, [requestId]);

  const verify = async () => {
    if (!requestId || !apiKey.trim() || submitting.current) return;
    submitting.current = true;
    setBusy(true);
    setError(null);
    const key = apiKey.trim();
    setApiKey('');
    try {
      // Keep secrets out of query/mutation caches, URLs and browser storage.
      const result = await verifyMcpOAuthApiKey({
        request_id: requestId,
        api_key: key
      });
      if (active.current) setApproval(result);
    } catch {
      if (active.current) setError('key');
    } finally {
      submitting.current = false;
      if (active.current) setBusy(false);
    }
  };

  const decide = async (approved: boolean) => {
    if (!requestId || submitting.current || (approved && !approval)) return;
    submitting.current = true;
    setBusy(true);
    setError(null);
    setApiKey('');
    try {
      const result = await decideMcpOAuthAuthorization({
        request_id: requestId,
        ...(approved && approval
          ? { approval_token: approval.approval_token }
          : {}),
        approved
      });
      if (active.current) window.location.replace(result.redirect_uri);
    } catch {
      if (active.current) {
        setApproval(null);
        setError('decision');
        setBusy(false);
      }
      submitting.current = false;
    }
  };

  const errorMessage =
    error === 'request'
      ? t('mcp_oauth.request_error')
      : error === 'key'
        ? t('mcp_oauth.key_error')
        : t('mcp_oauth.decision_error');

  return (
    <main className="mcp-authorization-page">
      <section
        className="mcp-authorization-panel"
        aria-labelledby="mcp-authorization-title"
      >
        <Typography.Text className="mcp-authorization-brand">
          1flowbase
        </Typography.Text>
        <Typography.Title level={2} id="mcp-authorization-title">
          {t('mcp_oauth.title')}
        </Typography.Title>
        <Typography.Paragraph type="secondary">
          {t('mcp_oauth.introduction')}
        </Typography.Paragraph>
        {loading ? <Spin aria-label={t('mcp_oauth.loading')} /> : null}
        {error ? (
          <Alert type="error" showIcon role="alert" title={errorMessage} />
        ) : null}
        {authorization && !loading ? (
          <Space orientation="vertical" size="large" style={{ width: '100%' }}>
            <Descriptions
              column={1}
              size="small"
              items={[
                {
                  key: 'client',
                  label: t('mcp_oauth.client'),
                  children: authorization.client_name
                },
                {
                  key: 'instance',
                  label: t('mcp_oauth.instance'),
                  children: approval?.instance_name ?? authorization.instance_id
                },
                ...(approval
                  ? [
                      {
                        key: 'workspace',
                        label: t('mcp_oauth.workspace'),
                        children: approval.workspace_name
                      }
                    ]
                  : [])
              ]}
            />
            {approval ? (
              <>
                <Alert
                  showIcon
                  type="info"
                  title={t('mcp_oauth.permission_notice')}
                  description={t('mcp_oauth.lifecycle_notice')}
                />
                <Space wrap>
                  <Button
                    type="primary"
                    loading={busy}
                    disabled={busy}
                    onClick={() => void decide(true)}
                  >
                    {t('mcp_oauth.approve')}
                  </Button>
                  <Button
                    disabled={busy}
                    onClick={() => {
                      setApproval(null);
                      setError(null);
                    }}
                  >
                    {t('mcp_oauth.change_key')}
                  </Button>
                  <Button disabled={busy} onClick={() => void decide(false)}>
                    {t('mcp_oauth.deny')}
                  </Button>
                </Space>
              </>
            ) : (
              <Form layout="vertical" onFinish={() => void verify()}>
                <Form.Item
                  label={t('mcp_oauth.api_key')}
                  htmlFor="mcp-authorization-key"
                  extra={t('mcp_oauth.key_help')}
                >
                  <Input.Password
                    id="mcp-authorization-key"
                    value={apiKey}
                    onChange={(event) => setApiKey(event.target.value)}
                    autoComplete="off"
                    spellCheck={false}
                    disabled={busy}
                  />
                </Form.Item>
                <Space wrap>
                  <Button
                    type="primary"
                    htmlType="submit"
                    loading={busy}
                    disabled={busy || !apiKey.trim()}
                  >
                    {t('mcp_oauth.verify')}
                  </Button>
                  <Button disabled={busy} onClick={() => void decide(false)}>
                    {t('mcp_oauth.deny')}
                  </Button>
                </Space>
              </Form>
            )}
          </Space>
        ) : null}
      </section>
    </main>
  );
}
