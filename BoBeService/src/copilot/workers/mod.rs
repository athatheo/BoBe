pub(crate) mod batch;
pub(crate) mod chat;
pub(crate) mod vision;

// `ChatWorker` trait removed — its single implementation `CopilotChatWorker`
// exposes `send(...)` as an inherent method. The trait existed only to be
// `dyn`-dispatched, which never happened (`WorkerRegistry::chat()` returns
// `Arc<CopilotChatWorker>` directly), so it was paying `async_trait`'s
// `Box::pin` cost per call for zero polymorphism.
