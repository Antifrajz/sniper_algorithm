use tokio::sync::mpsc;

use crate::common_types::tracked_sender::TrackedSender;

use super::{feed_handle::FeedHandle, messages::messages::FeedUpdate};

#[derive(Clone)]
pub struct FeedService {
    feed_handle: FeedHandle,
    algo_id: String,
    meesage_sender: TrackedSender<FeedUpdate>,
}

impl FeedService {
    pub fn new<AlgoId>(
        feed_handle: &FeedHandle,
        context_id: &String,
        algo_id: AlgoId,
        sender: &mpsc::Sender<FeedUpdate>,
    ) -> Self
    where
        AlgoId: Into<String>,
    {
        Self {
            feed_handle: feed_handle.clone(),
            algo_id: algo_id.into(),
            meesage_sender: TrackedSender::new(sender.clone(), context_id.clone()),
        }
    }

    pub fn subscribe_to_l1<Symbol>(&self, base: Symbol, quote: Symbol)
    where
        Symbol: Into<String>,
    {
        self.feed_handle.subscribe_to_l1(
            self.algo_id.as_str(),
            base.into(),
            quote.into(),
            self.meesage_sender.clone(),
        );
    }

    pub fn unsubscribe_from_l1<Symbol>(&self, base: Symbol, quote: Symbol)
    where
        Symbol: Into<String>,
    {
        self.feed_handle.unsubscribe_from_l1(
            self.algo_id.as_str(),
            base.into(),
            quote.into(),
            &self.meesage_sender,
        );
    }
}
