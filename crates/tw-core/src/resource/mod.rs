use std::collections::VecDeque;
use std::fmt;

use crate::component::ComponentId;
use crate::tracing::TraceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(pub u64);

/// A quantity that can be allocated from a resource.
///
/// Resources deliberately do not lock themselves to `u64`: discrete resources
/// (machines, connections, slots) use [`Amount::Discrete`], while continuous
/// resources (bandwidth, power, memory) use [`Amount::Continuous`]. The two
/// kinds never mix call by call; a request whose kind does not match the
/// resource capacity is rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Amount {
    Discrete(u64),
    Continuous(f64),
}

impl Amount {
    pub const ZERO: Amount = Amount::Discrete(0);

    /// The kind name, used in diagnostics.
    pub fn kind(&self) -> &'static str {
        match self {
            Amount::Discrete(_) => "discrete",
            Amount::Continuous(_) => "continuous",
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Amount::Discrete(v) => Some(*v),
            Amount::Continuous(_) => None,
        }
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            Amount::Discrete(v) => *v as f64,
            Amount::Continuous(v) => *v,
        }
    }

    /// `self - other`, or `None` if the result would be negative, would mix
    /// kinds, or would not fit the underlying representation.
    pub fn checked_sub(&self, other: &Amount) -> Option<Amount> {
        match (self, other) {
            (Amount::Discrete(a), Amount::Discrete(b)) => a.checked_sub(*b).map(Amount::Discrete),
            (Amount::Continuous(a), Amount::Continuous(b)) => {
                let result = a - b;
                if result >= 0.0 {
                    Some(Amount::Continuous(result))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// `self + other`, or `None` when the kinds differ.
    pub fn checked_add(&self, other: &Amount) -> Option<Amount> {
        match (self, other) {
            (Amount::Discrete(a), Amount::Discrete(b)) => a.checked_add(*b).map(Amount::Discrete),
            (Amount::Continuous(a), Amount::Continuous(b)) => Some(Amount::Continuous(a + b)),
            _ => None,
        }
    }

    /// The smaller of `self` and `other`, keeping `self` if kinds differ.
    pub fn min_with(&self, other: &Amount) -> Amount {
        match (self, other) {
            (Amount::Discrete(a), Amount::Discrete(b)) => Amount::Discrete((*a).min(*b)),
            (Amount::Continuous(a), Amount::Continuous(b)) => Amount::Continuous(a.min(*b)),
            _ => *self,
        }
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Amount::Discrete(v) => write!(f, "{v}"),
            Amount::Continuous(v) => write!(f, "{v}"),
        }
    }
}

macro_rules! impl_from_discrete {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for Amount {
                fn from(v: $t) -> Self {
                    Amount::Discrete(v as u64)
                }
            }
        )*
    };
}

impl_from_discrete!(u8, u16, u32, u64, usize);

impl From<i32> for Amount {
    fn from(v: i32) -> Self {
        Amount::Discrete(v.max(0) as u64)
    }
}

impl From<i64> for Amount {
    fn from(v: i64) -> Self {
        Amount::Discrete(v.max(0) as u64)
    }
}

impl From<f32> for Amount {
    fn from(v: f32) -> Self {
        Amount::Continuous(v as f64)
    }
}

impl From<f64> for Amount {
    fn from(v: f64) -> Self {
        Amount::Continuous(v)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingRequest {
    pub requester: ComponentId,
    pub amount: Amount,
    /// Higher values are served first by [`PriorityPolicy`].
    pub priority: i64,
    pub cause: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrantedRequest {
    pub requester: ComponentId,
    pub amount: Amount,
    pub cause: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourceError {
    /// The request's amount kind does not match the resource capacity kind.
    #[error("requested amount is {actual} but resource capacity is {expected}")]
    KindMismatch {
        actual: &'static str,
        expected: &'static str,
    },
    /// The request asks for more than the resource can ever provide.
    #[error("requested amount {requested} exceeds total resource capacity {capacity}")]
    ExceedsCapacity { requested: String, capacity: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceRequestResult {
    Granted,
    Queued { queue_position: usize },
    Invalid { reason: ResourceError },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceSnapshot {
    pub id: ResourceId,
    pub capacity: Amount,
    pub available: Amount,
    pub pending_queue: VecDeque<PendingRequest>,
}

/// Decides which queued requests to grant when capacity becomes available.
///
/// The policy is intentionally pluggable: the resource mechanism provides the
/// primitive (queue + release → grants), while the model chooses the policy.
/// This keeps fairness/priority rules *out of* the engine.
pub trait AllocationPolicy: fmt::Debug {
    /// Given the currently `available` amount and the `queue` in arrival order,
    /// return the indices of the queued requests to grant. Each selected request
    /// is removed from the queue.
    fn select(&self, available: &Amount, queue: &VecDeque<PendingRequest>) -> Vec<usize>;
}

/// Strict first-in-first-out. Stops at the first request that cannot fit, so an
/// earlier request can never be overtaken by a later one.
#[derive(Debug, Default, Clone, Copy)]
pub struct FifoPolicy;

impl AllocationPolicy for FifoPolicy {
    fn select(&self, available: &Amount, queue: &VecDeque<PendingRequest>) -> Vec<usize> {
        let mut remaining = *available;
        let mut selected = Vec::new();
        for (index, request) in queue.iter().enumerate() {
            match remaining.checked_sub(&request.amount) {
                Some(rest) => {
                    remaining = rest;
                    selected.push(index);
                }
                None => break,
            }
        }
        selected
    }
}

/// Serves queued requests by descending priority (ties broken by arrival order).
/// A request that cannot fit does not block lower-priority requests behind it.
#[derive(Debug, Default, Clone, Copy)]
pub struct PriorityPolicy;

impl AllocationPolicy for PriorityPolicy {
    fn select(&self, available: &Amount, queue: &VecDeque<PendingRequest>) -> Vec<usize> {
        let mut order: Vec<usize> = (0..queue.len()).collect();
        order.sort_by(|&a, &b| queue[b].priority.cmp(&queue[a].priority).then_with(|| a.cmp(&b)));

        let mut remaining = *available;
        let mut selected = Vec::new();
        for index in order {
            if let Some(rest) = remaining.checked_sub(&queue[index].amount) {
                remaining = rest;
                selected.push(index);
            }
        }
        selected
    }
}

#[derive(Debug)]
pub struct Resource {
    pub id: ResourceId,
    pub name: String,
    pub capacity: Amount,
    pub available: Amount,
    pub pending_queue: VecDeque<PendingRequest>,
    /// How long a granted allocation holds capacity before the engine releases it
    /// automatically (in ticks). `0` means the holder releases manually.
    pub service_time: u64,
    policy: Box<dyn AllocationPolicy>,
}

impl Resource {
    pub fn new<S: Into<String>>(id: ResourceId, name: S, capacity: impl Into<Amount>) -> Self {
        let capacity = capacity.into();
        Self {
            id,
            name: name.into(),
            capacity,
            available: capacity,
            pending_queue: VecDeque::new(),
            service_time: 0,
            policy: Box::new(FifoPolicy),
        }
    }

    /// Give the resource a service time (latency): granted allocations are
    /// released automatically by the engine after this many ticks.
    pub fn with_service_time(mut self, service_time: u64) -> Self {
        self.service_time = service_time;
        self
    }

    /// Install a custom allocation policy.
    pub fn with_policy(mut self, policy: impl AllocationPolicy + 'static) -> Self {
        self.policy = Box::new(policy);
        self
    }

    pub fn request(
        &mut self,
        requester: ComponentId,
        amount: impl Into<Amount>,
        cause: Option<TraceId>,
    ) -> ResourceRequestResult {
        self.request_with_priority(requester, amount, 0, cause)
    }

    pub fn request_with_priority(
        &mut self,
        requester: ComponentId,
        amount: impl Into<Amount>,
        priority: i64,
        cause: Option<TraceId>,
    ) -> ResourceRequestResult {
        let amount = amount.into();

        if amount.kind() != self.capacity.kind() {
            return ResourceRequestResult::Invalid {
                reason: ResourceError::KindMismatch {
                    actual: amount.kind(),
                    expected: self.capacity.kind(),
                },
            };
        }

        if self.capacity.checked_sub(&amount).is_none() {
            return ResourceRequestResult::Invalid {
                reason: ResourceError::ExceedsCapacity {
                    requested: amount.to_string(),
                    capacity: self.capacity.to_string(),
                },
            };
        }

        if self.pending_queue.is_empty() && self.available.checked_sub(&amount).is_some() {
            self.available = self
                .available
                .checked_sub(&amount)
                .expect("checked immediately above");
            ResourceRequestResult::Granted
        } else {
            self.pending_queue.push_back(PendingRequest {
                requester,
                amount,
                priority,
                cause,
            });
            ResourceRequestResult::Queued {
                queue_position: self.pending_queue.len(),
            }
        }
    }

    pub fn release(&mut self, amount: impl Into<Amount>) -> Vec<GrantedRequest> {
        let amount = amount.into();
        if let Some(sum) = self.available.checked_add(&amount) {
            self.available = sum.min_with(&self.capacity);
        }

        // `selected` is in the order the policy wants requests served.
        let selected = self.policy.select(&self.available, &self.pending_queue);
        let mut granted_mask = vec![false; self.pending_queue.len()];
        for &index in &selected {
            granted_mask[index] = true;
        }

        let mut remaining = self.available;
        let mut granted = Vec::with_capacity(selected.len());
        for &index in &selected {
            let request = &self.pending_queue[index];
            remaining = remaining
                .checked_sub(&request.amount)
                .expect("policy selected a fitting request");
            granted.push(GrantedRequest {
                requester: request.requester,
                amount: request.amount,
                cause: request.cause,
            });
        }

        // Rebuild the queue, preserving the arrival order of what remains.
        let mut remaining_queue = VecDeque::with_capacity(self.pending_queue.len());
        for (index, request) in self.pending_queue.drain(..).enumerate() {
            if !granted_mask[index] {
                remaining_queue.push_back(request);
            }
        }
        self.pending_queue = remaining_queue;
        self.available = remaining;

        granted
    }

    /// Capture the mutable resource state for checkpoint/restore.
    pub fn snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            id: self.id,
            capacity: self.capacity,
            available: self.available,
            pending_queue: self.pending_queue.clone(),
        }
    }

    /// Restore mutable state from a previously captured snapshot.
    pub fn restore(&mut self, snapshot: &ResourceSnapshot) {
        self.available = snapshot.available.min_with(&self.capacity);
        self.pending_queue = snapshot.pending_queue.clone();
    }
}
