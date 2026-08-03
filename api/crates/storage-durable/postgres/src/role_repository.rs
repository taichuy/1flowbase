use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateWorkspaceRoleInput, ReplaceRoleConsolePolicyInput,
        ReplaceRoleDataPolicyInput, RoleConsolePolicyMigrationCutoverMarker,
        RoleConsolePolicyMigrationCutoverState, RoleConsolePolicyMigrationGrantInventory,
        RoleConsolePolicyMigrationRehearsalInput, RoleConsolePolicyMigrationRepository,
        RoleConsolePolicyMigrationSource, RoleConsolePolicyReader, RoleDataPolicyDefaultsInput,
        RoleRepository, UpdateWorkspaceRoleInput,
    },
    role::console_policy_migration::{
        validate_console_policy_migration_actor_previews, ConsolePolicyMigrationActorPreview,
        ConsolePolicyMigrationActorRoleBinding,
    },
};
use domain::{ActorContext, AuditLogRecord, RoleScopeKind};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    mappers::role_mapper::PgRoleMapper,
    repositories::{
        find_role_by_code, permission_codes_for_role, stored_role_from_row,
        tenant_id_for_workspace, workspace_id_for_user, PgControlPlaneStore,
    },
};

mod console_policy;
mod data_policy;
mod policy_migration;
mod role_store;

use console_policy::replace_role_console_policy_rows;
use data_policy::{
    data_policy_scope_from_db, default_role_data_policy, insert_default_role_data_policy,
    optional_data_policy_scope_from_db,
};

pub(crate) use console_policy::role_console_policy_by_id;
