//! Registrations follow real frozen graphs, including arbitrary declared managed EventStreams.
use super::*;
use control_plane::lifecycle_outbox_dispatcher::LifecycleSourceObserver;
use domain::event_center::*;

impl ManagedExtensionComposition {
    pub(crate) async fn attach_lifecycle_source_observer(
        &self,
        observer: Arc<LifecycleSourceObserver>,
    ) -> Result<()> {
        self.source_observer
            .set(observer)
            .map_err(|_| anyhow::anyhow!("lifecycle source observer already attached"))?;
        self.refresh_lifecycle_source_observations().await;
        Ok(())
    }

    pub(super) async fn refresh_lifecycle_source_observations(&self) {
        let Some(observer) = self.source_observer.get() else {
            return;
        };
        let snapshots = self.snapshots.lock().await;
        let mut sources = BTreeMap::new();
        // Retained original consumers remain real owners until backlog/fence reclamation.
        for snapshot in snapshots.retained.values().flatten() {
            if let Some(workspace) = snapshot
                .bindings
                .values()
                .next()
                .and_then(|b| Uuid::parse_str(b.handle.identity().workspace_id().as_str()).ok())
            {
                Self::collect_snapshot_sources(&mut sources, workspace, snapshot, false);
            }
        }
        for (workspace, snapshot) in &snapshots.current {
            Self::collect_snapshot_sources(&mut sources, *workspace, snapshot, true);
            if let Some(plan) = &snapshot.lifecycle_plan {
                crate::host_extensions::lifecycle::register_native_lifecycle_sources(
                    observer,
                    plan,
                    EventSourceScope::Workspace {
                        workspace_id: *workspace,
                    },
                );
            }
        }
        drop(snapshots);
        observer.retain_managed_sources(&sources.keys().cloned().collect());
        for (_, (descriptor, targets)) in sources {
            observer.register(descriptor, &targets);
        }
    }

    /// Called under the graph publication lock after its durable authority commit.
    pub(super) fn register_candidate_lifecycle_sources(
        &self,
        workspace: Uuid,
        snapshot: &ManagedWorkspaceSnapshot,
    ) {
        let Some(observer) = self.source_observer.get() else {
            return;
        };
        let mut sources = BTreeMap::new();
        Self::collect_snapshot_sources(&mut sources, workspace, snapshot, true);
        if let Some(plan) = &snapshot.lifecycle_plan {
            crate::host_extensions::lifecycle::register_native_lifecycle_sources(
                observer,
                plan,
                EventSourceScope::Workspace {
                    workspace_id: workspace,
                },
            );
        }
        for (_, (descriptor, targets)) in sources {
            observer.register(descriptor, &targets);
        }
    }

    fn collect_snapshot_sources(
        sources: &mut BTreeMap<
            (EventSourceScope, String, String),
            (
                EventSourceDescriptor,
                Vec<control_plane_contracts::ports::LifecycleSubscriberTarget>,
            ),
        >,
        workspace: Uuid,
        snapshot: &ManagedWorkspaceSnapshot,
        current: bool,
    ) {
        let scope = EventSourceScope::Workspace {
            workspace_id: workspace,
        };
        for point in snapshot.graph.points() {
            let descriptor = point.descriptor();
            if descriptor.point_kind != ExtensionPointKind::EventStream {
                continue;
            }
            // Native Create's point contract is distinct from its typed business fact.
            if descriptor.point_id.as_str() == extension_contracts::MANAGED_CREATE_EVENT_POINT {
                continue;
            }
            if descriptor.point_id.as_str() != extension_contracts::COMMUNICATION_COMMAND_POINT
                && !descriptor.is_managed_composition_event(&descriptor.owner_module_id)
            {
                continue;
            }
            let Some(schema) = descriptor.contract.payload_schema.as_ref() else {
                continue;
            };
            let targets = snapshot
                .lifecycle_plan
                .as_ref()
                .map(|p| {
                    p.subscribers()
                        .iter()
                        .filter(|s| {
                            s.fact_contract_id == descriptor.contract.contract_id.as_str()
                                && s.fact_contract_version
                                    == descriptor.contract.contract_version.as_str()
                        })
                        .map(
                            |s| control_plane_contracts::ports::LifecycleSubscriberTarget {
                                subscriber_id: s.subscriber_id.clone(),
                                handler_id: s.handler_id.clone(),
                                handler_version: s.handler_version.clone(),
                            },
                        )
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let source = EventSourceDescriptor {
                source_id: control_plane::lifecycle_outbox_dispatcher::lifecycle_source_id(&scope, descriptor.contract.contract_id.as_str(), descriptor.contract.contract_version.as_str()),
                owner_id: descriptor.owner_module_id.as_str().into(),
                point_id: descriptor.point_id.as_str().into(),
                contract_id: descriptor.contract.contract_id.as_str().into(),
                contract_version: descriptor.contract.contract_version.as_str().into(),
                name: descriptor.point_id.as_str().into(),
                description:
                    "Registered frozen managed EventStream. Scoped queue counts cover trusted metadata only; historical unbound deliveries are listed separately"
                        .into(),
                payload_schema: serde_json::json!({"type":"object","required":["payload"],"properties":{"payload":schema}}),
                origin: EventDefinitionOrigin::Plugin,
                scope: scope.clone(),
                binding: EventSourceBinding::Active,
                coverage: EventSourceCoverage::Live,
                supported_phases: if current {
                    vec![EventTriggerPhase::Published, EventTriggerPhase::Consumed]
                } else {
                    vec![EventTriggerPhase::Consumed]
                },
            };
            let key = (
                scope.clone(),
                source.contract_id.clone(),
                source.contract_version.clone(),
            );
            if let Some((existing, subscribers)) = sources.get_mut(&key) {
                if current {
                    *existing = source;
                }
                for target in targets {
                    if !subscribers
                        .iter()
                        .any(|s| s.subscriber_id == target.subscriber_id)
                    {
                        subscribers.push(target);
                    }
                }
            } else {
                sources.insert(key, (source, targets));
            }
        }
    }
}
