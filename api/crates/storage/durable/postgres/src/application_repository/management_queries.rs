use super::*;

#[async_trait]
impl ApplicationManagementRepository for PgControlPlaneStore {
    async fn list_application_management(
        &self,
        workspace_id: Uuid,
        query: &ApplicationManagementQuery,
    ) -> Result<ApplicationManagementPage> {
        let mut count_builder = QueryBuilder::<Postgres>::new(
            "select count(*) from applications a where a.workspace_id = ",
        );
        count_builder.push_bind(workspace_id);
        append_application_management_filter(&mut count_builder, &query.filter)?;
        let total = count_builder
            .build_query_scalar::<i64>()
            .fetch_one(self.pool())
            .await?;

        let mut list_builder = QueryBuilder::<Postgres>::new(
            r#"
            select
                a.id,
                a.application_type,
                a.workflow_trigger_type,
                a.name,
                a.description,
                a.icon,
                a.icon_type,
                a.icon_background,
                a.created_by,
                coalesce(nullif(creator.nickname, ''), nullif(creator.name, ''), creator.account, '')
                    as created_by_display_name,
                a.created_at,
                a.updated_at,
                exists(
                    select 1
                    from application_publication_versions publication
                    where publication.application_id = a.id
                      and publication.active = true
                ) as published,
                a.api_enabled,
                coalesce(tags.tags, '[]'::jsonb) as tags
            from applications a
            left join users creator on creator.id = a.created_by
            left join lateral (
                select jsonb_agg(
                    jsonb_build_object('id', tag.id, 'name', tag.name)
                    order by tag.name asc, tag.id asc
                ) as tags
                from application_tag_bindings binding
                join application_tags tag on tag.id = binding.tag_id
                where binding.application_id = a.id
            ) tags on true
            where a.workspace_id =
            "#,
        );
        list_builder.push_bind(workspace_id);
        append_application_management_filter(&mut list_builder, &query.filter)?;
        append_application_management_sort(
            &mut list_builder,
            query.sort_field,
            query.sort_direction,
        );
        list_builder.push(" limit ");
        list_builder.push_bind(query.page_size);
        list_builder.push(" offset ");
        list_builder.push_bind(query.page.saturating_sub(1).saturating_mul(query.page_size));

        let rows = list_builder.build().fetch_all(self.pool()).await?;
        let items = rows
            .into_iter()
            .map(|row| {
                let application_type =
                    parse_application_type(row.get::<String, _>("application_type").as_str())?;
                let workflow_trigger_type = row
                    .get::<Option<String>, _>("workflow_trigger_type")
                    .as_deref()
                    .map(parse_workflow_trigger_type)
                    .transpose()?;
                let published: bool = row.get("published");

                Ok(ApplicationManagementRecord {
                    id: row.get("id"),
                    application_type,
                    workflow_trigger_type,
                    name: row.get("name"),
                    description: row.get("description"),
                    icon: row.get("icon"),
                    icon_type: row.get("icon_type"),
                    icon_background: row.get("icon_background"),
                    created_by: row.get("created_by"),
                    created_by_display_name: row.get("created_by_display_name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    tags: serde_json::from_value(row.get::<Value, _>("tags"))?,
                    api_enabled: row.get("api_enabled"),
                    publication_status: if application_type == domain::ApplicationType::AgentLogs {
                        domain::ApplicationPublicationStatus::NotApplicable
                    } else if published {
                        domain::ApplicationPublicationStatus::Published
                    } else {
                        domain::ApplicationPublicationStatus::Unpublished
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(ApplicationManagementPage {
            items,
            total,
            page: query.page,
            page_size: query.page_size,
        })
    }
}

pub(super) fn append_application_management_filter(
    builder: &mut QueryBuilder<Postgres>,
    filter: &domain::ResourceFilterExpr,
) -> Result<()> {
    if matches!(filter, domain::ResourceFilterExpr::All(items) if items.is_empty()) {
        return Ok(());
    }

    builder.push(" and ");
    append_application_management_filter_expr(builder, filter)
}

pub(super) fn append_application_management_filter_expr(
    builder: &mut QueryBuilder<Postgres>,
    filter: &domain::ResourceFilterExpr,
) -> Result<()> {
    match filter {
        domain::ResourceFilterExpr::All(items) => {
            if items.is_empty() {
                builder.push("true");
                return Ok(());
            }
            builder.push("(");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    builder.push(" and ");
                }
                append_application_management_filter_expr(builder, item)?;
            }
            builder.push(")");
        }
        domain::ResourceFilterExpr::Any(items) => {
            if items.is_empty() {
                builder.push("false");
                return Ok(());
            }
            builder.push("(");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    builder.push(" or ");
                }
                append_application_management_filter_expr(builder, item)?;
            }
            builder.push(")");
        }
        domain::ResourceFilterExpr::Field {
            field,
            operator,
            value,
        } => {
            if field == "tags.id" {
                append_application_tag_filter(builder, *operator, value)?;
            } else {
                let expression = match field.as_str() {
                    "id" => "a.id::text",
                    "name" => "a.name",
                    "application_type" => "a.application_type",
                    "workflow_trigger_type" => "coalesce(a.workflow_trigger_type, '')",
                    "publication_status" => {
                        "case when a.application_type = 'agent_logs' then 'not_applicable' when exists (select 1 from application_publication_versions publication where publication.application_id = a.id and publication.active = true) then 'published' else 'unpublished' end"
                    }
                    "created_by" => "a.created_by::text",
                    _ => anyhow::bail!(ControlPlaneError::InvalidInput("filter")),
                };
                append_application_text_filter(builder, expression, *operator, value)?;
            }
        }
    }

    Ok(())
}

pub(super) fn append_application_tag_filter(
    builder: &mut QueryBuilder<Postgres>,
    operator: domain::ResourceFilterOperator,
    value: &Value,
) -> Result<()> {
    builder.push(
        "exists (select 1 from application_tag_bindings filter_binding where filter_binding.application_id = a.id and ",
    );
    append_application_text_filter(builder, "filter_binding.tag_id::text", operator, value)?;
    builder.push(")");
    Ok(())
}

pub(super) fn append_application_text_filter(
    builder: &mut QueryBuilder<Postgres>,
    expression: &'static str,
    operator: domain::ResourceFilterOperator,
    value: &Value,
) -> Result<()> {
    if operator == domain::ResourceFilterOperator::In {
        let values = value
            .as_array()
            .ok_or(ControlPlaneError::InvalidInput("filter"))?;
        if values.is_empty() {
            builder.push("false");
            return Ok(());
        }
        builder.push(expression);
        builder.push(" in (");
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                builder.push(", ");
            }
            builder.push_bind(application_management_filter_text(value)?);
        }
        builder.push(")");
        return Ok(());
    }

    builder.push(expression);
    builder.push(match operator {
        domain::ResourceFilterOperator::Eq => " = ",
        domain::ResourceFilterOperator::Ne => " <> ",
        domain::ResourceFilterOperator::Includes => " ilike ",
        domain::ResourceFilterOperator::NotIncludes => " not ilike ",
        _ => anyhow::bail!(ControlPlaneError::InvalidInput("filter")),
    });
    let value = application_management_filter_text(value)?;
    if matches!(
        operator,
        domain::ResourceFilterOperator::Includes | domain::ResourceFilterOperator::NotIncludes
    ) {
        builder.push_bind(format!("%{value}%"));
    } else {
        builder.push_bind(value);
    }
    Ok(())
}

pub(super) fn application_management_filter_text(value: &Value) -> Result<String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ControlPlaneError::InvalidInput("filter").into())
}

pub(super) fn append_application_management_sort(
    builder: &mut QueryBuilder<Postgres>,
    field: ApplicationManagementSortField,
    direction: ApplicationManagementSortDirection,
) {
    builder.push(" order by ");
    builder.push(match field {
        ApplicationManagementSortField::UpdatedAt => "a.updated_at",
        ApplicationManagementSortField::CreatedAt => "a.created_at",
        ApplicationManagementSortField::Name => "a.name",
        ApplicationManagementSortField::ApplicationType => "a.application_type",
    });
    builder.push(match direction {
        ApplicationManagementSortDirection::Asc => " asc",
        ApplicationManagementSortDirection::Desc => " desc",
    });
    builder.push(", a.id");
    builder.push(match direction {
        ApplicationManagementSortDirection::Asc => " asc",
        ApplicationManagementSortDirection::Desc => " desc",
    });
}
