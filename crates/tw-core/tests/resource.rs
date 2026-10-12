use tw_core::{ComponentId, Resource, ResourceId, ResourceRequestResult};

fn resource(capacity: u64) -> Resource {
    Resource::new(ResourceId(1), "TestResource", capacity)
}

#[test]
fn grants_when_capacity_available() {
    let mut res = resource(10);
    let result = res.request(ComponentId(1), 6, None);
    assert_eq!(result, ResourceRequestResult::Granted);
    assert_eq!(res.available, 4);
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
    assert_eq!(res.available, 4);
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
    assert_eq!(granted[0].amount, 4);
    assert_eq!(granted[1].requester, ComponentId(3));
    assert_eq!(granted[1].amount, 3);
    assert_eq!(res.available, 3);
}

#[test]
fn release_stops_at_first_request_that_does_not_fit() {
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

    // Only 5 free: first queued request needs 8, so nothing can be granted.
    let granted = res.release(5);
    assert!(granted.is_empty());
    assert_eq!(res.available, 5);
    assert_eq!(res.pending_queue.len(), 2);
}

#[test]
fn rejects_request_larger_than_capacity() {
    let mut res = resource(10);
    match res.request(ComponentId(1), 11, None) {
        ResourceRequestResult::Invalid { .. } => {}
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert_eq!(res.available, 10);
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
    assert_eq!(res.available, 10);
}
