use super::{l1_data::L1Data, l2_data::L2Data};
use crate::common_types::tracked_sender::TrackedSender;
type AlgoId = String;

#[derive(Debug, Clone)]
pub enum FeedUpdate {
    L1Update(Vec<AlgoId>, L1Data),
    L2Update(Vec<AlgoId>, L2Data),
}
pub(crate) enum FeedMessages {
    SubscribeToL1 {
        algo_id: String,
        base: String,
        quote: String,
        subscriber: TrackedSender<FeedUpdate>,
    },
    UnsubscribeFromL1 {
        algo_id: String,
        base: String,
        quote: String,
        subscriber: TrackedSender<FeedUpdate>,
    },
    SubscribeToL2 {
        algo_id: String,
        base: String,
        quote: String,
        subscriber: TrackedSender<FeedUpdate>,
    },
    UnsubscribeFromL2 {
        algo_id: String,
        base: String,
        quote: String,
        subscriber: TrackedSender<FeedUpdate>,
    },
}
