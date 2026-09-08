//! Host-resolved graph inputs. These are projections of durable authority, never worker input.
use super::{ContributionId, ManagedContributionSubject, ModuleId, PermissionCode};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize)]
pub struct ManagedContributionAuthority {
    pub subject: ManagedContributionSubject,
    pub revision: i64,
    pub permissions: BTreeSet<PermissionCode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedGraphAuthority {
    pub host_policy_identity: String,
    pub managed_modules: BTreeSet<ModuleId>,
    pub contributions: BTreeMap<ContributionId, ManagedContributionAuthority>,
}
impl ManagedGraphAuthority {
    pub fn new(host_policy_identity: String) -> Self {
        Self {
            host_policy_identity,
            managed_modules: BTreeSet::new(),
            contributions: BTreeMap::new(),
        }
    }
}
