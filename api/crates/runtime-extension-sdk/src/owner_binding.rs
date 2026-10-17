use std::collections::{hash_map::Entry, HashMap};
use std::hash::Hash;

/// Process-local associations from trusted resource identities to already selected owners.
///
/// The caller supplies identity, owner selection and retention policy. This table does not
/// allocate identities, select routes, close resources, or authorize a missing association.
/// Rebinding an identity cannot redirect later control messages to a different owner.
pub struct ResourceOwnerBindings<I, O> {
    owners: HashMap<I, O>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("resource identity is already bound to another owner")]
pub struct ResourceOwnerBindingConflict;

impl<I, O> Default for ResourceOwnerBindings<I, O> {
    fn default() -> Self {
        Self {
            owners: HashMap::new(),
        }
    }
}

impl<I: Eq + Hash, O: PartialEq> ResourceOwnerBindings<I, O> {
    /// Establish an immutable binding. Repeating the same association is idempotent.
    /// A conflicting association leaves the existing owner intact.
    pub fn bind(&mut self, identity: I, owner: O) -> Result<(), ResourceOwnerBindingConflict> {
        match self.owners.entry(identity) {
            Entry::Vacant(entry) => {
                entry.insert(owner);
                Ok(())
            }
            Entry::Occupied(entry) if entry.get() == &owner => Ok(()),
            Entry::Occupied(_) => Err(ResourceOwnerBindingConflict),
        }
    }

    pub fn get(&self, identity: &I) -> Option<&O> {
        self.owners.get(identity)
    }

    /// Forget only associations whose owner/evidence lifetime has ended according to the caller.
    pub fn retain(&mut self, mut keep: impl FnMut(&I, &O) -> bool) {
        self.owners.retain(|identity, owner| keep(identity, owner));
    }
}

#[cfg(test)]
#[path = "_tests/owner_binding.rs"]
mod tests;
