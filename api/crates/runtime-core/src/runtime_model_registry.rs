use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::model_metadata::ModelMetadata;

#[derive(Debug, Default, Clone)]
pub struct RuntimeModelRegistry {
    models: Arc<RwLock<HashMap<String, Vec<RegisteredRuntimeModel>>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredRuntimeModel {
    pub metadata: ModelMetadata,
    pub availability: RuntimeDataModelAvailability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDataModelAvailability {
    Available,
    NotPublished,
    Disabled,
    Broken,
}

impl RuntimeDataModelAvailability {
    pub fn from_status(status: domain::DataModelStatus) -> Self {
        match status {
            domain::DataModelStatus::Published => Self::Available,
            domain::DataModelStatus::Draft => Self::NotPublished,
            domain::DataModelStatus::Disabled => Self::Disabled,
            domain::DataModelStatus::Broken => Self::Broken,
        }
    }
}

impl RuntimeModelRegistry {
    pub fn rebuild(&self, models: Vec<ModelMetadata>) {
        let models = models
            .into_iter()
            .map(|metadata| RegisteredRuntimeModel {
                availability: RuntimeDataModelAvailability::from_status(metadata.status),
                metadata,
            })
            .collect();
        self.rebuild_registered(models);
    }

    pub fn rebuild_with_status(&self, models: Vec<(ModelMetadata, domain::DataModelStatus)>) {
        let models = models
            .into_iter()
            .map(|(mut metadata, status)| {
                metadata.status = status;
                RegisteredRuntimeModel {
                    metadata,
                    availability: RuntimeDataModelAvailability::from_status(status),
                }
            })
            .collect();
        self.rebuild_registered(models);
    }

    fn rebuild_registered(&self, models: Vec<RegisteredRuntimeModel>) {
        let mut guard = self.models.write().expect("runtime registry poisoned");
        let mut grouped = HashMap::<String, Vec<RegisteredRuntimeModel>>::new();
        for model in models {
            grouped
                .entry(model.metadata.model_code.clone())
                .or_default()
                .push(model);
        }
        *guard = grouped;
    }

    pub fn upsert(&self, model: ModelMetadata) {
        let availability = RuntimeDataModelAvailability::from_status(model.status);
        self.upsert_registered(RegisteredRuntimeModel {
            metadata: model,
            availability,
        });
    }

    pub fn upsert_with_status(&self, mut model: ModelMetadata, status: domain::DataModelStatus) {
        model.status = status;
        self.upsert_registered(RegisteredRuntimeModel {
            metadata: model,
            availability: RuntimeDataModelAvailability::from_status(status),
        });
    }

    fn upsert_registered(&self, model: RegisteredRuntimeModel) {
        let mut guard = self.models.write().expect("runtime registry poisoned");
        let models = guard.entry(model.metadata.model_code.clone()).or_default();
        if let Some(existing) = models.iter_mut().find(|existing| {
            existing.metadata.scope_kind == model.metadata.scope_kind
                && existing.metadata.scope_id == model.metadata.scope_id
        }) {
            *existing = model;
        } else {
            models.push(model);
        }
    }

    pub fn remove(
        &self,
        scope_kind: domain::DataModelScopeKind,
        scope_id: uuid::Uuid,
        model_code: &str,
    ) {
        let mut guard = self.models.write().expect("runtime registry poisoned");
        if let Some(models) = guard.get_mut(model_code) {
            models.retain(|model| {
                !(model.metadata.scope_kind == scope_kind && model.metadata.scope_id == scope_id)
            });
            if models.is_empty() {
                guard.remove(model_code);
            }
        }
    }

    pub fn get(
        &self,
        scope_kind: domain::DataModelScopeKind,
        scope_id: uuid::Uuid,
        model_code: &str,
    ) -> Option<ModelMetadata> {
        self.models
            .read()
            .expect("runtime registry poisoned")
            .get(model_code)
            .and_then(|models| {
                models
                    .iter()
                    .find(|model| {
                        model.metadata.scope_kind == scope_kind
                            && model.metadata.scope_id == scope_id
                    })
                    .map(|model| model.metadata.clone())
            })
    }

    pub fn get_runtime_model(
        &self,
        scope_kind: domain::DataModelScopeKind,
        scope_id: uuid::Uuid,
        model_code: &str,
    ) -> Option<RegisteredRuntimeModel> {
        self.models
            .read()
            .expect("runtime registry poisoned")
            .get(model_code)
            .and_then(|models| {
                models
                    .iter()
                    .find(|model| {
                        model.metadata.scope_kind == scope_kind
                            && model.metadata.scope_id == scope_id
                    })
                    .cloned()
            })
    }

    pub fn list_available_for_workspace(&self, workspace_id: uuid::Uuid) -> Vec<ModelMetadata> {
        let guard = self.models.read().expect("runtime registry poisoned");
        let mut models = guard
            .values()
            .flatten()
            .filter(|model| model.availability == RuntimeDataModelAvailability::Available)
            .filter(|model| match model.metadata.scope_kind {
                domain::DataModelScopeKind::Workspace => model.metadata.scope_id == workspace_id,
                domain::DataModelScopeKind::System => true,
            })
            .map(|model| model.metadata.clone())
            .collect::<Vec<_>>();
        models.sort_by(|left, right| left.model_code.cmp(&right.model_code));
        models
    }
}
