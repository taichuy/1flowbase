import { ArrowLeftOutlined } from '@ant-design/icons';
import { useState } from 'react';
import { Alert, Button, Input, Space } from 'antd';

type AuthInputs = {
  authenticator_id?: string;
  authenticator_selection_available?: boolean;
  public_variables?: {
    self_registration_enabled?: boolean;
  };
};

type AuthContext = {
  inputs: AuthInputs;
  api: {
    post<TResponse = unknown>(
      path: string,
      request?: { body?: unknown }
    ): Promise<TResponse>;
  };
  events: {
    emit(event: string): void;
  };
};

export default function PasswordLocalAuth({ ctx }: { ctx: AuthContext }) {
  const [mode, setMode] = useState<'sign_in' | 'sign_up'>('sign_in');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [identifier, setIdentifier] = useState('');
  const [password, setPassword] = useState('');
  const [account, setAccount] = useState('');
  const [email, setEmail] = useState('');
  const registrationEnabled =
    ctx.inputs.public_variables?.self_registration_enabled === true;
  const authenticatorSelectionAvailable =
    ctx.inputs.authenticator_selection_available === true;

  const submitSignIn = async (event) => {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      await ctx.api.post('/api/public/auth/sign-in', {
        body: {
          authenticator_id: ctx.inputs.authenticator_id,
          identifier,
          password
        }
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Authentication failed');
    } finally {
      setPending(false);
    }
  };

  const submitSignUp = async (event) => {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      await ctx.api.post('/api/public/auth/sign-up', {
        body: {
          authenticator_id: ctx.inputs.authenticator_id,
          account,
          email,
          password
        }
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Registration failed');
    } finally {
      setPending(false);
    }
  };

  return (
    <Space direction="vertical" size="middle" style={{ width: '100%' }}>
      {authenticatorSelectionAvailable ? (
        <Button
          aria-label="Back to other sign-in options"
          disabled={pending}
          icon={<ArrowLeftOutlined aria-hidden="true" />}
          onClick={() => ctx.events.emit('authenticator_selector_requested')}
          style={{ alignSelf: 'flex-start' }}
          type="text"
        />
      ) : null}
      <h2>
        {mode === 'sign_in' ? 'Sign in' : 'Create an account'}
      </h2>
      {error ? <Alert type="error" showIcon message={error} /> : null}
      {mode === 'sign_in' ? (
        <form onSubmit={submitSignIn} style={{ display: 'grid', gap: 12 }}>
          <label style={{ display: 'grid', gap: 6 }}>
            Account or email
            <Input
              required
              autoComplete="username"
              value={identifier}
              onChange={(event) => setIdentifier(event.target.value)}
            />
          </label>
          <label style={{ display: 'grid', gap: 6 }}>
            Password
            <Input
              required
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
          </label>
          <Button htmlType="submit" type="primary" block loading={pending}>
            Sign in
          </Button>
        </form>
      ) : (
        <form onSubmit={submitSignUp} style={{ display: 'grid', gap: 12 }}>
          <label style={{ display: 'grid', gap: 6 }}>
            Account
            <Input
              required
              autoComplete="username"
              value={account}
              onChange={(event) => setAccount(event.target.value)}
            />
          </label>
          <label style={{ display: 'grid', gap: 6 }}>
            Email
            <Input
              type="email"
              autoComplete="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
          </label>
          <label style={{ display: 'grid', gap: 6 }}>
            Password
            <Input
              required
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
          </label>
          <Button htmlType="submit" type="primary" block loading={pending}>
            Register
          </Button>
        </form>
      )}
      {registrationEnabled ? (
        <Button
          type="link"
          disabled={pending}
          onClick={() => setMode((current) =>
            current === 'sign_in' ? 'sign_up' : 'sign_in'
          )}
        >
          {mode === 'sign_in' ? 'Create an account' : 'Back to sign in'}
        </Button>
      ) : null}
    </Space>
  );
}
