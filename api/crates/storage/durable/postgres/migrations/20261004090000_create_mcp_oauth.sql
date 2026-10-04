create table mcp_oauth_state (
  kind text not null, token_hash text not null, payload jsonb not null,
  expires_at timestamptz, primary key(kind, token_hash),
  check ((kind = 'client') = (expires_at is null))
);
create index mcp_oauth_state_expiry on mcp_oauth_state(expires_at);
create table mcp_oauth_grants (
  id uuid primary key, user_id uuid not null, api_key_id uuid not null,
  payload jsonb not null, expires_at timestamptz not null, revoked boolean not null default false
);
create index mcp_oauth_grants_user on mcp_oauth_grants(user_id) where not revoked;
create table mcp_oauth_refresh_tokens (
  token_hash text primary key, grant_id uuid not null references mcp_oauth_grants(id) on delete cascade,
  expires_at timestamptz not null, consumed boolean not null default false
);
create index mcp_oauth_refresh_grant on mcp_oauth_refresh_tokens(grant_id);
