use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

use time::{Duration, OffsetDateTime};
use uuid::Uuid;

const REAUTH_TTL: Duration = Duration::minutes(5);
const RECOVERY_INTENT_TTL: Duration = Duration::minutes(2);
const MAX_REAUTH_CHALLENGES: usize = 256;

#[derive(Debug, Clone)]
struct ReauthChallenge {
    actor_user_id: Uuid,
    session_id: String,
    backup_set_id: domain::BackupSetId,
    plan_digest: domain::ContentDigest,
    exact_backup_name: String,
    expires_at: OffsetDateTime,
    consumed: bool,
}

#[derive(Default)]
struct AuthorizationState {
    challenges: BTreeMap<Uuid, ReauthChallenge>,
}

fn state() -> &'static Mutex<AuthorizationState> {
    static STATE: OnceLock<Mutex<AuthorizationState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(AuthorizationState::default()))
}

pub struct IssuedReauthChallenge {
    pub token: Uuid,
    pub expires_at: OffsetDateTime,
}

pub fn issue_reauth_challenge(
    actor_user_id: Uuid,
    session_id: &str,
    backup_set_id: domain::BackupSetId,
    plan_digest: domain::ContentDigest,
    exact_backup_name: &str,
) -> IssuedReauthChallenge {
    let now = OffsetDateTime::now_utc();
    let token = Uuid::now_v7();
    let expires_at = now + REAUTH_TTL;
    let mut state = state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state
        .challenges
        .retain(|_, challenge| challenge.expires_at > now);
    while state.challenges.len() >= MAX_REAUTH_CHALLENGES {
        let Some(oldest) = state.challenges.keys().next().copied() else {
            break;
        };
        state.challenges.remove(&oldest);
    }
    state.challenges.insert(
        token,
        ReauthChallenge {
            actor_user_id,
            session_id: session_id.to_owned(),
            backup_set_id,
            plan_digest,
            exact_backup_name: exact_backup_name.to_owned(),
            expires_at,
            consumed: false,
        },
    );
    IssuedReauthChallenge { token, expires_at }
}

pub fn consume_reauth_challenge(
    token: Uuid,
    actor_user_id: Uuid,
    session_id: &str,
    backup_set_id: domain::BackupSetId,
    plan_digest: &domain::ContentDigest,
    exact_backup_name: &str,
) -> Result<(), &'static str> {
    let now = OffsetDateTime::now_utc();
    let mut state = state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let challenge = state
        .challenges
        .get_mut(&token)
        .ok_or("recovery_reauth_challenge")?;
    if challenge.expires_at <= now {
        return Err("recovery_reauth_expired");
    }
    if challenge.actor_user_id != actor_user_id
        || challenge.session_id != session_id
        || challenge.backup_set_id != backup_set_id
        || &challenge.plan_digest != plan_digest
        || challenge.exact_backup_name != exact_backup_name
    {
        return Err("recovery_reauth_binding");
    }
    if challenge.consumed {
        return Err("recovery_reauth_replayed");
    }
    challenge.consumed = true;
    Ok(())
}

pub const fn recovery_intent_ttl() -> Duration {
    RECOVERY_INTENT_TTL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reauth_challenge_is_bound_and_single_use() {
        let actor = Uuid::now_v7();
        let backup = domain::BackupSetId::new();
        let digest = domain::ContentDigest::try_from("a".repeat(64)).unwrap();
        let challenge =
            issue_reauth_challenge(actor, "session-a", backup, digest.clone(), "backup-a");
        assert!(consume_reauth_challenge(
            challenge.token,
            actor,
            "session-b",
            backup,
            &digest,
            "backup-a"
        )
        .is_err());
        assert!(consume_reauth_challenge(
            challenge.token,
            actor,
            "session-a",
            domain::BackupSetId::new(),
            &digest,
            "backup-a"
        )
        .is_err());
        assert!(consume_reauth_challenge(
            challenge.token,
            actor,
            "session-a",
            backup,
            &digest,
            "backup-a"
        )
        .is_ok());
        assert_eq!(
            consume_reauth_challenge(
                challenge.token,
                actor,
                "session-a",
                backup,
                &digest,
                "backup-a"
            ),
            Err("recovery_reauth_replayed")
        );
    }
}
