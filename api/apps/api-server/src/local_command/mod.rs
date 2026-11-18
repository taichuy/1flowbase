pub(crate) use api_console_interaction::local_command::*;

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
pub(crate) mod process;
