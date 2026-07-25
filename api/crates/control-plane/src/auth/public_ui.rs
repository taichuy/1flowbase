use serde_json::{json, Map, Value};

pub const PASSWORD_LOCAL_PUBLIC_UI_BLOCK: &str = r#"import type {
  BlockContext,
  BlockModule,
  BlockResult
} from '@1flowbase/block-sdk';

import {
  Alert,
  Button,
  Form,
  FormItem,
  Input,
  Stack,
  Text,
  Title
} from '@1flowbase/block-renderer/antd-facade';

type AuthEvent = {
  action_id?: string;
  values?: Record<string, unknown>;
};

type AuthInputs = {
  authenticator_id?: string;
  public_variables?: {
    self_registration_enabled?: boolean;
  };
  auth_event?: AuthEvent;
};

async function main(ctx: BlockContext<AuthInputs>): Promise<BlockResult> {
  const event = ctx.inputs.auth_event;
  const values = event?.values ?? {};
  let feedback = null;

  try {
    if (event?.action_id === 'sign_in') {
      await ctx.api.post('/api/public/auth/sign-in', {
        body: {
          authenticator_id: ctx.inputs.authenticator_id,
          identifier: String(values.identifier ?? ''),
          password: String(values.password ?? '')
        }
      });
      feedback = <Alert type="success" message="Signed in" />;
    }
    if (event?.action_id === 'sign_up') {
      await ctx.api.post('/api/public/auth/sign-up', {
        body: {
          authenticator_id: ctx.inputs.authenticator_id,
          account: String(values.account ?? ''),
          email: String(values.email ?? ''),
          password: String(values.registration_password ?? '')
        }
      });
      feedback = <Alert type="success" message="Account created" />;
    }
  } catch {
    feedback = <Alert type="error" message="Authentication failed" />;
  }

  const registrationEnabled =
    ctx.inputs.public_variables?.self_registration_enabled === true;

  return {
    view: (
      <Stack>
        <Title>Sign in</Title>
        {feedback}
        <Form>
          <FormItem name="identifier" label="Account or email">
            <Input />
          </FormItem>
          <FormItem name="password" label="Password">
            <Input type="password" />
          </FormItem>
          <Button actionId="sign_in" type="primary">Sign in</Button>
        </Form>
        {registrationEnabled ? (
          <Stack>
            <Text>Create an account</Text>
            <Form>
              <FormItem name="account" label="Account">
                <Input />
              </FormItem>
              <FormItem name="email" label="Email">
                <Input />
              </FormItem>
              <FormItem name="registration_password" label="Password">
                <Input type="password" />
              </FormItem>
              <Button actionId="sign_up">Register</Button>
            </Form>
          </Stack>
        ) : null}
      </Stack>
    ),
    outputs: {}
  };
}

export default { main } satisfies BlockModule;
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
