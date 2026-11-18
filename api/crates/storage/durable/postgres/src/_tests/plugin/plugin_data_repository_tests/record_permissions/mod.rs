use super::*;
use control_plane_contracts::ports::RecordPermissionRepository;
use domain::{
    ActorContext, RecordPermissionGrant, RecordPermissionKey, RecordPermissionMode,
    RecordPermissionPolicy, RecordPermissionResourceKey, ReplaceRecordPermissions,
};
use extension_contracts::{PluginDataRecordPermissionContext, PluginDataRecordResource};

mod support;
use support::*;
mod create_and_receipts;
mod policies;
mod queries;
