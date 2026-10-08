use std::collections::VecDeque;

use crate::component::ComponentId;
use crate::tracing::TraceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRequest {
    pub requester: ComponentId,
    pub amount: u64,
    pub cause: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedRequest {
    pub requester: ComponentId,
    pub amount: u64,
    pub cause: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceRequestResult {
    Granted,
    Queued { queue_position: usize },
    Invalid { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub id: ResourceId,
    pub name: String,
    pub capacity: u64,
    pub available: u64,
    pub pending_queue: VecDeque<PendingRequest>,
}

impl Resource {
    pub fn new<S: Into<String>>(id: ResourceId, name: S, capacity: u64) -> Self {
        Self {
            id,
            name: name.into(),
            capacity,
            available: capacity,
            pending_queue: VecDeque::new(),
        }
    }

    pub fn request(
        &mut self,
        requester: ComponentId,
        amount: u64,
        cause: Option<TraceId>,
    ) -> ResourceRequestResult {
        if amount > self.capacity {
            return ResourceRequestResult::Invalid {
                reason: format!(
                    "Requested amount {} exceeds total resource capacity {}",
                    amount, self.capacity
                ),
            };
        }

        if self.pending_queue.is_empty() && self.available >= amount {
            self.available -= amount;
            ResourceRequestResult::Granted
        } else {
            self.pending_queue.push_back(PendingRequest {
                requester,
                amount,
                cause,
            });
            ResourceRequestResult::Queued {
                queue_position: self.pending_queue.len(),
            }
        }
    }

    pub fn release(&mut self, amount: u64) -> Vec<GrantedRequest> {
        self.available = (self.available + amount).min(self.capacity);
        let mut granted = Vec::new();

        while let Some(front) = self.pending_queue.front() {
            if self.available >= front.amount {
                let req = self.pending_queue.pop_front().unwrap();
                self.available -= req.amount;
                granted.push(GrantedRequest {
                    requester: req.requester,
                    amount: req.amount,
                    cause: req.cause,
                });
            } else {
                break;
            }
        }

        granted
    }
}
