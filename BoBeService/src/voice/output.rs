use tokio::sync::mpsc;

use crate::speech::protocol::ServerMessage;

#[derive(Debug, Clone)]
pub(crate) enum VoiceOutputFrame {
    Control(ServerMessage),
    Audio(Vec<u8>),
    Ping,
}

#[derive(Clone)]
pub(crate) struct VoiceOutput {
    sender: mpsc::Sender<VoiceOutputFrame>,
}

impl VoiceOutput {
    pub(crate) fn channel(capacity: usize) -> (Self, mpsc::Receiver<VoiceOutputFrame>) {
        let (sender, receiver) = mpsc::channel(capacity);
        (Self { sender }, receiver)
    }

    pub(crate) async fn send(&self, frame: VoiceOutputFrame) -> bool {
        self.sender.send(frame).await.is_ok()
    }

    pub(crate) async fn control(&self, message: ServerMessage) -> bool {
        self.send(VoiceOutputFrame::Control(message)).await
    }

    pub(crate) async fn audio(&self, bytes: Vec<u8>) -> bool {
        self.send(VoiceOutputFrame::Audio(bytes)).await
    }

    pub(crate) async fn ping(&self) -> bool {
        self.send(VoiceOutputFrame::Ping).await
    }
}
