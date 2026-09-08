use super::{filesystem::StagedArtifactPath, *};
use crate::ports::PluginInstallationLease;

// Field drop order is deliberate, including task panic/runtime shutdown: restore paths before
// dropping the connection owner. Request cancellation does not drop this task (see below).
struct InstallationPublication {
    archive: Option<StagedArtifactPath>,
    directory: Option<StagedArtifactPath>,
    lease: Option<Box<dyn PluginInstallationLease>>,
}

/// Own the COMMIT await together with the filesystem rollback guards. Dropping the request's
/// JoinHandle cannot abort publication. Filesystem acceptance/restoration happens under the lease.
pub(super) async fn publish_installation(
    lease: Box<dyn PluginInstallationLease>,
    directory: StagedArtifactPath,
    archive: Option<StagedArtifactPath>,
    input: CommitPluginInstallationInput,
) -> Result<domain::PluginInstallationRecord> {
    let publication = InstallationPublication {
        archive,
        directory: Some(directory),
        lease: Some(lease),
    };
    tokio::spawn(publication.commit(input)).await?
}

impl InstallationPublication {
    async fn commit(
        mut self,
        input: CommitPluginInstallationInput,
    ) -> Result<domain::PluginInstallationRecord> {
        let result = async {
            self.directory
                .as_mut()
                .expect("publication directory")
                .activate()?;
            if let Some(archive) = self.archive.as_mut() {
                archive.activate()?;
            }
            self.lease
                .as_mut()
                .expect("publication lease")
                .commit(&input)
                .await
        }
        .await;
        match result {
            Ok(record) => {
                self.directory
                    .take()
                    .expect("publication directory")
                    .finish();
                if let Some(archive) = self.archive.take() {
                    archive.finish();
                }
                // The commit already succeeded. Unlock failure closes the connection via Drop;
                // it cannot turn success into a filesystem rollback.
                let _ = self
                    .lease
                    .take()
                    .expect("publication lease")
                    .release()
                    .await;
                Ok(record)
            }
            Err(error) => {
                let archive_error = self
                    .archive
                    .as_mut()
                    .and_then(|value| value.rollback().err());
                let directory_error = self
                    .directory
                    .as_mut()
                    .expect("publication directory")
                    .rollback()
                    .err();
                // Drop retries unsuccessful guards before releasing the installation lock.
                self.archive.take();
                self.directory.take();
                let _ = self
                    .lease
                    .take()
                    .expect("publication lease")
                    .release()
                    .await;
                match archive_error.or(directory_error) {
                    Some(rollback) => {
                        Err(error
                            .context(format!("installation artifact rollback failed: {rollback}")))
                    }
                    None => Err(error),
                }
            }
        }
    }
}
