#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use super::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use crate::routes::external_auth::transaction;
#[cfg_attr(test, api_test_selection::group(routes))]
pub(crate) use api_console_interaction::routes::identity_group::me_authentication_methods::proof::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/proof.rs"]
mod tests;
