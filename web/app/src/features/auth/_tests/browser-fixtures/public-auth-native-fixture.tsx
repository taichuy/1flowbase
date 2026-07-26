import '@ant-design/v5-patch-for-react-19';
import { ConfigProvider } from 'antd';
import { StrictMode, useState } from 'react';
import { createRoot } from 'react-dom/client';

import { PublicAuthBlock } from '../../components/PublicAuthBlock';

const source = `
  import { useState } from 'react';
  import { Alert, Button, Input, Space } from 'antd';
  export default function PublicAuthFixture({ ctx }) {
    const [mode, setMode] = useState('sign_in');
    const [error, setError] = useState(null);
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
    return <Space direction="vertical" size="middle" style={{ width: '100%' }}>
      <h2>
        {mode === 'sign_in' ? 'Sign in' : 'Create an account'}
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
    </Space>;
  }
`;

function Fixture() {
  const [completion, setCompletion] = useState('idle');
  return (
    <main
      data-testid="public-auth-native-fixture"
      data-viewport={window.innerWidth <= 390 ? 'mobile-390' : 'desktop'}
      style={{ width: 'min(100% - 32px, 440px)', margin: '40px auto' }}
    >
      <PublicAuthBlock
        instance={{
          id: 'auth-password-local',
          auth_type: 'password-local',
          title: 'Password',
          description: null,
          sort_order: 0,
          public_ui_block: source,
          public_variables: { self_registration_enabled: true }
        }}
        onAuthenticated={() => setCompletion('authenticated')}
      />
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
