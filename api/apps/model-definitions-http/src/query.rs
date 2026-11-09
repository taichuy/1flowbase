use super::*;
use axum::extract::{rejection::QueryRejection, FromRequestParts, Query};
use axum::http::request::Parts;
use std::future::Future;

// Only these two frozen wire queries use this private macro.
macro_rules! query_extractor {
    ($query:ident, $decode:ident) => {
        pub(crate) async fn $decode(parts: &mut Parts) -> Result<$query, QueryRejection> {
            let Query(query) =
                <Query<$query> as FromRequestParts<()>>::from_request_parts(parts, &()).await?;
            Ok(query)
        }

        impl<S: Send + Sync> FromRequestParts<S> for $query {
            type Rejection = QueryRejection;

            fn from_request_parts(
                parts: &mut Parts,
                _state: &S,
            ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
                $decode(parts)
            }
        }
    };
}

query_extractor!(ListModelsQuery, decode_list_models_query);
query_extractor!(ConfirmationQuery, decode_confirmation_query);
