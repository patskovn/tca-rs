use criterion::BenchmarkId;

use tca::ActionSender;
use tca::ChangeObserver;
use tca::Effect;

use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

#[derive(Debug)]
struct Event {}

#[derive(Default, Clone, PartialEq, Eq)]
struct State {
    counter: usize,
}

#[derive(Debug)]
enum Action {
    StartButtonTapped,
    Event(Event),
}

struct Feature {}
impl tca::Reducer<State, Action> for Feature {
    fn reduce(state: &mut State, action: Action) -> Effect<Action> {
        match action {
            Action::StartButtonTapped => Effect::run(move |send| async move {
                send.send(Action::Event(Event {}));
            }),
            Action::Event(_) => {
                state.counter += 1;
                Effect::none()
            }
        }
    }
}

async fn store_direct_throughput(size: usize) {
    let store = tca::Store::new::<Feature>(State::default());
    let mut observer = store.observe();
    let counter = size;
    for _i in 0..counter {
        store.send(Action::StartButtonTapped);
    }

    loop {
        let event = observer.recv().await;
        match event {
            Ok(()) => {
                if store.state().counter == counter {
                    return;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                break;
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
        }
    }
}
fn store_throughput_bench(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let size: usize = 1000;
    c.bench_with_input(
        BenchmarkId::new("store_throughput_bench", size),
        &size,
        |b, &s| b.to_async(&runtime).iter(|| store_direct_throughput(s)),
    );
}

criterion_group!(store, store_throughput_bench);
criterion_main!(store);
