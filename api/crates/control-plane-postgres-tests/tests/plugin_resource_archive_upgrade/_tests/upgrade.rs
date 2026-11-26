use super::{
    fixture::{assert_permission_denied, Fixture},
    inputs::*,
    preservation::{self, Snapshot},
};
use anyhow::{ensure, Context, Result};
use control_plane::{plugin_management::*, ports::*};
use domain::ContributionAuthorizationStatus;
use std::fs;

pub async fn run(f: &Fixture, inputs: &Inputs) -> Result<()> {
    // U1/AC005: genuine authoritative old archive, no prepared installation inputs.
    let old = f.upload(&inputs.old, OLD_VERSION).await?;
    let old_id = old.result.installation.id;
    ensure!(
        old.manifest.resource_exports.is_empty(),
        "old release unexpectedly contains the new provider export"
    );
    ensure!(
        !old.manifest.data_models.is_empty(),
        "old package has no authoritative schema"
    );
    ensure!(
        f.store.list_managed_schema_ownership().await?.is_empty(),
        "upload performed schema activation"
    );
    let unapproved = f.authority().query(&f.root_actor, old_id).await?;
    assert_permission_denied(
        f.activation.prepare(&f.member, old_id).await,
        "managed_contribution_authorization_required",
    )?;
    ensure!(
        f.authority().query(&f.root_actor, old_id).await? == unapproved,
        "nonroot preparation minted authority"
    );
    ensure!(
        f.store.list_managed_schema_ownership().await?.is_empty(),
        "failed nonroot preparation mutated schema"
    );
    f.activation
        .prepare(&f.root_actor, old_id)
        .await
        .context("old archive Root activation preparation failed")?;
    f.assert_not_runtime_active(old_id).await?;
    let old_authority = f.authority().query(&f.root_actor, old_id).await?;
    ensure!(
        !old_authority.authorizations.is_empty(),
        "Root explicit preparation did not approve declared grants"
    );
    let seed = preservation::seed(f, &old).await?;
    let before = Snapshot::capture(f, old_id).await?;

    // A real new archive, packaged from the pinned BUSINESS candidate and a separately
    // freshly built binary. Identity/hash checks reject a relabelled old executable.
    let new = f.upload(&inputs.new, NEW_VERSION).await?;
    let new_id = new.result.installation.id;
    ensure!(
        new_id != old_id,
        "new version overwrote historical installation identity"
    );
    ensure!(
        new.runtime_sha256 == inputs.runtime_sha256 && new.runtime_sha256 != old.runtime_sha256,
        "new archive does not contain the expected distinct freshly built candidate executable"
    );
    let installed_path = new
        .result
        .local_artifact
        .local_path
        .as_ref()
        .context("new artifact path missing")?;
    ensure!(
        fs::read(std::path::Path::new(installed_path).join("manifest.yaml"))?
            == inputs.source_manifest,
        "packaged manifest differs from the approved actual BUSINESS candidate"
    );
    ensure!(
        new.manifest.plugin_dependencies.is_empty(),
        "standalone provider unexpectedly depends on another plugin"
    );
    ensure!(
        new.manifest
            .resource_exports
            .iter()
            .any(|export| export.resource_code == "ssh-hosts-v1"),
        "candidate lacks the approved provider export"
    );
    before.assert_preserved_by(&Snapshot::capture(f, old_id).await?)?;
    seed.assert_access(f, &old).await?;
    ensure!(
        f.authority().query(&f.root_actor, old_id).await? == old_authority,
        "new upload rewrote original contribution authority"
    );

    let unapproved = f.authority().query(&f.root_actor, new_id).await?;
    assert_permission_denied(
        f.activation.prepare(&f.member, new_id).await,
        "managed_contribution_authorization_required",
    )?;
    ensure!(
        f.authority().query(&f.root_actor, new_id).await? == unapproved,
        "new upload silently gained authority"
    );
    f.activation
        .prepare(&f.root_actor, new_id)
        .await
        .context("candidate Root activation preparation failed")?;
    f.assert_not_runtime_active(new_id).await?;
    let expected_objects: usize = new
        .manifest
        .data_models
        .iter()
        .map(|model| {
            model
                .owned_collections
                .iter()
                .map(|collection| 1 + collection.fields.len())
                .sum::<usize>()
                + model.extension_fields.len()
        })
        .sum();
    let membership: i64 = sqlx::query_scalar(
        "select count(*) from plugin_schema_version_objects where owner_id=$1 and owner_version=$2",
    )
    .bind(OWNER)
    .bind(NEW_VERSION)
    .fetch_one(&f.pool)
    .await?;
    ensure!(
        membership == expected_objects as i64,
        "new full manifest schema membership was not applied"
    );
    let approved = f.authority().query(&f.root_actor, new_id).await?;
    ensure!(
        approved
            .authorizations
            .iter()
            .any(|grant| grant.status == ContributionAuthorizationStatus::Active),
        "candidate grants missing after Root preparation"
    );
    before.assert_preserved_by(&Snapshot::capture(f, old_id).await?)?;
    seed.assert_access(f, &new).await?;
    ensure!(
        f.authority().query(&f.root_actor, old_id).await? == old_authority,
        "upgrade rewrote original grants"
    );

    // Root approval is idempotent, and a nonroot caller may reuse exact existing
    // approvals but may not recreate a revoked grant. These are real service writes.
    f.activation.prepare(&f.root_actor, new_id).await?;
    f.activation.prepare(&f.member, new_id).await?;
    ensure!(
        f.authority().query(&f.root_actor, new_id).await? == approved,
        "preparation duplicated or rewrote existing grants"
    );
    let grant = approved
        .authorizations
        .iter()
        .find(|grant| {
            grant.status == ContributionAuthorizationStatus::Active
                && grant.permission == "plugin_data.owned.write"
        })
        .context("candidate's exact owned-data grant missing")?;
    let revoked = f
        .authority()
        .revoke(
            &f.root_actor,
            new_id,
            RevokeContributionPermission {
                authorization_id: grant.id,
                expected_revision: approved.revision,
            },
        )
        .await?;
    assert_permission_denied(
        f.activation.prepare(&f.member, new_id).await,
        "managed_contribution_authorization_required",
    )?;
    ensure!(
        f.authority().query(&f.root_actor, new_id).await? == revoked,
        "revocation was not durable"
    );
    before.assert_preserved_by(&Snapshot::capture(f, old_id).await?)?;
    seed.assert_access(f, &new).await?;
    f.assert_not_runtime_active(old_id).await?;
    f.assert_not_runtime_active(new_id).await?;
    Ok(())
}
