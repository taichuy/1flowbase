use super::*;
#[doc(hidden)]
impl CommunicationWorker {
        pub fn inactive() -> Self {
        let (stop, _) = watch::channel(false);
        Self {
            stop,
            task: tokio::sync::Mutex::new(None),
        }
    }
}
