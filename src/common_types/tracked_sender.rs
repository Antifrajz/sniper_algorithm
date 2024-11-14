use std::hash::{Hash, Hasher};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct TrackedSender<T> {
    pub sender: mpsc::Sender<T>,
    pub receiver_id: String,
}

impl<T> TrackedSender<T> {
    pub fn new(sender: mpsc::Sender<T>, receiver_id: String) -> Self {
        Self {
            sender,
            receiver_id,
        }
    }
}

impl<T> PartialEq for TrackedSender<T> {
    fn eq(&self, other: &Self) -> bool {
        self.receiver_id == other.receiver_id
    }
}

impl<T> Eq for TrackedSender<T> {}

impl<T> Hash for TrackedSender<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.receiver_id.hash(state);
    }
}
