use sqlx::PgPool;
use storage_postgres::run_migrations;
use uuid::Uuid;

const LEGACY_AUTH_TEAM_ACL_CHECKSUM_HEX: &str =
    "c588c5dafce2a9f065474ad847cc264e80ba8ce16e62479071c98bc6923e2019e2bc310c4c9f17b541fbdd0ec0a499f1";

const LEGACY_AUTH_TEAM_ACL_SQL: &str = r#"
create table if not exists tenants (
  id uuid primary key,
  code text not null unique,
  name text not null,
  is_root boolean not null default false,
  is_hidden boolean not null default false,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create table if not exists users (
  id uuid primary key,
  account text not null unique,
  email text not null unique,
  phone text unique,
  password_hash text not null,
  name text not null,
  nickname text not null,
  avatar_url text,
  introduction text not null default '',
  default_display_role text,
  email_login_enabled boolean not null default true,
  phone_login_enabled boolean not null default false,
  status text not null,
  session_version bigint not null default 1,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  check (status in ('active', 'disabled'))
);

create table if not exists workspaces (
  id uuid primary key,
  tenant_id uuid not null references tenants(id) on delete cascade,
  name text not null,
  logo_url text,
  introduction text not null default '',
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create unique index if not exists workspaces_tenant_name_uidx
  on workspaces (tenant_id, lower(name));

create table if not exists workspace_memberships (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  user_id uuid not null references users(id) on delete cascade,
  introduction text not null default '',
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  unique (workspace_id, user_id)
);

create table if not exists roles (
  id uuid primary key,
  scope_kind text not null,
  workspace_id uuid references workspaces(id) on delete cascade,
  code text not null,
  name text not null,
  introduction text not null default '',
  is_builtin boolean not null default false,
  is_editable boolean not null default true,
  system_kind text,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  check (scope_kind in ('system', 'workspace'))
);

create unique index if not exists roles_system_code_uidx
  on roles (code)
  where scope_kind = 'system';

create unique index if not exists roles_workspace_code_uidx
  on roles (workspace_id, code)
  where scope_kind = 'workspace';

create table if not exists permission_definitions (
  id uuid primary key,
  resource text not null,
  action text not null,
  scope text not null,
  code text not null unique,
  name text not null,
  introduction text not null default '',
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create table if not exists role_permissions (
  id uuid primary key,
  role_id uuid not null references roles(id) on delete cascade,
  permission_id uuid not null references permission_definitions(id) on delete cascade,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  unique (role_id, permission_id)
);

create table if not exists user_role_bindings (
  id uuid primary key,
  user_id uuid not null references users(id) on delete cascade,
  role_id uuid not null references roles(id) on delete cascade,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now(),
  unique (user_id, role_id)
);

create table if not exists authenticators (
  id uuid primary key,
  name text not null unique,
  auth_type text not null,
  title text not null,
  enabled boolean not null default true,
  is_builtin boolean not null default false,
  sort_order integer not null default 0,
  options jsonb not null default '{}'::jsonb,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create table if not exists user_auth_identities (
  id uuid primary key,
  user_id uuid not null references users(id) on delete cascade,
  authenticator_name text not null references authenticators(name) on delete cascade,
  subject_type text not null,
  subject_value text not null,
  metadata jsonb not null default '{}'::jsonb,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

create unique index if not exists user_auth_identities_subject_uidx
  on user_auth_identities (authenticator_name, subject_type, lower(subject_value));

create table if not exists audit_logs (
  id uuid primary key,
  workspace_id uuid references workspaces(id) on delete set null,
  actor_user_id uuid references users(id) on delete set null,
  target_type text not null,
  target_id uuid,
  event_code text not null,
  payload jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);
"#;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn seed_legacy_auth_team_acl_migration(pool: &PgPool) {
    sqlx::raw_sql(LEGACY_AUTH_TEAM_ACL_SQL)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        create table _sqlx_migrations (
            version bigint primary key,
            description text not null,
            installed_on timestamptz not null default now(),
            success boolean not null,
            checksum bytea not null,
            execution_time bigint not null
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into _sqlx_migrations (
            version, description, success, checksum, execution_time
        ) values ($1, $2, true, decode($3, 'hex'), 0)
        "#,
    )
    .bind(20260412183000_i64)
    .bind("create auth team acl tables")
    .bind(LEGACY_AUTH_TEAM_ACL_CHECKSUM_HEX)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_legacy_password_identity(
    pool: &PgPool,
    user_id: Uuid,
    identity_id: Uuid,
    legacy_authenticator_id: Uuid,
) {
    sqlx::query(
        r#"
        insert into users (
            id, account, email, password_hash, name, nickname, status
        ) values (
            $1, 'root', 'root@example.com', 'hash', 'Root', 'Root', 'active'
        )
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into authenticators (
            id, name, auth_type, title, enabled, is_builtin, sort_order, options
        ) values (
            $1, 'password-local', 'password-local', 'Password', true, true, 0, '{}'
        )
        "#,
    )
    .bind(legacy_authenticator_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into user_auth_identities (
            id, user_id, authenticator_name, subject_type, subject_value, metadata
        ) values (
            $1, $2, 'password-local', 'account', 'root', '{}'
        )
        "#,
    )
    .bind(identity_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn migrations_upgrade_legacy_authenticator_identity_schema() {
    let pool = isolated_database().await.connect().await.unwrap();
    let user_id = Uuid::now_v7();
    let identity_id = Uuid::now_v7();
    let legacy_authenticator_id = Uuid::now_v7();
    seed_legacy_auth_team_acl_migration(&pool).await;
    seed_legacy_password_identity(&pool, user_id, identity_id, legacy_authenticator_id).await;

    run_migrations(&pool).await.unwrap();

    let authenticator_name_column_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'user_auth_identities'
          and column_name = 'authenticator_name'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let authenticator_id: Uuid =
        sqlx::query_scalar("select authenticator_id from user_auth_identities where id = $1")
            .bind(identity_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let authenticator_name_key_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'authenticators'
          and column_name = 'name'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let legacy_authenticator_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from authenticators
        where id = $1
        "#,
    )
    .bind(legacy_authenticator_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let subject_index: String = sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = current_schema()
          and tablename = 'user_auth_identities'
          and indexname = 'user_auth_identities_subject_uidx'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let has_legacy_name_config: bool = sqlx::query_scalar(
        r#"
        select exists (
            select 1
            from authenticators
            cross join jsonb_array_elements(
                coalesce(options -> 'config_form_schema', '[]'::jsonb)
            ) item
            where id = $1
              and item ->> 'key' = 'name'
        )
        "#,
    )
    .bind(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(authenticator_name_column_count, 0);
    assert_eq!(authenticator_id, domain::PASSWORD_LOCAL_AUTHENTICATOR_ID);
    assert_eq!(authenticator_name_key_count, 0);
    assert_eq!(legacy_authenticator_count, 0);
    assert!(subject_index.contains("authenticator_id"));
    assert!(!has_legacy_name_config);
}
