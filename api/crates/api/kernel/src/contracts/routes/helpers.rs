use axum::Json;
use serde::{de::Deserializer, Deserialize};
use uuid::Uuid;

use crate::{error_response::ApiError, response::ApiSuccess};

pub(crate) type ApiJson<T> = Json<ApiSuccess<T>>;

pub(crate) fn ok<T>(data: T) -> ApiJson<T> {
    Json(ApiSuccess::new(data))
}

pub(crate) fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

pub(crate) fn deserialize_present_optional<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}
