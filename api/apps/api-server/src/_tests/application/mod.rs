pub(crate) use crate::test_support::application::*;
pub(crate) mod application_runtime_routes {
    pub(crate) use crate::test_support::application::*;
    mod logs_routes {
        mod unified_log_queries {
            include!("../targets/retained/application_query.rs");
        }
    }
}
