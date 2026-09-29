//! Exact captured namespace/name lookup. Roots own JSON once; leaves retain paths.
use super::classify::{bounded_id, tool_name};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

type Key = (Option<String>, String);
struct Locator {
    root: usize,
    path: Vec<usize>,
}
#[derive(Default)]
pub(super) struct SchemaIndex {
    roots: Vec<Arc<Value>>,
    entries: BTreeMap<Key, Locator>,
    ambiguous: BTreeSet<Key>,
    pub incomplete: bool,
}
impl SchemaIndex {
    pub fn insert_root(&mut self, value: &Value) {
        let root = Arc::new(value.clone());
        let index = self.roots.len();
        self.roots.push(Arc::clone(&root));
        self.index(&root, index, &mut Vec::new(), None);
    }
    fn index(
        &mut self,
        value: &Value,
        root: usize,
        path: &mut Vec<usize>,
        namespace: Option<String>,
    ) {
        if value["type"] == "namespace" {
            let Some(name) = value["name"].as_str().and_then(bounded_id) else {
                self.incomplete = true;
                return;
            };
            let Some(tools) = value["tools"].as_array() else {
                self.incomplete = true;
                return;
            };
            for (index, tool) in tools.iter().enumerate() {
                path.push(index);
                // Use the declared namespace, never synthesize a dotted path.
                self.index(tool, root, path, Some(name.clone()));
                path.pop();
            }
            return;
        }
        let Some(name) = tool_name(value) else {
            return;
        };
        let namespace = if value["namespace"].is_null() {
            namespace
        } else {
            let Some(namespace) = value["namespace"].as_str().and_then(bounded_id) else {
                self.incomplete = true;
                return;
            };
            Some(namespace)
        };
        let key = (namespace, name);
        if self.ambiguous.contains(&key) {
            return;
        }
        if self.entries.remove(&key).is_some() {
            self.ambiguous.insert(key);
            self.incomplete = true;
            return;
        }
        self.entries.insert(
            key,
            Locator {
                root,
                path: path.clone(),
            },
        );
    }
    pub fn get(&self, namespace: Option<&str>, name: &str) -> Option<&Value> {
        let location = self
            .entries
            .get(&(namespace.map(str::to_owned), name.to_owned()))?;
        let mut value = self.roots[location.root].as_ref();
        for index in &location.path {
            value = value.get("tools")?.get(*index)?;
        }
        Some(value)
    }
}
