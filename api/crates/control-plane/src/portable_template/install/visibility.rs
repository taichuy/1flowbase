use super::*;
use crate::role::{ReplaceRoleFrontstageRoutesCommand, RoleService};
use std::collections::BTreeSet;
fn role_codes(package: &PortableTemplatePackage) -> BTreeSet<String> {
    package
        .pages
        .iter()
        .flat_map(|p| p.visibility_rules.iter().map(|r| r.role_code.clone()))
        .collect()
}
impl<R: PortableTemplateInstallRepository> PortableTemplateInstallService<R> {
    pub async fn preflight_visibility(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
    ) -> Result<()> {
        let codes = role_codes(package);
        if codes.is_empty() {
            return Ok(());
        }
        let roles =
            RoleRepository::list_roles(&self.repository, actor.current_workspace_id).await?;
        for code in codes {
            anyhow::ensure!(
                code != "root",
                "portable_template_root_role_rule_unsupported"
            );
            anyhow::ensure!(
                roles.iter().any(|r| r.code == code),
                "portable_template_target_role_missing:{code}"
            );
            let rules = FrontstagePageRepository::list_frontstage_page_visibility_rules_for_role(
                &self.repository,
                actor.current_workspace_id,
                &code,
            )
            .await?;
            anyhow::ensure!(
                !rules
                    .iter()
                    .any(|r| r.visibility == domain::frontstage::FrontstagePageVisibility::Hidden),
                "portable_template_target_hidden_rule_unsupported:{code}"
            );
        }
        for page in &package.pages {
            for rule in &page.visibility_rules {
                anyhow::ensure!(
                    rule.visibility == domain::frontstage::FrontstagePageVisibility::Visible,
                    "portable_template_hidden_rule_unsupported:{}",
                    rule.role_code
                );
                anyhow::ensure!(
                    rule.tab_id
                        .is_none_or(|id| page.tabs.iter().any(|t| t.id == id)),
                    "portable_template_visibility_tab_missing"
                );
            }
        }
        Ok(())
    }
    pub(super) async fn install_visibility(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        result: &PortableTemplateInstallResult,
    ) -> Result<()> {
        let owner = RoleService::new(self.repository.clone());
        for code in role_codes(package) {
            let existing = owner.get_frontstage_routes(actor.user_id, &code).await?;
            // Merge existing grants before calling the sole policy write owner.
            anyhow::ensure!(
                !existing
                    .rules
                    .iter()
                    .any(|r| r.visibility == domain::frontstage::FrontstagePageVisibility::Hidden),
                "portable_template_target_hidden_rule_changed:{code}"
            );
            let mut pages: BTreeSet<Uuid> =
                existing.rules.iter().filter_map(|r| r.page_id).collect();
            let mut tabs: BTreeSet<Uuid> = existing.rules.iter().filter_map(|r| r.tab_id).collect();
            for page in &package.pages {
                for rule in page.visibility_rules.iter().filter(|r| r.role_code == code) {
                    match rule.tab_id {
                        Some(id) => {
                            tabs.insert(result.mapped(id)?);
                        }
                        None => {
                            pages.insert(result.mapped(page.id)?);
                        }
                    }
                }
            }
            owner
                .replace_frontstage_routes(ReplaceRoleFrontstageRoutesCommand {
                    actor_user_id: actor.user_id,
                    role_code: code,
                    page_ids: pages.into_iter().collect(),
                    tab_ids: tabs.into_iter().collect(),
                })
                .await?;
        }
        Ok(())
    }
}
