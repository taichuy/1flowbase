use super::*;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl AvailableMethods {
    pub fn test_providers(&self) -> &Vec<crate::authentication_runtime::AuthenticationProvider> { &self.providers }
    pub fn test_authorities(&self) -> &Vec<String> { &self.authorities }
    pub fn test_leases(&self) -> &Vec<Box<dyn control_plane::ports::ContributionAuthorityLease>> { &self.leases }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl AvailableMethods { pub fn test_take_leases(&mut self) -> Vec<Box<dyn control_plane::ports::ContributionAuthorityLease>> { std::mem::take(&mut self.leases) } }
