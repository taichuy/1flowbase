use serde_json::{json, Map, Value};

pub const PASSWORD_LOCAL_PUBLIC_UI_BLOCK: &str =
    include_str!("public_ui_blocks/password_local.tsx");

// Kept as the exact comparison source for the startup upgrade. User-edited
// Blocks never match this value and therefore remain untouched.
pub const PREVIOUS_PASSWORD_LOCAL_PUBLIC_UI_BLOCK: &str = r#"import { useState } from 'react';
import { Alert, Button, Input, Space } from 'antd';

type AuthInputs = {
  authenticator_id?: string;
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
"#;

pub fn auth_common_config_form_schema() -> Value {
    json!([
        {
            "key": "title",
            "label": "Authenticator title",
            "type": "string",
            "required": true
        },
        {
            "key": "description",
            "label": "Description",
            "type": "string",
            "control": "textarea",
            "read_only": false,
            "required": false
        },
        {
            "key": "enabled",
            "label": "Enabled",
            "type": "boolean",
            "control": "switch"
        }
    ])
}

pub fn password_local_config_form_schema() -> Value {
    let mut fields = auth_common_config_form_schema()
        .as_array()
        .cloned()
        .expect("common auth config schema must be an array");
    fields.insert(
        3,
        json!({
            "key": "self_registration_enabled",
            "label": "Allow self registration",
            "type": "boolean",
            "control": "switch"
        }),
    );
    Value::Array(fields)
}

pub fn password_local_options(description: Option<String>) -> Value {
    let mut options = Map::new();
    if let Some(description) = description {
        options.insert("description".to_string(), Value::String(description));
    }
    options.insert(
        "config_form_schema".to_string(),
        password_local_config_form_schema(),
    );
    options.insert(
        "extension_config".to_string(),
        json!({ "self_registration_enabled": false }),
    );
    Value::Object(options)
}

pub fn password_local_public_variables(options: &Value) -> Map<String, Value> {
    let self_registration_enabled = password_local_self_registration_enabled(options);
    Map::from_iter([(
        "self_registration_enabled".to_string(),
        Value::Bool(self_registration_enabled),
    )])
}

pub fn authenticator_host_public_variables(
    authenticator: &domain::AuthenticatorRecord,
) -> Map<String, Value> {
    let mut variables = Map::from_iter([
        (
            "title".to_string(),
            Value::String(authenticator.title.clone()),
        ),
        ("enabled".to_string(), Value::Bool(authenticator.enabled)),
    ]);
    if let Some(description) = authenticator
        .options
        .get("description")
        .and_then(Value::as_str)
    {
        variables.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
    variables
}

pub fn password_local_self_registration_enabled(options: &Value) -> bool {
    options
        .get("extension_config")
        .and_then(|config| config.get("self_registration_enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
