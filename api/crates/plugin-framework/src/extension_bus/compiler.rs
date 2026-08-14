use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    Cardinality, ContributionDescriptor, ContributionId, ContributionInactivityReason,
    ContributionMode, ContributionResolutionReceipt, ContributionResolutionStatus,
    DeliverySemantics, EffectiveContribution, EffectiveExtensionGraph, EffectiveExtensionPoint,
    ExtensionBusVersion, ExtensionGraphFingerprint, ExtensionPointDescriptor, ExtensionPointId,
    ExtensionPointKind, ModuleActivationDeclaration, ModuleDescriptor, ModuleId,
    ModuleInactivityReason, ModuleKind, ModuleResolutionReceipt, ModuleResolutionStatus,
    OrderingSemantics, OverridePolicy, PermissionCode, Provenance,
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CompilationError {
    #[error("duplicate module descriptor for {module_id:?}")]
    DuplicateModule { module_id: ModuleId },
    #[error("module {module_id:?} depends on missing module {dependency_id:?}")]
    MissingModuleDependency {
        module_id: ModuleId,
        dependency_id: ModuleId,
    },
    #[error("module {module_id:?} requires version {required_version:?} of {dependency_id:?}, found {actual_version:?}")]
    IncompatibleModuleDependencyVersion {
        module_id: ModuleId,
        dependency_id: ModuleId,
        required_version: super::ModuleVersion,
        actual_version: super::ModuleVersion,
    },
    #[error("dependency cycle contains {nodes:?}")]
    DependencyCycle { nodes: Vec<String> },
    #[error("module {module_id:?} cannot define extension points")]
    UnauthorizedPointDefinition {
        module_id: ModuleId,
        module_kind: ModuleKind,
    },
    #[error("point {point_id:?} declares owner {declared_owner:?}, expected {module_id:?}")]
    PointOwnerMismatch {
        point_id: ExtensionPointId,
        declared_owner: ModuleId,
        module_id: ModuleId,
    },
    #[error("point {point_id:?} is owned by both {existing_owner:?} and {incoming_owner:?}")]
    DuplicatePointOwner {
        point_id: ExtensionPointId,
        existing_owner: ModuleId,
        incoming_owner: ModuleId,
    },
    #[error("point {point_id:?} kind {point_kind:?} is incompatible with delivery {delivery:?}")]
    IncompatibleDeliverySemantics {
        point_id: ExtensionPointId,
        point_kind: ExtensionPointKind,
        delivery: DeliverySemantics,
    },
    #[error("contribution {contribution_id:?} declares contributor {declared_contributor:?}, expected {module_id:?}")]
    ContributorMismatch {
        contribution_id: ContributionId,
        declared_contributor: ModuleId,
        module_id: ModuleId,
    },
    #[error("duplicate contribution id {contribution_id:?}")]
    DuplicateContribution { contribution_id: ContributionId },
    #[error("contribution {contribution_id:?} targets missing point {point_id:?}")]
    MissingExtensionPoint {
        contribution_id: ContributionId,
        point_id: ExtensionPointId,
    },
    #[error(
        "contribution {contribution_id:?} uses contract version {actual:?}, expected {expected:?}"
    )]
    ContractVersionMismatch {
        contribution_id: ContributionId,
        expected: super::ContractVersion,
        actual: super::ContractVersion,
    },
    #[error("module {module_id:?} cannot override point {point_id:?}")]
    IllegalOverride {
        module_id: ModuleId,
        point_id: ExtensionPointId,
    },
    #[error("multiple trusted overrides target point {point_id:?}")]
    ConflictingOverrides { point_id: ExtensionPointId },
    #[error("contribution {contribution_id:?} escalates permission {permission:?}")]
    PermissionEscalation {
        contribution_id: ContributionId,
        permission: PermissionCode,
    },
    #[error(
        "contribution {contribution_id:?} orders against missing contribution {dependency_id:?}"
    )]
    MissingContributionDependency {
        contribution_id: ContributionId,
        dependency_id: ContributionId,
    },
    #[error("point {point_id:?} has {actual} effective contributions, incompatible with {cardinality:?}")]
    CardinalityConflict {
        point_id: ExtensionPointId,
        cardinality: Cardinality,
        actual: usize,
    },
}

struct PointDeclaration {
    descriptor: ExtensionPointDescriptor,
    provenance: Provenance,
}

#[derive(Clone)]
struct ContributionDeclaration {
    descriptor: ContributionDescriptor,
    provenance: Provenance,
}

pub fn compile_extension_graph(
    modules: Vec<ModuleDescriptor>,
) -> Result<EffectiveExtensionGraph, CompilationError> {
    let modules = index_modules(modules)?;
    let module_order = compile_module_order(&modules)?;
    let module_provenances = module_order
        .iter()
        .map(|module_id| module_provenance(&modules[module_id]))
        .collect::<Vec<_>>();
    let module_statuses = resolve_module_statuses(&modules, &module_order);
    let module_receipts = module_order
        .iter()
        .map(|module_id| {
            ModuleResolutionReceipt::new(
                module_provenance(&modules[module_id]),
                module_statuses[module_id].clone(),
            )
        })
        .collect::<Vec<_>>();
    let points = index_points(&modules)?;
    let contributions = index_contributions(&modules, &points)?;
    let (effective_points, contribution_receipts) =
        compile_points(points, contributions, &module_statuses)?;
    let fingerprint = fingerprint(
        ExtensionBusVersion::V1,
        &module_order,
        &module_provenances,
        &module_receipts,
        &effective_points,
        &contribution_receipts,
    );

    Ok(EffectiveExtensionGraph::new(
        ExtensionBusVersion::V1,
        module_order,
        module_provenances,
        module_receipts,
        effective_points,
        contribution_receipts,
        fingerprint,
    ))
}

fn index_modules(
    modules: Vec<ModuleDescriptor>,
) -> Result<BTreeMap<ModuleId, ModuleDescriptor>, CompilationError> {
    let mut indexed = BTreeMap::new();
    for module in modules {
        let module_id = module.module_id.clone();
        if indexed.insert(module_id.clone(), module).is_some() {
            return Err(CompilationError::DuplicateModule { module_id });
        }
    }
    Ok(indexed)
}

fn compile_module_order(
    modules: &BTreeMap<ModuleId, ModuleDescriptor>,
) -> Result<Vec<ModuleId>, CompilationError> {
    let mut incoming = modules
        .keys()
        .cloned()
        .map(|module_id| (module_id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<ModuleId, BTreeSet<ModuleId>>::new();

    for module in modules.values() {
        for dependency in &module.dependencies {
            let dependency_module = modules.get(&dependency.module_id).ok_or_else(|| {
                CompilationError::MissingModuleDependency {
                    module_id: module.module_id.clone(),
                    dependency_id: dependency.module_id.clone(),
                }
            })?;
            if dependency_module.module_version != dependency.required_version {
                return Err(CompilationError::IncompatibleModuleDependencyVersion {
                    module_id: module.module_id.clone(),
                    dependency_id: dependency.module_id.clone(),
                    required_version: dependency.required_version.clone(),
                    actual_version: dependency_module.module_version.clone(),
                });
            }
            *incoming
                .get_mut(&module.module_id)
                .expect("module is indexed") += 1;
            dependents
                .entry(dependency.module_id.clone())
                .or_default()
                .insert(module.module_id.clone());
        }
    }

    topological_module_order(incoming, dependents)
}

fn topological_module_order(
    mut incoming: BTreeMap<ModuleId, usize>,
    dependents: BTreeMap<ModuleId, BTreeSet<ModuleId>>,
) -> Result<Vec<ModuleId>, CompilationError> {
    let mut ready = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(module_id, _)| module_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(incoming.len());

    while let Some(module_id) = ready.pop_first() {
        ordered.push(module_id.clone());
        if let Some(next_modules) = dependents.get(&module_id) {
            for next_module in next_modules {
                let count = incoming.get_mut(next_module).expect("dependent is indexed");
                *count -= 1;
                if *count == 0 {
                    ready.insert(next_module.clone());
                }
            }
        }
    }

    if ordered.len() != incoming.len() {
        let nodes = incoming
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(module_id, _)| module_id.as_str().to_string())
            .collect();
        return Err(CompilationError::DependencyCycle { nodes });
    }
    Ok(ordered)
}

fn resolve_module_statuses(
    modules: &BTreeMap<ModuleId, ModuleDescriptor>,
    module_order: &[ModuleId],
) -> BTreeMap<ModuleId, ModuleResolutionStatus> {
    let mut statuses = BTreeMap::new();
    for module_id in module_order {
        let module = &modules[module_id];
        let status = match &module.activation {
            ModuleActivationDeclaration::Disabled { reason } => ModuleResolutionStatus::Inactive {
                reason: ModuleInactivityReason::Disabled { reason: *reason },
            },
            ModuleActivationDeclaration::Active => module
                .dependencies
                .iter()
                .find(|dependency| {
                    matches!(
                        statuses.get(&dependency.module_id),
                        Some(ModuleResolutionStatus::Inactive { .. })
                    )
                })
                .map_or(ModuleResolutionStatus::Active, |dependency| {
                    ModuleResolutionStatus::Inactive {
                        reason: ModuleInactivityReason::DependencyInactive {
                            dependency_module_id: dependency.module_id.clone(),
                        },
                    }
                }),
        };
        statuses.insert(module_id.clone(), status);
    }
    statuses
}

fn index_points(
    modules: &BTreeMap<ModuleId, ModuleDescriptor>,
) -> Result<BTreeMap<ExtensionPointId, PointDeclaration>, CompilationError> {
    let mut points = BTreeMap::new();
    for module in modules.values() {
        if !module.extension_points.is_empty() && !module.module_kind.may_define_points() {
            return Err(CompilationError::UnauthorizedPointDefinition {
                module_id: module.module_id.clone(),
                module_kind: module.module_kind,
            });
        }
        let provenance = module_provenance(module);
        for point in &module.extension_points {
            if point.owner_module_id != module.module_id {
                return Err(CompilationError::PointOwnerMismatch {
                    point_id: point.point_id.clone(),
                    declared_owner: point.owner_module_id.clone(),
                    module_id: module.module_id.clone(),
                });
            }
            validate_delivery_semantics(point)?;
            let declaration = PointDeclaration {
                descriptor: point.clone(),
                provenance: provenance.clone(),
            };
            if let Some(existing) = points.insert(point.point_id.clone(), declaration) {
                return Err(CompilationError::DuplicatePointOwner {
                    point_id: point.point_id.clone(),
                    existing_owner: existing.descriptor.owner_module_id,
                    incoming_owner: point.owner_module_id.clone(),
                });
            }
        }
    }
    Ok(points)
}

fn validate_delivery_semantics(point: &ExtensionPointDescriptor) -> Result<(), CompilationError> {
    let compatible = match point.point_kind {
        ExtensionPointKind::EventStream => matches!(
            point.delivery,
            DeliverySemantics::AfterCommitDurable
                | DeliverySemantics::RequiredStream
                | DeliverySemantics::DiagnosticBestEffort
        ),
        _ => matches!(
            point.delivery,
            DeliverySemantics::Synchronous | DeliverySemantics::Asynchronous
        ),
    };
    if !compatible {
        return Err(CompilationError::IncompatibleDeliverySemantics {
            point_id: point.point_id.clone(),
            point_kind: point.point_kind,
            delivery: point.delivery,
        });
    }
    Ok(())
}

fn index_contributions(
    modules: &BTreeMap<ModuleId, ModuleDescriptor>,
    points: &BTreeMap<ExtensionPointId, PointDeclaration>,
) -> Result<BTreeMap<ExtensionPointId, Vec<ContributionDeclaration>>, CompilationError> {
    let mut contribution_ids = BTreeSet::new();
    let mut indexed = BTreeMap::<ExtensionPointId, Vec<ContributionDeclaration>>::new();

    for module in modules.values() {
        for contribution in &module.contributions {
            if contribution.contributor_module_id != module.module_id {
                return Err(CompilationError::ContributorMismatch {
                    contribution_id: contribution.contribution_id.clone(),
                    declared_contributor: contribution.contributor_module_id.clone(),
                    module_id: module.module_id.clone(),
                });
            }
            if !contribution_ids.insert(contribution.contribution_id.clone()) {
                return Err(CompilationError::DuplicateContribution {
                    contribution_id: contribution.contribution_id.clone(),
                });
            }
            let point = points.get(&contribution.point_id).ok_or_else(|| {
                CompilationError::MissingExtensionPoint {
                    contribution_id: contribution.contribution_id.clone(),
                    point_id: contribution.point_id.clone(),
                }
            })?;
            if contribution.contract_version != point.descriptor.contract.contract_version {
                return Err(CompilationError::ContractVersionMismatch {
                    contribution_id: contribution.contribution_id.clone(),
                    expected: point.descriptor.contract.contract_version.clone(),
                    actual: contribution.contract_version.clone(),
                });
            }
            validate_override(module, &point.descriptor, contribution)?;
            validate_permissions(module, &point.descriptor, contribution)?;

            indexed
                .entry(contribution.point_id.clone())
                .or_default()
                .push(ContributionDeclaration {
                    descriptor: contribution.clone(),
                    provenance: module_provenance(module),
                });
        }
    }
    Ok(indexed)
}

fn validate_override(
    module: &ModuleDescriptor,
    point: &ExtensionPointDescriptor,
    contribution: &ContributionDescriptor,
) -> Result<(), CompilationError> {
    if contribution.mode == ContributionMode::Override
        && (!module.module_kind.may_override()
            || point.override_policy != OverridePolicy::TrustedHost)
    {
        return Err(CompilationError::IllegalOverride {
            module_id: module.module_id.clone(),
            point_id: point.point_id.clone(),
        });
    }
    Ok(())
}

fn validate_permissions(
    module: &ModuleDescriptor,
    point: &ExtensionPointDescriptor,
    contribution: &ContributionDescriptor,
) -> Result<(), CompilationError> {
    for permission in &contribution.required_permissions {
        if !module.granted_permissions.contains(permission)
            || !point.allowed_permissions.contains(permission)
        {
            return Err(CompilationError::PermissionEscalation {
                contribution_id: contribution.contribution_id.clone(),
                permission: permission.clone(),
            });
        }
    }
    Ok(())
}

fn compile_points(
    points: BTreeMap<ExtensionPointId, PointDeclaration>,
    mut contributions: BTreeMap<ExtensionPointId, Vec<ContributionDeclaration>>,
    module_statuses: &BTreeMap<ModuleId, ModuleResolutionStatus>,
) -> Result<
    (
        Vec<EffectiveExtensionPoint>,
        Vec<ContributionResolutionReceipt>,
    ),
    CompilationError,
> {
    let mut effective_points = Vec::with_capacity(points.len());
    let mut receipts = Vec::new();
    for (point_id, point) in points {
        let declarations = contributions.remove(&point_id).unwrap_or_default();
        validate_candidate_ordering(&point.descriptor, &declarations)?;

        let mut eligible = Vec::new();
        for declaration in declarations {
            match &module_statuses[&declaration.descriptor.contributor_module_id] {
                ModuleResolutionStatus::Active => eligible.push(declaration),
                ModuleResolutionStatus::Inactive { reason } => receipts.push(contribution_receipt(
                    declaration,
                    ContributionResolutionStatus::Inactive {
                        reason: ContributionInactivityReason::ModuleInactive {
                            reason: reason.clone(),
                        },
                    },
                )),
            }
        }

        if let ModuleResolutionStatus::Inactive { reason } =
            &module_statuses[&point.descriptor.owner_module_id]
        {
            receipts.extend(eligible.into_iter().map(|declaration| {
                contribution_receipt(
                    declaration,
                    ContributionResolutionStatus::Inactive {
                        reason: ContributionInactivityReason::PointOwnerInactive {
                            owner_module_id: point.descriptor.owner_module_id.clone(),
                            reason: reason.clone(),
                        },
                    },
                )
            }));
            continue;
        }

        let (selected, superseded_receipts) = select_overrides(&point_id, eligible)?;
        receipts.extend(superseded_receipts);
        if !point.descriptor.cardinality.accepts(selected.len()) {
            return Err(CompilationError::CardinalityConflict {
                point_id,
                cardinality: point.descriptor.cardinality,
                actual: selected.len(),
            });
        }
        let ordered = order_contributions(&point.descriptor, selected)?;
        let mut effective_contributions = Vec::with_capacity(ordered.len());
        for declaration in ordered {
            receipts.push(contribution_receipt(
                declaration.clone(),
                ContributionResolutionStatus::Active,
            ));
            effective_contributions.push(EffectiveContribution::new(
                declaration.descriptor,
                declaration.provenance,
            ));
        }
        effective_points.push(EffectiveExtensionPoint::new(
            point.descriptor,
            point.provenance,
            effective_contributions,
        ));
    }
    receipts.sort_by(|left, right| {
        left.descriptor()
            .contribution_id
            .cmp(&right.descriptor().contribution_id)
    });
    Ok((effective_points, receipts))
}

fn select_overrides(
    point_id: &ExtensionPointId,
    declarations: Vec<ContributionDeclaration>,
) -> Result<
    (
        Vec<ContributionDeclaration>,
        Vec<ContributionResolutionReceipt>,
    ),
    CompilationError,
> {
    let override_count = declarations
        .iter()
        .filter(|entry| entry.descriptor.mode == ContributionMode::Override)
        .count();
    if override_count > 1 {
        return Err(CompilationError::ConflictingOverrides {
            point_id: point_id.clone(),
        });
    }
    if override_count == 1 {
        let winner_id = declarations
            .iter()
            .find(|entry| entry.descriptor.mode == ContributionMode::Override)
            .expect("one override was counted")
            .descriptor
            .contribution_id
            .clone();
        let mut selected = Vec::with_capacity(1);
        let mut receipts = Vec::new();
        for declaration in declarations {
            if declaration.descriptor.mode == ContributionMode::Override {
                selected.push(declaration);
            } else {
                receipts.push(contribution_receipt(
                    declaration,
                    ContributionResolutionStatus::SupersededBy {
                        contribution_id: winner_id.clone(),
                    },
                ));
            }
        }
        return Ok((selected, receipts));
    }
    Ok((declarations, Vec::new()))
}

fn order_contributions(
    point: &ExtensionPointDescriptor,
    declarations: Vec<ContributionDeclaration>,
) -> Result<Vec<ContributionDeclaration>, CompilationError> {
    let mut indexed = declarations
        .into_iter()
        .map(|entry| (entry.descriptor.contribution_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();

    let order = match point.ordering {
        OrderingSemantics::Lexicographic => indexed.keys().cloned().collect(),
        OrderingSemantics::Dependency => contribution_dependency_order(&indexed, false)?,
    };

    Ok(order
        .into_iter()
        .map(|contribution_id| {
            indexed
                .remove(&contribution_id)
                .expect("compiled contribution is indexed")
        })
        .collect())
}

fn validate_candidate_ordering(
    point: &ExtensionPointDescriptor,
    declarations: &[ContributionDeclaration],
) -> Result<(), CompilationError> {
    if point.ordering == OrderingSemantics::Dependency {
        let indexed = declarations
            .iter()
            .cloned()
            .map(|entry| (entry.descriptor.contribution_id.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        contribution_dependency_order(&indexed, true)?;
    }
    Ok(())
}

fn contribution_receipt(
    declaration: ContributionDeclaration,
    status: ContributionResolutionStatus,
) -> ContributionResolutionReceipt {
    ContributionResolutionReceipt::new(declaration.descriptor, declaration.provenance, status)
}

fn contribution_dependency_order(
    contributions: &BTreeMap<ContributionId, ContributionDeclaration>,
    reject_missing: bool,
) -> Result<Vec<ContributionId>, CompilationError> {
    let mut incoming = contributions
        .keys()
        .cloned()
        .map(|id| (id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<ContributionId, BTreeSet<ContributionId>>::new();

    for (id, declaration) in contributions {
        for dependency in &declaration.descriptor.ordering.after {
            add_contribution_edge(
                dependency,
                id,
                id,
                contributions,
                &mut incoming,
                &mut dependents,
                reject_missing,
            )?;
        }
        for successor in &declaration.descriptor.ordering.before {
            add_contribution_edge(
                id,
                successor,
                id,
                contributions,
                &mut incoming,
                &mut dependents,
                reject_missing,
            )?;
        }
    }

    let mut ready = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(incoming.len());
    while let Some(id) = ready.pop_first() {
        ordered.push(id.clone());
        if let Some(next_ids) = dependents.get(&id) {
            for next_id in next_ids {
                let count = incoming.get_mut(next_id).expect("dependent is indexed");
                *count -= 1;
                if *count == 0 {
                    ready.insert(next_id.clone());
                }
            }
        }
    }
    if ordered.len() != incoming.len() {
        let nodes = incoming
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(id, _)| id.as_str().to_string())
            .collect();
        return Err(CompilationError::DependencyCycle { nodes });
    }
    Ok(ordered)
}

fn add_contribution_edge(
    predecessor: &ContributionId,
    successor: &ContributionId,
    declared_by: &ContributionId,
    contributions: &BTreeMap<ContributionId, ContributionDeclaration>,
    incoming: &mut BTreeMap<ContributionId, usize>,
    dependents: &mut BTreeMap<ContributionId, BTreeSet<ContributionId>>,
    reject_missing: bool,
) -> Result<(), CompilationError> {
    let dependency_id = if !contributions.contains_key(predecessor) {
        predecessor
    } else if !contributions.contains_key(successor) {
        successor
    } else {
        if dependents
            .entry(predecessor.clone())
            .or_default()
            .insert(successor.clone())
        {
            *incoming.get_mut(successor).expect("successor is indexed") += 1;
        }
        return Ok(());
    };
    if reject_missing {
        Err(CompilationError::MissingContributionDependency {
            contribution_id: declared_by.clone(),
            dependency_id: dependency_id.clone(),
        })
    } else {
        Ok(())
    }
}

fn module_provenance(module: &ModuleDescriptor) -> Provenance {
    Provenance::new(
        module.module_id.clone(),
        module.module_version.clone(),
        module.module_kind,
    )
}

#[derive(Serialize)]
struct FingerprintMaterial<'a> {
    bus_version: ExtensionBusVersion,
    module_order: &'a [ModuleId],
    module_provenance: &'a [Provenance],
    module_receipts: &'a [ModuleResolutionReceipt],
    points: &'a [EffectiveExtensionPoint],
    contribution_receipts: &'a [ContributionResolutionReceipt],
}

fn fingerprint(
    bus_version: ExtensionBusVersion,
    module_order: &[ModuleId],
    module_provenance: &[Provenance],
    module_receipts: &[ModuleResolutionReceipt],
    points: &[EffectiveExtensionPoint],
    contribution_receipts: &[ContributionResolutionReceipt],
) -> ExtensionGraphFingerprint {
    let material = FingerprintMaterial {
        bus_version,
        module_order,
        module_provenance,
        module_receipts,
        points,
        contribution_receipts,
    };
    let canonical = serde_json::to_vec(&material)
        .expect("typed Extension Bus fingerprint material is always serializable");
    let digest = Sha256::digest(canonical);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing a String cannot fail");
    }
    ExtensionGraphFingerprint::new(encoded)
}
