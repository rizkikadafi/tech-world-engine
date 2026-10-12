use tw_core::{
    Amount, ComponentId, FifoPolicy, PriorityPolicy, Resource, ResourceId, ResourceRequestResult,
};

fn resource(capacity: u64) -> Resource {
    Resource::new(ResourceId(1), "TestResource", capacity)
}

fn total(request: &tw_core::GrantedRequest) -> u64 {
    request.amount.as_u64().unwrap()
}

#[test]
fn grants_when_capacity_available() {
    let mut res = resource(10);
    let result = res.request(ComponentId(1), 6, None);
    assert_eq!(result, ResourceRequestResult::Granted);
    assert_eq!(res.available, Amount::Discrete(4));
    assert!(res.pending_queue.is_empty());
}

#[test]
fn queues_when_capacity_exhausted() {
    let mut res = resource(10);
    assert_eq!(
        res.request(ComponentId(1), 6, None),
        ResourceRequestResult::Granted
    );
    assert_eq!(
        res.request(ComponentId(2), 5, None),
        ResourceRequestResult::Queued { queue_position: 1 }
    );
    assert_eq!(res.available, Amount::Discrete(4));
    assert_eq!(res.pending_queue.len(), 1);
}

#[test]
fn queues_even_when_available_unless_queue_is_empty() {
    // Fairness: a queued request always stays ahead of a later small request.
    let mut res = resource(10);
    assert_eq!(
        res.request(ComponentId(1), 8, None),
        ResourceRequestResult::Granted
    );
    assert_eq!(
        res.request(ComponentId(2), 5, None),
        ResourceRequestResult::Queued { queue_position: 1 }
    );
    // 2 units are available, but the queue is not empty, so this must queue too.
    assert_eq!(
        res.request(ComponentId(3), 1, None),
        ResourceRequestResult::Queued { queue_position: 2 }
    );
}

#[test]
fn release_unblocks_queued_requests_in_fifo_order() {
    let mut res = resource(10);
    assert_eq!(
        res.request(ComponentId(1), 10, None),
        ResourceRequestResult::Granted
    );
    assert_eq!(
        res.request(ComponentId(2), 4, None),
        ResourceRequestResult::Queued { queue_position: 1 }
    );
    assert_eq!(
        res.request(ComponentId(3), 3, None),
        ResourceRequestResult::Queued { queue_position: 2 }
    );

    let granted = res.release(10);
    assert_eq!(granted.len(), 2);
    assert_eq!(granted[0].requester, ComponentId(2));
    assert_eq!(granted[0].amount, Amount::Discrete(4));
    assert_eq!(granted[1].requester, ComponentId(3));
    assert_eq!(granted[1].amount, Amount::Discrete(3));
    assert_eq!(res.available, Amount::Discrete(3));
}

#[test]
fn fifo_release_stops_at_first_request_that_does_not_fit() {
    let mut res = resource(10);
    assert_eq!(
        res.request(ComponentId(1), 10, None),
        ResourceRequestResult::Granted
    );
    assert_eq!(
        res.request(ComponentId(2), 8, None),
        ResourceRequestResult::Queued { queue_position: 1 }
    );
    assert_eq!(
        res.request(ComponentId(3), 1, None),
        ResourceRequestResult::Queued { queue_position: 2 }
    );

    // Only 5 free: first queued request needs 8, so FIFO grants nothing.
    let granted = res.release(5);
    assert!(granted.is_empty());
    assert_eq!(res.available, Amount::Discrete(5));
    assert_eq!(res.pending_queue.len(), 2);
}

#[test]
fn priority_policy_serves_highest_priority_first() {
    let mut res = resource(10).with_policy(PriorityPolicy);
    assert_eq!(
        res.request(ComponentId(1), 10, None),
        ResourceRequestResult::Granted
    );
    // Low priority arrived first, high priority second.
    res.request_with_priority(ComponentId(2), 4, 0, None);
    res.request_with_priority(ComponentId(3), 4, 10, None);

    let granted = res.release(10);
    assert_eq!(granted.len(), 2);
    assert_eq!(granted[0].requester, ComponentId(3));
    assert_eq!(granted[1].requester, ComponentId(2));
}

#[test]
fn explicit_fifo_policy_matches_default() {
    let mut a = resource(10);
    let mut b = resource(10).with_policy(FifoPolicy);
    for res in [&mut a, &mut b] {
        res.request(ComponentId(1), 10, None);
        res.request(ComponentId(2), 4, None);
    }
    assert_eq!(
        a.release(10).iter().map(total).collect::<Vec<_>>(),
        b.release(10).iter().map(total).collect::<Vec<_>>()
    );
}

#[test]
fn continuous_capacity_supports_fractional_allocation() {
    // 1 Gbps link; two 0.4 Gbps flows fit, a third 0.4 Gbps flow must wait.
    let mut res = Resource::new(ResourceId(2), "Link", 1.0f64);
    assert_eq!(
        res.request(ComponentId(1), 0.4, None),
        ResourceRequestResult::Granted
    );
    assert_eq!(
        res.request(ComponentId(2), 0.4, None),
        ResourceRequestResult::Granted
    );
    assert_eq!(
        res.request(ComponentId(3), 0.4, None),
        ResourceRequestResult::Queued { queue_position: 1 }
    );

    let available = res.available.as_f64();
    assert!((available - 0.2).abs() < 1e-9, "available was {available}");
}

#[test]
fn amount_kind_must_match_capacity() {
    let mut res = resource(10);
    match res.request(ComponentId(1), 1.5f64, None) {
        ResourceRequestResult::Invalid { reason } => assert!(reason.contains("capacity")),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn rejects_request_larger_than_capacity() {
    let mut res = resource(10);
    match res.request(ComponentId(1), 11, None) {
        ResourceRequestResult::Invalid { .. } => {}
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert_eq!(res.available, Amount::Discrete(10));
    assert!(res.pending_queue.is_empty());
}

#[test]
fn release_never_exceeds_capacity() {
    let mut res = resource(10);
    assert_eq!(
        res.request(ComponentId(1), 4, None),
        ResourceRequestResult::Granted
    );
    res.release(100);
    assert_eq!(res.available, Amount::Discrete(10));
}
