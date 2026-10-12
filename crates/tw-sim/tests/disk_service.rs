use tw_core::Amount;
use tw_sim::scenarios::disk_service::{
    self, DISK_ID, JOB1_ID, JOB2_ID, JOB3_ID, SERVICE_TIME,
};
use tw_sim::SimulationEngine;

fn run_to_completion() -> SimulationEngine {
    let mut engine = SimulationEngine::new(disk_service::build());
    while engine.simulation_mut().step().is_some() {}
    engine
}

#[test]
fn service_time_turns_contention_into_latency() {
    let engine = run_to_completion();

    // Single-capacity disk: jobs are granted one after another, each held for
    // SERVICE_TIME ticks. The start delays are 0, 1, 2.
    assert_eq!(disk_service::granted_at(&engine, JOB1_ID), Some(0));
    assert_eq!(
        disk_service::granted_at(&engine, JOB2_ID),
        Some(SERVICE_TIME as i64)
    );
    assert_eq!(
        disk_service::granted_at(&engine, JOB3_ID),
        Some(2 * SERVICE_TIME as i64)
    );

    // Latency = granted_at - arrived_at, and it grows with queue position.
    assert_eq!(
        disk_service::latencies(&engine),
        vec![(JOB1_ID, 0), (JOB2_ID, 2), (JOB3_ID, 4)]
    );
}

#[test]
fn completion_releases_capacity_automatically() {
    let engine = run_to_completion();

    // Each job finishes exactly SERVICE_TIME after it is granted.
    assert_eq!(disk_service::released_at(&engine, JOB1_ID), Some(3));
    assert_eq!(disk_service::released_at(&engine, JOB2_ID), Some(6));
    assert_eq!(disk_service::released_at(&engine, JOB3_ID), Some(9));

    // With every service complete, the disk is fully free again.
    assert_eq!(
        engine
            .simulation()
            .get_resource(DISK_ID)
            .unwrap()
            .available,
        Amount::Discrete(1)
    );
}
