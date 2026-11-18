use std::sync::atomic::{AtomicBool, Ordering};

use control_plane::ports::BackupRepositoryError;

use super::*;

fn event(subject: BackupJournalSubject, sequence: u64) -> BackupJournalEvent {
    BackupJournalEvent {
        event_id: Uuid::now_v7(),
        sequence,
        subject,
        backup_set_id: BackupSetId::new(),
        actor_user_id: None,
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        event: BackupJournalEventKind::BackupStateChanged {
            state: BackupJobState::Capturing,
        },
    }
}

fn journal_path(root: &std::path::Path, job_id: BackupJobId) -> PathBuf {
    root.join("journal")
        .join(format!("backup-{}", job_id.as_uuid()))
}

#[tokio::test]
async fn journal_reads_committed_prefix_and_rejects_published_corruption() {
    let root = temporary_root("journal-publication");
    let repository = LocalBackupRepository::open(&root, &[]).await.unwrap();
    let job_id = BackupJobId::new();
    let subject = BackupJournalSubject::Backup(job_id);
    let journal = journal_path(&root, job_id);
    let legacy = journal.join(format!("{:020}", 1));
    tokio::fs::create_dir_all(&legacy).await.unwrap();
    let committed = event(subject, 1);
    tokio::fs::write(
        legacy.join("event.json"),
        serde_json::to_vec(&committed).unwrap(),
    )
    .await
    .unwrap();

    // An unfinished append is not committed, including after writer interruption.
    let pending = journal.join(format!(".pending-{}", Uuid::now_v7()));
    tokio::fs::create_dir(&pending).await.unwrap();
    tokio::fs::write(pending.join("event.json"), b"{\"event_id\":")
        .await
        .unwrap();
    let read = repository.read_journal(subject).await.unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].event_id, committed.event_id);

    let corrupt = journal.join(format!("{:020}", 2));
    tokio::fs::create_dir(&corrupt).await.unwrap();
    assert!(matches!(
        repository.read_journal(subject).await,
        Err(BackupRepositoryError::Integrity)
    ));
    tokio::fs::write(corrupt.join("event.json"), b"{\"event_id\":")
        .await
        .unwrap();
    assert!(matches!(
        repository.read_journal(subject).await,
        Err(BackupRepositoryError::Integrity)
    ));
    tokio::fs::remove_dir_all(&corrupt).await.unwrap();

    // Only the repository's exact temporary naming convention can be skipped.
    tokio::fs::create_dir(journal.join(".pending-not-a-uuid"))
        .await
        .unwrap();
    assert!(matches!(
        repository.read_journal(subject).await,
        Err(BackupRepositoryError::Integrity)
    ));
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn concurrent_duplicate_journal_append_commits_exactly_once() {
    let root = temporary_root("journal-duplicate");
    let first = LocalBackupRepository::open(&root, &[]).await.unwrap();
    let second = LocalBackupRepository::open(&root, &[]).await.unwrap();
    let subject = BackupJournalSubject::Backup(BackupJobId::new());
    let left = event(subject, 1);
    let right = event(subject, 1);
    let (left_result, right_result) = tokio::join!(
        first.append_journal_event(&left),
        second.append_journal_event(&right)
    );
    assert!(matches!(
        (&left_result, &right_result),
        (Ok(()), Err(BackupRepositoryError::Conflict))
            | (Err(BackupRepositoryError::Conflict), Ok(()))
    ));
    let read = first.read_journal(subject).await.unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(
        read[0].event_id,
        if left_result.is_ok() {
            left.event_id
        } else {
            right.event_id
        }
    );
    let mut entries = tokio::fs::read_dir(journal_path(
        &root,
        match subject {
            BackupJournalSubject::Backup(id) => id,
            _ => unreachable!(),
        },
    ))
    .await
    .unwrap();
    let mut count = 0;
    while entries.next_entry().await.unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, 1, "failed duplicate staging must be removed");
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn journal_append_does_not_replace_existing_empty_committed_directory() {
    let root = temporary_root("journal-empty-committed");
    let repository = LocalBackupRepository::open(&root, &[]).await.unwrap();
    let job_id = BackupJobId::new();
    let subject = BackupJournalSubject::Backup(job_id);
    let committed = journal_path(&root, job_id).join(format!("{:020}", 1));
    tokio::fs::create_dir_all(&committed).await.unwrap();
    assert!(matches!(
        repository.append_journal_event(&event(subject, 1)).await,
        Err(BackupRepositoryError::Conflict)
    ));
    assert!(!tokio::fs::try_exists(committed.join("event.json"))
        .await
        .unwrap());
    assert!(matches!(
        repository.read_journal(subject).await,
        Err(BackupRepositoryError::Integrity)
    ));
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn concurrent_journal_reads_only_observe_complete_ordered_events() {
    let root = temporary_root("journal-read-append");
    let writer = LocalBackupRepository::open(&root, &[]).await.unwrap();
    let reader = LocalBackupRepository::open(&root, &[]).await.unwrap();
    let subject = BackupJournalSubject::Backup(BackupJobId::new());
    let finished = AtomicBool::new(false);
    let expected = (1..=32)
        .map(|sequence| event(subject, sequence))
        .collect::<Vec<_>>();
    tokio::join!(
        async {
            for event in &expected {
                writer.append_journal_event(event).await.unwrap();
            }
            finished.store(true, Ordering::Release);
        },
        async {
            while !finished.load(Ordering::Acquire) {
                let snapshot = reader.read_journal(subject).await.unwrap();
                for (actual, expected) in snapshot.iter().zip(&expected) {
                    assert_eq!(actual.sequence, expected.sequence);
                    assert_eq!(actual.event_id, expected.event_id);
                }
                tokio::task::yield_now().await;
            }
        }
    );
    let snapshot = reader.read_journal(subject).await.unwrap();
    assert_eq!(snapshot.len(), expected.len());
    for (actual, expected) in snapshot.iter().zip(&expected) {
        assert_eq!(actual.event_id, expected.event_id);
    }
    tokio::fs::remove_dir_all(root).await.unwrap();
}
