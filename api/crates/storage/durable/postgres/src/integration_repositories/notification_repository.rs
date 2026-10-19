use crate::{
    secret_crypto::{decrypt_secret_json_with_aad, encrypt_secret_json_with_aad},
    PgControlPlaneStore,
};
use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{
        NotificationAuthorizationSnapshot, NotificationMessageListInput, NotificationRepository,
        SaveNotificationAuthorizationInput, SaveNotificationInstanceInput,
    },
    ControlPlaneContractError as Error,
};
use domain::{
    NotificationInstanceRecord, NotificationInstanceStatus, NotificationMessagePage,
    NotificationRecipientOption, SystemNotificationRecord,
};
use serde_json::{json, Value};
use sqlx::{postgres::PgRow, Row};
use time::OffsetDateTime;
use uuid::Uuid;

fn instance(row: PgRow) -> Result<NotificationInstanceRecord> {
    Ok(NotificationInstanceRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        type_code: row.get("type_code"),
        installation_id: row.get("installation_id"),
        display_name: row.get("display_name"),
        status: match row.get::<String, _>("status").as_str() {
            "enabled" => NotificationInstanceStatus::Enabled,
            "disabled" => NotificationInstanceStatus::Disabled,
            _ => return Err(Error::Conflict("notification_status").into()),
        },
        config_json: row.get("config_json"),
        configured_secret_fields: serde_json::from_value(row.get("configured_secret_fields"))?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
fn message(row: PgRow) -> SystemNotificationRecord {
    SystemNotificationRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        instance_id: row.get("instance_id"),
        recipient_user_id: row.get("recipient_user_id"),
        title: row.get("title"),
        content: row.get("content"),
        read_at: row.get("read_at"),
        created_at: row.get("created_at"),
    }
}
fn send_log(row: PgRow) -> Result<domain::NotificationSendLog> {
    Ok(domain::NotificationSendLog {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        actor_user_id: row.get("actor_user_id"),
        instance_id: row.get("instance_id"),
        display_name: row.get("display_name"),
        type_code: row.get("type_code"),
        source: match row.get::<String, _>("source").as_str() {
            "api" => domain::NotificationSource::Api,
            "workflow" => domain::NotificationSource::Workflow,
            _ => return Err(Error::Conflict("notification_log_source").into()),
        },
        status: row.get("status"),
        failure_code: row.get("failure_code"),
        title: row.get("title"),
        content: row.get("content"),
        recipient_count: row.get("recipient_count"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    })
}
fn parse_cursor(cursor: Option<String>) -> Result<Option<(OffsetDateTime, Uuid)>> {
    cursor
        .map(|cursor| -> Result<_> {
            let (ts, id) = cursor
                .split_once(':')
                .ok_or(Error::InvalidInput("cursor"))?;
            Ok((
                OffsetDateTime::from_unix_timestamp_nanos(
                    ts.parse().map_err(|_| Error::InvalidInput("cursor"))?,
                )
                .map_err(|_| Error::InvalidInput("cursor"))?,
                Uuid::parse_str(id).map_err(|_| Error::InvalidInput("cursor"))?,
            ))
        })
        .transpose()
}
pub(crate) fn aad(workspace: Uuid, id: Uuid) -> Vec<u8> {
    format!("notification-instance:{workspace}:{id}").into_bytes()
}

pub(crate) fn managed_aad(workspace: Uuid, id: Uuid) -> Vec<u8> {
    format!("notification-managed-authorization:{workspace}:{id}").into_bytes()
}

#[async_trait]
impl NotificationRepository for PgControlPlaneStore {
    async fn notification_authorization_view(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> Result<Option<extension_contracts::NotificationAuthorizationView>> {
        let value: Option<Value> = sqlx::query_scalar(
            "SELECT authorization_json FROM notification_instances WHERE workspace_id=$1 AND id=$2",
        )
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(self.pool())
        .await?
        .ok_or(Error::NotFound("notification_instance"))?;
        value
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| Error::Conflict("notification_authorization_state").into())
    }
    async fn resolve_notification_authorization(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        master_key: &str,
    ) -> Result<NotificationAuthorizationSnapshot> {
        let row = sqlx::query("SELECT authorization_json,encrypted_managed_secret_json,authorization_revision FROM notification_instances WHERE workspace_id=$1 AND id=$2")
            .bind(workspace_id).bind(id).fetch_optional(self.pool()).await?
            .ok_or(Error::NotFound("notification_instance"))?;
        let authorization: Option<Value> = row.get("authorization_json");
        let encrypted: Option<Value> = row.get("encrypted_managed_secret_json");
        let managed_secrets = match encrypted {
            Some(value) => serde_json::from_value(decrypt_secret_json_with_aad(
                &value,
                master_key,
                &managed_aad(workspace_id, id),
            )?)?,
            None => Default::default(),
        };
        Ok(NotificationAuthorizationSnapshot {
            authorization: authorization
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| Error::Conflict("notification_authorization_state"))?,
            managed_secrets,
            revision: row.get("authorization_revision"),
        })
    }
    async fn save_notification_authorization(
        &self,
        input: SaveNotificationAuthorizationInput,
    ) -> Result<Option<i64>> {
        input
            .authorization
            .validate()
            .map_err(|_| Error::Conflict("notification_authorization_state"))?;
        let encrypted = encrypt_secret_json_with_aad(
            &serde_json::to_value(input.managed_secrets)?,
            &input.master_key,
            &managed_aad(input.workspace_id, input.instance_id),
        )?;
        Ok(sqlx::query_scalar("UPDATE notification_instances SET authorization_json=$4,encrypted_managed_secret_json=$5,authorization_revision=authorization_revision+1,updated_at=now() WHERE workspace_id=$1 AND id=$2 AND authorization_revision=$3 RETURNING authorization_revision")
            .bind(input.workspace_id).bind(input.instance_id).bind(input.expected_revision)
            .bind(serde_json::to_value(input.authorization)?).bind(encrypted)
            .fetch_optional(self.pool()).await?)
    }
    async fn create_notification_send_log(&self, log: domain::NotificationSendLog) -> Result<()> {
        sqlx::query("INSERT INTO notification_send_logs (id,workspace_id,actor_user_id,instance_id,display_name,type_code,source,status,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,'pending',$8)")
            .bind(log.id).bind(log.workspace_id).bind(log.actor_user_id).bind(log.instance_id).bind(log.display_name).bind(log.type_code).bind(log.source.as_str()).bind(log.created_at).execute(self.pool()).await?;
        Ok(())
    }
    async fn complete_notification_send_log(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        status: &str,
        failure_code: Option<&str>,
    ) -> Result<()> {
        let count = sqlx::query("UPDATE notification_send_logs SET status=$3,failure_code=$4,completed_at=now() WHERE workspace_id=$1 AND id=$2 AND status='pending'")
            .bind(workspace_id).bind(id).bind(status).bind(failure_code).execute(self.pool()).await?.rows_affected();
        if count != 1 {
            return Err(Error::Conflict("notification_log_state").into());
        }
        Ok(())
    }
    async fn get_notification_send_log(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> Result<Option<domain::NotificationSendLog>> {
        sqlx::query("SELECT * FROM notification_send_logs WHERE workspace_id=$1 AND id=$2")
            .bind(workspace_id)
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .map(send_log)
            .transpose()
    }
    async fn list_notification_send_logs(
        &self,
        workspace_id: Uuid,
        input: control_plane_contracts::ports::NotificationLogListInput,
    ) -> Result<domain::NotificationSendLogPage> {
        let page = input.page.unwrap_or(1);
        let size = input.page_size.unwrap_or(50);
        if page == 0 || size == 0 || size > 100 {
            return Err(Error::InvalidInput("pagination").into());
        }
        if input
            .status
            .as_deref()
            .is_some_and(|s| !["pending", "success", "failure", "unknown"].contains(&s))
        {
            return Err(Error::InvalidInput("status").into());
        }
        let parse_time = |s: Option<String>| -> Result<Option<OffsetDateTime>> {
            s.map(|s| {
                OffsetDateTime::parse(&s, &time::format_description::well_known::Rfc3339)
                    .map_err(|_| Error::InvalidInput("created_at").into())
            })
            .transpose()
        };
        let from = parse_time(input.created_from)?;
        let to = parse_time(input.created_to)?;
        if from.zip(to).is_some_and(|(a, b)| a > b) {
            return Err(Error::InvalidInput("created_at").into());
        }
        let predicate = " FROM notification_send_logs WHERE workspace_id=$1 AND ($2::uuid IS NULL OR instance_id=$2) AND ($3::text IS NULL OR type_code=$3) AND ($4::text IS NULL OR status=$4) AND ($5::text IS NULL OR source=$5) AND ($6::timestamptz IS NULL OR created_at >= $6) AND ($7::timestamptz IS NULL OR created_at <= $7)";
        let source = input.source.map(|s| s.as_str());
        // SQL safety: Only the static notification predicate is interpolated; all filter values remain bound.
        let total: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*){predicate}")))
                .bind(workspace_id)
                .bind(input.instance_id)
                .bind(&input.type_code)
                .bind(&input.status)
                .bind(source)
                .bind(from)
                .bind(to)
                .fetch_one(self.pool())
                .await?;
        let items = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT *{predicate} ORDER BY created_at DESC,id DESC LIMIT $8 OFFSET $9"
        )))
        .bind(workspace_id)
        .bind(input.instance_id)
        .bind(&input.type_code)
        .bind(&input.status)
        .bind(source)
        .bind(from)
        .bind(to)
        .bind(i64::from(size))
        .bind(i64::from(page - 1) * i64::from(size))
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(send_log)
        .collect::<Result<Vec<_>>>()?;
        Ok(domain::NotificationSendLogPage {
            items,
            total,
            page,
            page_size: size,
        })
    }
    async fn list_notification_channels(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        input: control_plane_contracts::ports::NotificationChannelListInput,
    ) -> Result<domain::NotificationChannelPage> {
        let size = input.page_size.unwrap_or(50);
        if size == 0 || size > 100 {
            return Err(Error::InvalidInput("page_size").into());
        }
        let cursor = parse_cursor(input.cursor)?;
        let read_status = input.read_status.unwrap_or_default().as_str();
        // Filter before choosing the latest message; aggregate unread facts independently.
        // LEFT JOIN preserves inbox history after instance deletion. All stored inbox rows are
        // system messages, so missing instance metadata has a formal system-type fallback.
        let mut items = sqlx::query("WITH latest AS (SELECT DISTINCT ON (instance_id) instance_id,title,content,created_at FROM system_notifications WHERE workspace_id=$1 AND recipient_user_id=$2 AND ($3='all' OR ($3='unread' AND read_at IS NULL) OR ($3='read' AND read_at IS NOT NULL)) ORDER BY instance_id,created_at DESC,id DESC), unread AS (SELECT instance_id,count(*) AS unread_count FROM system_notifications WHERE workspace_id=$1 AND recipient_user_id=$2 AND read_at IS NULL GROUP BY instance_id) SELECT l.instance_id,coalesce(i.display_name,'Deleted notification channel') AS display_name,coalesce(i.type_code,'system') AS type_code,i.id IS NULL AS deleted,l.title,l.content,l.created_at,coalesce(u.unread_count,0) AS unread_count FROM latest l LEFT JOIN notification_instances i ON i.workspace_id=$1 AND i.id=l.instance_id LEFT JOIN unread u ON u.instance_id=l.instance_id WHERE ($4::timestamptz IS NULL OR (l.created_at,l.instance_id)<($4,$5)) ORDER BY l.created_at DESC,l.instance_id DESC LIMIT $6")
            .bind(workspace_id).bind(user_id).bind(read_status).bind(cursor.map(|c|c.0)).bind(cursor.map(|c|c.1)).bind(i64::from(size)+1).fetch_all(self.pool()).await?.into_iter().map(|r|domain::NotificationChannel {
                instance_id:r.get("instance_id"),display_name:r.get("display_name"),type_code:r.get("type_code"),deleted:r.get("deleted"),latest_title:r.get("title"),latest_content:r.get("content"),latest_created_at:r.get("created_at"),unread_count:r.get("unread_count"),
            }).collect::<Vec<_>>();
        let more = items.len() > size as usize;
        items.truncate(size as usize);
        let next_cursor = if more {
            items.last().map(|r| {
                format!(
                    "{}:{}",
                    r.latest_created_at.unix_timestamp_nanos(),
                    r.instance_id
                )
            })
        } else {
            None
        };
        Ok(domain::NotificationChannelPage { items, next_cursor })
    }
    async fn mark_system_notification_unread(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<Option<SystemNotificationRecord>> {
        Ok(sqlx::query("UPDATE system_notifications SET read_at=NULL WHERE workspace_id=$1 AND recipient_user_id=$2 AND id=$3 RETURNING *").bind(workspace_id).bind(user_id).bind(id).fetch_optional(self.pool()).await?.map(message))
    }
    async fn mark_notification_channel_read(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        instance_id: Uuid,
    ) -> Result<u64> {
        Ok(sqlx::query("UPDATE system_notifications SET read_at=now() WHERE workspace_id=$1 AND recipient_user_id=$2 AND instance_id=$3 AND read_at IS NULL").bind(workspace_id).bind(user_id).bind(instance_id).execute(self.pool()).await?.rows_affected())
    }
    async fn bootstrap_system_notification_instances(&self) -> Result<()> {
        sqlx::query("INSERT INTO notification_instances (id,workspace_id,type_code,display_name,is_builtin) SELECT gen_random_uuid(),id,'system','System notification',true FROM workspaces ON CONFLICT (workspace_id) WHERE is_builtin DO NOTHING").execute(self.pool()).await?;
        Ok(())
    }
    async fn list_notification_instances(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<NotificationInstanceRecord>> {
        sqlx::query(
            "SELECT * FROM notification_instances WHERE workspace_id=$1 ORDER BY created_at,id",
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(instance)
        .collect()
    }
    async fn get_notification_instance(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> Result<Option<NotificationInstanceRecord>> {
        sqlx::query("SELECT * FROM notification_instances WHERE workspace_id=$1 AND id=$2")
            .bind(workspace_id)
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .map(instance)
            .transpose()
    }
    async fn save_notification_instance(
        &self,
        input: SaveNotificationInstanceInput,
    ) -> Result<NotificationInstanceRecord> {
        let r = input.record;
        if let Some(view) = &input.initial_authorization {
            view.validate()
                .map_err(|_| Error::Conflict("notification_authorization_state"))?;
        }
        let encrypted = encrypt_secret_json_with_aad(
            &input.secret_config_json,
            &input.master_key,
            &aad(r.workspace_id, r.id),
        )?;
        let row = if input.create {
            sqlx::query("INSERT INTO notification_instances (id,workspace_id,type_code,installation_id,display_name,status,config_json,configured_secret_fields,encrypted_secret_json,authorization_json) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING *")
                .bind(r.id).bind(r.workspace_id).bind(&r.type_code).bind(r.installation_id).bind(&r.display_name).bind(r.status.as_str()).bind(&r.config_json).bind(json!(r.configured_secret_fields)).bind(encrypted).bind(input.initial_authorization.map(serde_json::to_value).transpose()?).fetch_one(self.pool()).await?
        } else {
            sqlx::query("UPDATE notification_instances SET display_name=$3,status=$4,config_json=$5,configured_secret_fields=$6,encrypted_secret_json=$7,updated_at=now() WHERE workspace_id=$1 AND id=$2 AND updated_at=$8 RETURNING *")
                .bind(r.workspace_id).bind(r.id).bind(&r.display_name).bind(r.status.as_str()).bind(&r.config_json).bind(json!(r.configured_secret_fields)).bind(encrypted).bind(r.updated_at).fetch_optional(self.pool()).await?.ok_or(Error::Conflict("notification_instance_changed"))?
        };
        instance(row)
    }
    async fn delete_notification_instance(&self, workspace_id: Uuid, id: Uuid) -> Result<bool> {
        Ok(
            sqlx::query("DELETE FROM notification_instances WHERE workspace_id=$1 AND id=$2")
                .bind(workspace_id)
                .bind(id)
                .execute(self.pool())
                .await?
                .rows_affected()
                > 0,
        )
    }
    async fn resolve_notification_secret_config(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        master_key: &str,
    ) -> Result<Value> {
        let value:Option<Value>=sqlx::query_scalar("SELECT encrypted_secret_json FROM notification_instances WHERE workspace_id=$1 AND id=$2").bind(workspace_id).bind(id).fetch_optional(self.pool()).await?.ok_or(Error::NotFound("notification_instance"))?;
        match value {
            Some(value) => decrypt_secret_json_with_aad(&value, master_key, &aad(workspace_id, id)),
            None => Ok(json!({})),
        }
    }
    async fn notification_recipient_options(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<NotificationRecipientOption>> {
        Ok(sqlx::query("SELECT u.id,coalesce(nullif(u.name,''),nullif(u.nickname,''),u.account) AS display_name FROM workspace_memberships wm JOIN users u ON u.id=wm.user_id WHERE wm.workspace_id=$1 AND u.status='active' ORDER BY display_name,u.id").bind(workspace_id).fetch_all(self.pool()).await?.into_iter().map(|r|NotificationRecipientOption{id:r.get("id"),display_name:r.get("display_name")}).collect())
    }
    async fn commit_system_notifications(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        recipients: &[Uuid],
        send_log_id: Uuid,
        title: &str,
        content: &str,
    ) -> Result<Vec<SystemNotificationRecord>> {
        let mut tx = self.pool().begin().await?;
        let status:Option<String>=sqlx::query_scalar("SELECT status FROM notification_instances WHERE workspace_id=$1 AND id=$2 AND type_code='system' FOR SHARE").bind(workspace_id).bind(instance_id).fetch_optional(&mut *tx).await?;
        match status.as_deref() {
            Some("enabled") => {}
            Some(_) => return Err(Error::Conflict("instance_disabled").into()),
            None => return Err(Error::NotFound("notification_instance").into()),
        }
        let valid:Vec<Uuid>=sqlx::query_scalar("SELECT u.id FROM workspace_memberships wm JOIN users u ON u.id=wm.user_id WHERE wm.workspace_id=$1 AND u.id=ANY($2) AND u.status='active' FOR SHARE OF wm,u").bind(workspace_id).bind(recipients).fetch_all(&mut *tx).await?;
        if valid.len() != recipients.len() {
            return Err(Error::InvalidInput("notification_recipients").into());
        }
        let mut records = Vec::with_capacity(recipients.len());
        for user_id in recipients {
            records.push(message(sqlx::query("INSERT INTO system_notifications (id,workspace_id,instance_id,recipient_user_id,title,content) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *").bind(Uuid::now_v7()).bind(workspace_id).bind(instance_id).bind(user_id).bind(title).bind(content).fetch_one(&mut *tx).await?));
        }
        let updated = sqlx::query("UPDATE notification_send_logs SET status='success',title=$3,content=$4,recipient_count=$5,completed_at=now() WHERE workspace_id=$1 AND id=$2 AND instance_id=$6 AND status='pending'")
            .bind(workspace_id).bind(send_log_id).bind(title).bind(content).bind(recipients.len() as i64).bind(instance_id).execute(&mut *tx).await?.rows_affected();
        if updated != 1 {
            return Err(Error::Conflict("notification_log_state").into());
        }
        tx.commit().await?;
        Ok(records)
    }
    async fn list_system_notifications(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        input: NotificationMessageListInput,
    ) -> Result<NotificationMessagePage> {
        let size = input.page_size.unwrap_or(50);
        if size == 0 || size > 100 {
            return Err(Error::InvalidInput("page_size").into());
        }
        let cursor = parse_cursor(input.cursor)?;
        let read_status = input
            .read_status
            .unwrap_or(if input.unread_only {
                domain::NotificationReadStatus::Unread
            } else {
                domain::NotificationReadStatus::All
            })
            .as_str();
        let mut rows = sqlx::query("SELECT * FROM system_notifications WHERE workspace_id=$1 AND recipient_user_id=$2 AND ($3='all' OR ($3='unread' AND read_at IS NULL) OR ($3='read' AND read_at IS NOT NULL)) AND ($4::timestamptz IS NULL OR (created_at,id)<($4,$5)) AND ($7::uuid IS NULL OR instance_id=$7) ORDER BY created_at DESC,id DESC LIMIT $6")
            .bind(workspace_id).bind(user_id).bind(read_status).bind(cursor.map(|c|c.0)).bind(cursor.map(|c|c.1)).bind(i64::from(size)+1).bind(input.instance_id).fetch_all(self.pool()).await?.into_iter().map(message).collect::<Vec<_>>();
        let more = rows.len() > size as usize;
        rows.truncate(size as usize);
        let next_cursor = if more {
            rows.last()
                .map(|r| format!("{}:{}", r.created_at.unix_timestamp_nanos(), r.id))
        } else {
            None
        };
        Ok(NotificationMessagePage {
            items: rows,
            next_cursor,
            unread_count: sqlx::query_scalar("SELECT count(*) FROM system_notifications WHERE workspace_id=$1 AND recipient_user_id=$2 AND read_at IS NULL AND ($3::uuid IS NULL OR instance_id=$3)").bind(workspace_id).bind(user_id).bind(input.instance_id).fetch_one(self.pool()).await?,
        })
    }
    async fn system_notification_unread_count(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<i64> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM system_notifications WHERE workspace_id=$1 AND recipient_user_id=$2 AND read_at IS NULL").bind(workspace_id).bind(user_id).fetch_one(self.pool()).await?)
    }
    async fn mark_system_notification_read(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<Option<SystemNotificationRecord>> {
        Ok(sqlx::query("UPDATE system_notifications SET read_at=coalesce(read_at,now()) WHERE workspace_id=$1 AND recipient_user_id=$2 AND id=$3 RETURNING *").bind(workspace_id).bind(user_id).bind(id).fetch_optional(self.pool()).await?.map(message))
    }
    async fn mark_all_system_notifications_read(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<u64> {
        Ok(sqlx::query("UPDATE system_notifications SET read_at=now() WHERE workspace_id=$1 AND recipient_user_id=$2 AND read_at IS NULL").bind(workspace_id).bind(user_id).execute(self.pool()).await?.rows_affected())
    }
}
