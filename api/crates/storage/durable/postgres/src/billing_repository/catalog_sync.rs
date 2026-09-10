use super::{
    pricing_rule, validate_pricing_schedule, write_pricing_rule, ControlPlaneError, PricingRule,
    PRICING_SELECT,
};
use anyhow::Result;
use control_plane_contracts::ports::PricingCatalogSyncSummary;
use sqlx::PgPool;
use std::collections::BTreeSet;

pub(super) async fn merge(
    pool: &PgPool,
    rules: &[PricingRule],
) -> Result<PricingCatalogSyncSummary> {
    let mut targets = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for rule in rules {
        rule.validate()?;
        if rule.source_kind != "official"
            || rule.source_catalog_id.as_deref() != Some(rule.id.to_string().as_str())
            || !targets.insert((&rule.provider_code, &rule.upstream_model_id))
            || !ids.insert(rule.id)
        {
            return Err(ControlPlaneError::InvalidInput("pricing_catalog_invalid").into());
        }
    }
    let mut transaction = pool.begin().await?;
    // Lock the finite catalog in a stable order before any row changes.
    for (provider, model) in targets {
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("{}:{}{}", provider.len(), provider, model))
            .execute(&mut *transaction)
            .await?;
    }
    let mut summary = PricingCatalogSyncSummary::default();
    for incoming in rules {
        let existing = sqlx::query(&(PRICING_SELECT.to_owned() +
            " where source_kind='official' and provider_code=$1 and upstream_model_id=$2 order by (source_catalog_id=$3) desc nulls last, enabled desc, created_at, id for update"))
            .bind(&incoming.provider_code).bind(&incoming.upstream_model_id).bind(&incoming.source_catalog_id)
            .fetch_all(&mut *transaction).await?
            .into_iter().map(pricing_rule).collect::<Result<Vec<_>>>()?;
        let mut rule = incoming.clone();
        // Compare at PostgreSQL's persisted precision so repeated pulls remain unchanged.
        let epoch = time::macros::datetime!(2000-01-01 0:00 UTC);
        for at in std::iter::once(&mut rule.effective_from).chain(rule.effective_to.iter_mut()) {
            *at = epoch + time::Duration::microseconds((*at - epoch).whole_microseconds() as i64);
        }
        for at in rule
            .local_time_start
            .iter_mut()
            .chain(rule.local_time_end.iter_mut())
        {
            *at -= time::Duration::nanoseconds(i64::from(at.nanosecond() % 1000));
        }
        for price in [
            &mut rule.input_token_unit_price,
            &mut rule.output_token_unit_price,
            &mut rule.cache_hit_token_unit_price,
            &mut rule.cache_write_token_unit_price,
        ] {
            *price = price
                .round_dp_with_strategy(18, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
        }
        if let Some(previous) = existing.first() {
            // Billing snapshots and reservations keep referring to the installed row ID.
            rule.id = previous.id;
            rule.created_by = previous.created_by;
            rule.created_at = previous.created_at;
            rule.updated_at = previous.updated_at;
            summary.retired += sqlx::query("update model_pricing_rules set enabled=false, updated_at=now() where source_kind='official' and provider_code=$1 and upstream_model_id=$2 and id<>$3 and enabled")
                .bind(&rule.provider_code).bind(&rule.upstream_model_id).bind(rule.id)
                .execute(&mut *transaction).await?.rows_affected() as usize;
            if &rule == previous {
                summary.unchanged += 1;
                continue;
            }
            summary.updated += 1;
        } else {
            // Insert-only keeps concurrent ID collisions from overwriting manual records.
            validate_pricing_schedule(&mut transaction, &rule).await?;
            insert_official_rule(&mut transaction, &rule).await?;
            summary.inserted += 1;
            continue;
        }
        write_pricing_rule(&mut transaction, &rule).await?;
    }
    transaction.commit().await?;
    Ok(summary)
}

async fn insert_official_rule(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rule: &PricingRule,
) -> Result<()> {
    let row = sqlx::query(r#"
            insert into model_pricing_rules (
                id, provider_code, upstream_model_id,
                input_token_unit_size, input_token_unit_price,
                output_token_unit_size, output_token_unit_price,
                cache_hit_token_unit_size, cache_hit_token_unit_price,
                cache_write_token_unit_size, cache_write_token_unit_price,
                currency_code, effective_from, effective_to, timezone, weekday_mask,
                local_time_start, local_time_end, priority, enabled,
                rules, source_kind,
                source_catalog_id, source_version, source_checksum, extensions, created_by
            ) values ($1,$2,$3,$4,$5::numeric,$6,$7::numeric,$8,$9::numeric,$10,$11::numeric,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27)
            on conflict do nothing
            returning id, provider_code, upstream_model_id,
                input_token_unit_size, input_token_unit_price::text as input_token_unit_price,
                output_token_unit_size, output_token_unit_price::text as output_token_unit_price,
                cache_hit_token_unit_size, cache_hit_token_unit_price::text as cache_hit_token_unit_price,
           cache_write_token_unit_size, cache_write_token_unit_price::text as cache_write_token_unit_price,
                currency_code, effective_from, effective_to, timezone, weekday_mask,
                local_time_start, local_time_end, priority, enabled,
                rules, source_kind,
                source_catalog_id, source_version, source_checksum, extensions, created_by, created_at, updated_at
        "#)
        .bind(rule.id).bind(&rule.provider_code).bind(&rule.upstream_model_id)
        .bind(rule.input_token_unit_size).bind(rule.input_token_unit_price.to_string())
        .bind(rule.output_token_unit_size).bind(rule.output_token_unit_price.to_string())
        .bind(rule.cache_hit_token_unit_size).bind(rule.cache_hit_token_unit_price.to_string())
        .bind(rule.cache_write_token_unit_size).bind(rule.cache_write_token_unit_price.to_string())
        .bind(&rule.currency_code).bind(rule.effective_from).bind(rule.effective_to)
        .bind(&rule.timezone).bind(rule.weekday_mask).bind(rule.local_time_start).bind(rule.local_time_end)
        .bind(rule.priority).bind(rule.enabled).bind(&rule.rules)
        .bind(&rule.source_kind).bind(&rule.source_catalog_id)
        .bind(&rule.source_version).bind(&rule.source_checksum).bind(&rule.extensions).bind(rule.created_by)
        .fetch_optional(&mut **transaction).await?;

    if row.is_none() {
        return Err(ControlPlaneError::Conflict("pricing_rule_conflict").into());
    }
    Ok(())
}
