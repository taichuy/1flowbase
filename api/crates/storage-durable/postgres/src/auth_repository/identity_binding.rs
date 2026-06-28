use anyhow::{anyhow, Result};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(crate) async fn insert_password_local_identities(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    account: &str,
    email: &str,
    phone: Option<&str>,
    actor_user_id: Option<Uuid>,
) -> Result<()> {
    for claim in domain::password_local_identity_claims(account, email, phone) {
        sqlx::query(
            r#"
            insert into user_auth_identities (
                id, user_id, authenticator_name, subject_type, subject_value, metadata,
                created_by, updated_by
            )
            values ($1, $2, $3, $4, $5, $6, $7, $7)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(&claim.authenticator_name)
        .bind(&claim.subject_type)
        .bind(&claim.subject_value)
        .bind(&claim.metadata)
        .bind(actor_user_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub(crate) async fn upsert_password_local_identities(
    pool: &PgPool,
    user: &domain::UserRecord,
) -> Result<()> {
    for claim in
        domain::password_local_identity_claims(&user.account, &user.email, user.phone.as_deref())
    {
        let bound_user_id: Uuid = sqlx::query_scalar(
            r#"
            insert into user_auth_identities (
                id, user_id, authenticator_name, subject_type, subject_value, metadata
            )
            values ($1, $2, $3, $4, $5, $6)
            on conflict (authenticator_name, subject_type, lower(subject_value))
            do update set subject_value = user_auth_identities.subject_value
            returning user_id
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(user.id)
        .bind(&claim.authenticator_name)
        .bind(&claim.subject_type)
        .bind(&claim.subject_value)
        .bind(&claim.metadata)
        .fetch_one(pool)
        .await?;

        if bound_user_id != user.id {
            return Err(anyhow!(
                "password-local identity conflict for {}:{}",
                claim.subject_type,
                claim.subject_value
            ));
        }
    }

    Ok(())
}

pub(crate) async fn replace_password_local_contact_identities(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    email: &str,
    phone: Option<&str>,
    actor_user_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"
        delete from user_auth_identities
        where user_id = $1
          and authenticator_name = $2
          and subject_type in ($3, $4)
        "#,
    )
    .bind(user_id)
    .bind(domain::PASSWORD_LOCAL_AUTHENTICATOR_NAME)
    .bind(domain::AUTH_SUBJECT_TYPE_EMAIL)
    .bind(domain::AUTH_SUBJECT_TYPE_PHONE)
    .execute(&mut **tx)
    .await?;

    for claim in domain::password_local_contact_identity_claims(email, phone) {
        sqlx::query(
            r#"
            insert into user_auth_identities (
                id, user_id, authenticator_name, subject_type, subject_value, metadata,
                created_by, updated_by
            )
            values ($1, $2, $3, $4, $5, $6, $7, $7)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(&claim.authenticator_name)
        .bind(&claim.subject_type)
        .bind(&claim.subject_value)
        .bind(&claim.metadata)
        .bind(actor_user_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}
