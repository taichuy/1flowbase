use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use futures_util::TryStreamExt;
use plugin_framework::{
    NativeSqlColumn, NativeSqlExecutionItem, NativeSqlExecutionOutput, NativeSqlLogicalType,
    NativeSqlValueEncoding, ProviderRuntimeError, ProviderRuntimeErrorKind,
};
use serde_json::{json, Value};
use sqlx::{
    postgres::{PgDatabaseError, PgRow},
    Column, Either, PgPool, Row, TypeInfo, ValueRef,
};

pub async fn execute_native_sql(
    pool: &PgPool,
    sql: &str,
) -> Result<NativeSqlExecutionOutput, ProviderRuntimeError> {
    let mut stream = sqlx::raw_sql(sql).fetch_many(pool);
    let mut results = Vec::new();
    let mut pending_columns: Option<Vec<NativeSqlColumn>> = None;
    let mut pending_rows = Vec::new();

    while let Some(event) = stream.try_next().await.map_err(map_sqlx_error)? {
        match event {
            Either::Right(row) => {
                let columns = pending_columns
                    .get_or_insert_with(|| row.columns().iter().map(column_contract).collect());
                pending_rows.push(encode_row(&row, columns)?);
            }
            Either::Left(completion) => {
                if let Some(columns) = pending_columns.take() {
                    results.push(NativeSqlExecutionItem::RowBatch {
                        columns,
                        rows: std::mem::take(&mut pending_rows),
                    });
                }
                results.push(NativeSqlExecutionItem::Completion {
                    affected_rows: completion.rows_affected(),
                    native_status: None,
                });
            }
        }
    }

    if let Some(columns) = pending_columns {
        results.push(NativeSqlExecutionItem::RowBatch {
            columns,
            rows: pending_rows,
        });
    }

    Ok(NativeSqlExecutionOutput { results })
}

fn column_contract(column: &sqlx::postgres::PgColumn) -> NativeSqlColumn {
    let native_type = column.type_info().name().to_string();
    let (logical_type, encoding) = classify_postgres_type(&native_type);
    NativeSqlColumn {
        name: column.name().to_string(),
        native_type,
        logical_type,
        encoding,
    }
}

fn classify_postgres_type(native_type: &str) -> (NativeSqlLogicalType, NativeSqlValueEncoding) {
    match native_type {
        "BOOL" => (NativeSqlLogicalType::Boolean, NativeSqlValueEncoding::Json),
        "INT2" | "INT4" | "INT8" => (NativeSqlLogicalType::Integer, NativeSqlValueEncoding::Json),
        "FLOAT4" | "FLOAT8" => (NativeSqlLogicalType::Number, NativeSqlValueEncoding::Text),
        "NUMERIC" | "MONEY" => (NativeSqlLogicalType::Decimal, NativeSqlValueEncoding::Text),
        "CHAR" | "NAME" | "TEXT" | "BPCHAR" | "VARCHAR" => {
            (NativeSqlLogicalType::String, NativeSqlValueEncoding::Json)
        }
        "JSON" | "JSONB" => (NativeSqlLogicalType::Json, NativeSqlValueEncoding::Json),
        "DATE" => (NativeSqlLogicalType::Date, NativeSqlValueEncoding::Text),
        "TIME" | "TIMETZ" => (NativeSqlLogicalType::Time, NativeSqlValueEncoding::Text),
        "TIMESTAMP" | "TIMESTAMPTZ" => {
            (NativeSqlLogicalType::DateTime, NativeSqlValueEncoding::Text)
        }
        "UUID" => (NativeSqlLogicalType::Uuid, NativeSqlValueEncoding::Text),
        "BYTEA" => (NativeSqlLogicalType::Binary, NativeSqlValueEncoding::Base64),
        _ => (NativeSqlLogicalType::Native, NativeSqlValueEncoding::Text),
    }
}

fn encode_row(
    row: &PgRow,
    columns: &[NativeSqlColumn],
) -> Result<Vec<Value>, ProviderRuntimeError> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| encode_value(row, index, column))
        .collect()
}

fn encode_value(
    row: &PgRow,
    index: usize,
    column: &NativeSqlColumn,
) -> Result<Value, ProviderRuntimeError> {
    let raw = row.try_get_raw(index).map_err(map_sqlx_error)?;
    if raw.is_null() {
        return Ok(Value::Null);
    }

    let value = match (column.logical_type, column.encoding) {
        (NativeSqlLogicalType::Boolean, NativeSqlValueEncoding::Json) => {
            Value::Bool(row.try_get::<bool, _>(index).map_err(map_sqlx_error)?)
        }
        (NativeSqlLogicalType::Integer, NativeSqlValueEncoding::Json) => {
            match column.native_type.as_str() {
                "INT2" => Value::from(row.try_get::<i16, _>(index).map_err(map_sqlx_error)?),
                "INT4" => Value::from(row.try_get::<i32, _>(index).map_err(map_sqlx_error)?),
                "INT8" => Value::from(row.try_get::<i64, _>(index).map_err(map_sqlx_error)?),
                _ => return Err(unsupported_result_type(column)),
            }
        }
        (NativeSqlLogicalType::String, NativeSqlValueEncoding::Json) => {
            Value::String(row.try_get::<String, _>(index).map_err(map_sqlx_error)?)
        }
        (NativeSqlLogicalType::Json, NativeSqlValueEncoding::Json) => {
            row.try_get::<Value, _>(index).map_err(map_sqlx_error)?
        }
        (NativeSqlLogicalType::Binary, NativeSqlValueEncoding::Base64) => {
            let bytes = row.try_get::<Vec<u8>, _>(index).map_err(map_sqlx_error)?;
            Value::String(BASE64_STANDARD.encode(bytes))
        }
        (_, NativeSqlValueEncoding::Text) => Value::String(
            raw.as_str()
                .map_err(|_| unsupported_result_type(column))?
                .to_string(),
        ),
        (_, NativeSqlValueEncoding::Base64) => Value::String(
            BASE64_STANDARD.encode(
                raw.as_bytes()
                    .map_err(|_| unsupported_result_type(column))?,
            ),
        ),
        _ => return Err(unsupported_result_type(column)),
    };
    Ok(value)
}

fn unsupported_result_type(column: &NativeSqlColumn) -> ProviderRuntimeError {
    ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::ProviderInvalidResponse,
        format!(
            "unsupported_result_type: PostgreSQL type {} cannot be encoded as {:?}",
            column.native_type, column.encoding
        ),
    )
    .with_provider_details(json!({
        "code": "unsupported_result_type",
        "native_type": column.native_type,
    }))
}

fn map_sqlx_error(error: sqlx::Error) -> ProviderRuntimeError {
    match error {
        sqlx::Error::Database(database) => {
            let code = database.code().map(|code| code.into_owned());
            let detail = database
                .try_downcast_ref::<PgDatabaseError>()
                .and_then(PgDatabaseError::detail)
                .map(ToOwned::to_owned);
            let mut provider_error = ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ProviderUpstreamError,
                database.message(),
            );
            if let Some(code) = code.as_deref() {
                provider_error = provider_error.with_provider_summary(code);
            }
            provider_error.with_provider_details(json!({
                "code": code,
                "detail": detail,
            }))
        }
        other => ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderTransportUnavailable,
            other.to_string(),
        )
        .with_provider_summary("outcome_unknown")
        .with_provider_details(json!({ "code": "outcome_unknown" })),
    }
}
