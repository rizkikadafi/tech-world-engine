use std::cell::RefCell;
use std::rc::Rc;

use tw_core::{Amount, Event, Observer, SimulationTime, Snapshot};
use tw_sim::scenarios::resource_contention::{self, RESOURCE_ID};
use tw_sim::SimulationEngine;

#[derive(Debug)]
struct CaptureObserver {
    snapshots: Rc<RefCell<Vec<Snapshot>>>,
}

impl Observer for CaptureObserver {
    fn on_event(&mut self, _event: &Event) {}
    fn on_snapshot(&mut self, snapshot: &Snapshot) {
        self.snapshots.borrow_mut().push(snapshot.clone());
    }
}

#[test]
fn checkpoint_notifies_observers_once_per_component() {
    let captured = Rc::new(RefCell::new(Vec::new()));
    let mut engine = SimulationEngine::new(resource_contention::build());
    engine
        .simulation_mut()
        .register_observer(Box::new(CaptureObserver {
            snapshots: Rc::clone(&captured),
        }));

    let checkpoint = engine.simulation_mut().checkpoint();

    assert_eq!(
        captured.borrow().len(),
        engine.simulation().component_count()
    );
    assert_eq!(checkpoint.component_states.len(), 3);
}

#[test]
fn checkpoint_captures_and_restores_resource_state() {
    let mut engine = SimulationEngine::new(resource_contention::build());

    // Process everything scheduled at t=0: both grants issued, worker3 queued.
    engine
        .simulation_mut()
        .run_until(SimulationTime::new(0));

    let checkpoint = engine.simulation_mut().checkpoint();
    let snap = checkpoint
        .resources
        .iter()
        .find(|r| r.id == RESOURCE_ID)
        .expect("resource captured");
    assert_eq!(snap.capacity, Amount::Discrete(2));
    assert_eq!(snap.available, Amount::Discrete(0));
    assert_eq!(snap.pending_queue.len(), 1);

    // Let the whole simulation finish: everything is released.
    while engine.simulation_mut().step().is_some() {}
    assert_eq!(
        engine.simulation().get_resource(RESOURCE_ID).unwrap().available,
        Amount::Discrete(2)
    );

    // Rewinding the engine-owned resource state restores the contention.
    engine.simulation_mut().restore_resources(&checkpoint);
    let restored = engine.simulation().get_resource(RESOURCE_ID).unwrap();
    assert_eq!(restored.available, Amount::Discrete(0));
    assert_eq!(restored.pending_queue.len(), 1);
}
