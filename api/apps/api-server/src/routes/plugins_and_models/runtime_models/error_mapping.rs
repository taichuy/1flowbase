use super::*;

pub(super) fn map_runtime_error(error: anyhow::Error) -> ApiError {
    if let Some(runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(reason)) =
        error.downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
    {
        return control_plane::errors::ControlPlaneError::PermissionDenied(reason).into();
    }

    if error.to_string().contains("runtime record not found") {
        return control_plane::errors::ControlPlaneError::NotFound("runtime_record").into();
    }

    if let Some(model_error) =
        error.downcast_ref::<runtime_core::runtime_engine::RuntimeModelError>()
    {
        let code = match model_error {
            runtime_core::runtime_engine::RuntimeModelError::Unavailable(_) => {
                "runtime_model_unavailable"
            }
            runtime_core::runtime_engine::RuntimeModelError::NotPublished(_) => {
                "model_not_published"
            }
            runtime_core::runtime_engine::RuntimeModelError::Disabled(_) => "model_disabled",
            runtime_core::runtime_engine::RuntimeModelError::Broken(_) => "model_broken",
            runtime_core::runtime_engine::RuntimeModelError::RecordActionNotAllowed { .. } => {
                "runtime_model_record_action_not_allowed"
            }
            runtime_core::runtime_engine::RuntimeModelError::MissingCreateRequiredFields {
                ..
            } => {
                return control_plane::errors::ControlPlaneError::InvalidInput("api_required")
                    .into();
            }
            runtime_core::runtime_engine::RuntimeModelError::InvalidOperationInput(_) => {
                return control_plane::errors::ControlPlaneError::InvalidInput(
                    "runtime_operation_input",
                )
                .into();
            }
            runtime_core::runtime_engine::RuntimeModelError::InvalidOperationField(_) => {
                return control_plane::errors::ControlPlaneError::InvalidInput(
                    "runtime_operation_field",
                )
                .into();
            }
            runtime_core::runtime_engine::RuntimeModelError::OrderedTreeUnavailable => {
                return ApiServiceUnavailable("ordered_tree_unavailable").into();
            }
        };
        return control_plane::errors::ControlPlaneError::Conflict(code).into();
    }

    if let Some(tree_error) =
        error.downcast_ref::<runtime_core::runtime_record_repository::OrderedTreeCommandError>()
    {
        use runtime_core::runtime_record_repository::OrderedTreeCommandError as Error;
        return match tree_error {
            Error::ConflictingAnchors | Error::FieldNotWritable(_) => {
                control_plane::errors::ControlPlaneError::InvalidInput(tree_error.code()).into()
            }
            Error::NodeNotFound | Error::ParentNotFound | Error::AnchorNotFound => {
                control_plane::errors::ControlPlaneError::NotFound(tree_error.code()).into()
            }
            Error::TreeNodeHasChildren
            | Error::ExpectedAffectedCountMismatch { .. }
            | Error::PositionConflict
            | Error::Cycle
            | Error::AnchorSiblingGroupConflict => {
                control_plane::errors::ControlPlaneError::Conflict(tree_error.code()).into()
            }
            Error::WrongTemplate => ApiServiceUnavailable(tree_error.code()).into(),
        };
    }

    if let Some(tree_error) =
        error.downcast_ref::<runtime_core::runtime_record_repository::OrderedTreeQueryError>()
    {
        use runtime_core::runtime_record_repository::OrderedTreeQueryError as Error;
        return match tree_error {
            Error::NodeNotFound => {
                control_plane::errors::ControlPlaneError::NotFound("tree_node_not_found").into()
            }
            Error::ParentNotFound => {
                control_plane::errors::ControlPlaneError::NotFound("tree_parent_not_found").into()
            }
            Error::InvalidResultLimit | Error::InvalidMaxDepth | Error::EmptySearchPrefix => {
                control_plane::errors::ControlPlaneError::InvalidInput("ordered_tree_query_input")
                    .into()
            }
            Error::InvalidCursor => control_plane::errors::ControlPlaneError::InvalidInput(
                "ordered_tree_invalid_cursor",
            )
            .into(),
            Error::StaleCursor => {
                control_plane::errors::ControlPlaneError::Conflict("ordered_tree_stale_cursor")
                    .into()
            }
            Error::WrongTemplate | Error::NoSearchableFields => {
                ApiServiceUnavailable("ordered_tree_unavailable").into()
            }
        };
    }

    error.into()
}

pub(super) fn runtime_acl_denial_reason(error: &anyhow::Error) -> Option<&'static str> {
    if let Some(runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(reason)) =
        error.downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
    {
        return Some(reason);
    }

    None
}
