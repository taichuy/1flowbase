import '@ant-design/v5-patch-for-react-19';
import { ConfigProvider } from 'antd';
import { StrictMode, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';

import type { PublicLoginInstance } from '../../api/session';
import { PublicAuthBlock } from '../../components/PublicAuthBlock';

const source = `
  import { useState } from 'react';
  import { Alert, Button, Input, Space } from 'antd';
  export default function PublicAuthFixture({ ctx }) {
    const [mode, setMode] = useState('sign_in');
    const [error, setError] = useState(null);
    const [localCount, setLocalCount] = useState(0);
    const submit = async (values) => {
      setError(null);
      try {
        await ctx.api.post(
          mode === 'sign_in'
            ? '/api/public/auth/sign-in'
            : '/api/public/auth/sign-up',
          { body: { authenticator_id: ctx.inputs.authenticator_id, ...values } }
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : 'Authentication failed');
      }
    };
    return <Space
      data-testid="public-auth-native-content"
      data-local-count={localCount}
      direction="vertical"
      size="middle"
      style={{ width: '100%' }}
    >
      <style>{\`:host { --auth-fixture-accent: rgb(22, 119, 255); }
        .auth-fixture-heading { color: var(--auth-fixture-accent); }\`}</style>
      <h2>
        <span className="auth-fixture-heading">
          {mode === 'sign_in' ? 'Sign in' : 'Create an account'}
        </span>
      </h2>
      {error ? <Alert type="error" showIcon message={error} /> : null}
      <form onSubmit={(event) => { event.preventDefault(); void submit({}); }}>
        <label>Account or email<Input autoComplete="username" /></label>
        {mode === 'sign_up' ? (
          <label>Email<Input type="email" /></label>
        ) : null}
        <label>Password<Input type="password" /></label>
        <Button htmlType="submit" type="primary" block>
          {mode === 'sign_in' ? 'Sign in' : 'Register'}
        </Button>
      </form>
      {ctx.inputs.public_variables.self_registration_enabled ? (
        <Button type="link" onClick={() => setMode(mode === 'sign_in' ? 'sign_up' : 'sign_in')}>
          {mode === 'sign_in' ? 'Create an account' : 'Back to sign in'}
        </Button>
      ) : null}
      <Button onClick={() => setLocalCount((value) => value + 1)}>
        local state {localCount}
      </Button>
    </Space>;
  }
`;

const legacySource = `
import type { BlockContext, BlockModule, BlockResult } from '@1flowbase/block-sdk';
async function main(ctx: BlockContext): Promise<BlockResult> {
  return { view: { type: 'text', value: String(ctx.inputs) }, outputs: {} };
}
export default { main } satisfies BlockModule;
`;

let fixtureRenderCount = 0;

function Fixture() {
  fixtureRenderCount += 1;
  const [completion, setCompletion] = useState('idle');
  const [showLegacy, setShowLegacy] = useState(false);
  const [pageMounted, setPageMounted] = useState(true);
  const activeSource = showLegacy ? legacySource : source;
  const instance = useMemo<PublicLoginInstance>(
    () => ({
      id: 'auth-password-local',
      auth_type: 'password-local',
      is_builtin: true,
      title: 'Password',
      description: null,
      sort_order: 0,
      public_ui_block: activeSource,
      public_variables: { self_registration_enabled: true }
    }),
    [activeSource]
  );
  return (
    <main
      data-testid="public-auth-native-fixture"
      data-viewport={window.innerWidth <= 390 ? 'mobile-390' : 'desktop'}
      data-source-mode={showLegacy ? 'legacy' : 'native'}
      data-page-mounted={pageMounted ? 'true' : 'false'}
      data-fixture-render-count={fixtureRenderCount}
      data-auth-completion={completion}
      data-legacy-source-preserved={
        showLegacy && activeSource === legacySource ? 'true' : 'false'
      }
      style={{ width: 'min(100% - 32px, 440px)', margin: '40px auto' }}
    >
      {pageMounted ? (
        <PublicAuthBlock
          instance={instance}
          onAuthenticated={() => setCompletion('authenticated')}
        />
      ) : null}
      <button type="button" onClick={() => setShowLegacy((value) => !value)}>
        {showLegacy ? 'show native source' : 'show legacy source'}
      </button>
      <button type="button" onClick={() => setPageMounted((value) => !value)}>
        {pageMounted ? 'exit auth page' : 'enter auth page'}
      </button>
      <output data-testid="public-auth-completion">{completion}</output>
    </main>
  );
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ConfigProvider>
      <Fixture />
    </ConfigProvider>
  </StrictMode>
);
