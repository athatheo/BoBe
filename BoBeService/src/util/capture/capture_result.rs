#[derive(Debug, Clone)]
pub(crate) struct CaptureResult {
    pub(crate) image: Vec<u8>,
    pub(crate) active_window: Option<String>,
}
