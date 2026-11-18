use std::{collections::HashMap, sync::Mutex};
use tokio::sync::watch;
use uuid::Uuid;
type Key = (Uuid, Uuid, Uuid);
#[derive(Default)]
pub struct ProviderActivity {
    pub entries: Mutex<HashMap<Key, watch::Sender<usize>>>,
}
