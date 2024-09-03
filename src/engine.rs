use crate::action_sender::ActionSender;
use crate::action_sender::AnyActionSender;
use crate::change_observer::ChangeObserver;
use crate::effect::Effect;
use crate::effect::EffectValue;
use crate::event_sender_holder::EventSenderHolder;
use crate::reducer::Reducer;
use crate::state_provider::BorrowedState;
use crate::store_event::StoreEvent;
use crate::StateProvider;
use futures::lock::Mutex;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinSet;

type EventReceiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;
type ReduceHandler<State, Action> = dyn Fn(&mut State, Action) -> Effect<Action> + Sync + Send;

pub(crate) struct EngineData<State, Action: Send + 'static> {
    pub(crate) state: parking_lot::ReentrantMutex<State>,
    reducer: Box<ReduceHandler<State, Action>>,
    event_sender: Arc<EventSenderHolder<Action>>,
    event_reciever: Mutex<EventReceiver<StoreEvent<Action>>>,
    redraw_sender: std::sync::RwLock<Option<broadcast::Sender<()>>>,
}

#[derive(Clone)]
pub struct StoreEngine<State, Action>
where
    Action: Send + 'static,
    State: PartialEq + Clone + Send + Sync + 'static,
{
    pub(crate) data: Arc<EngineData<State, Action>>,
}

impl<State, Action> StoreEngine<State, Action>
where
    Action: std::fmt::Debug + Send,
    State: PartialEq + Clone + Send + Sync + 'static,
{
    pub fn new<R: Reducer<State, Action> + Sync + Send + 'static>(state: State) -> Self {
        let (event_sender, event_reciever) =
            tokio::sync::mpsc::unbounded_channel::<StoreEvent<Action>>();

        let (tx, _) = broadcast::channel::<()>(10);

        let data = EngineData {
            state: parking_lot::ReentrantMutex::new(state),
            reducer: Box::new(R::reduce),
            event_sender: Arc::new(EventSenderHolder::new(event_sender)),
            event_reciever: Mutex::new(event_reciever),
            redraw_sender: std::sync::RwLock::new(Some(tx)),
        };

        Self {
            data: Arc::new(data),
        }
    }

    pub fn run_loop(&self) -> tokio::task::AbortHandle {
        let data = self.data.clone();

        let handle = tokio::spawn(async move {
            let mut event_receiver = data.event_reciever.lock().await;
            let mut join_set: JoinSet<()> = JoinSet::new();

            'run_loop: loop {
                tokio::select! {
                    Some(event) = event_receiver.recv() => {
                        match event {
                        StoreEvent::Effect(effect) => {
                            log::debug!("Handling {:#?}", effect.value);
                            match effect.value {
                                EffectValue::None => {}
                                EffectValue::Send(action) => {
                                    process(&data, action).await;

                                }
                                EffectValue::Quit => {
                                    let mut redraw = data.redraw_sender.write().unwrap();
                                    *redraw = None;
                                    break 'run_loop;
                                }
                                EffectValue::Async(job) => {
                                    let any_sender = AnyActionSender::new(Box::new(data.event_sender.clone()));
                                    join_set.spawn(job(any_sender));
                                }
                            }
                        }
                        StoreEvent::Action(action) => {
                            process(&data, action).await;
                        }
                        }
                    }
                }
            }
        });

        handle.abort_handle()
    }
}

async fn process<State, Action>(data: &EngineData<State, Action>, action: Action)
where
    State: Clone + Send + Sync + PartialEq,
    Action: Send,
{
    let effect = {
        let locked_state = data.state.lock();
        let mut new_state = locked_state.clone();
        let effect = (data.reducer)(&mut new_state, action);
        let has_changes = *locked_state != new_state;
        unsafe {
            *data.state.data_ptr() = new_state;
        }
        drop(locked_state);

        if has_changes {
            if let Ok(redraw) = data.redraw_sender.read() {
                match redraw.deref() {
                    Some(val) => {
                        // We may have error here if buffer already filled in.
                        // But that's okay, we'll just skip one redraw pass.
                        let _ = val.send(());
                    }
                    None => {}
                }
            }
        }
        effect
    };
    data.event_sender.send_event(StoreEvent::Effect(effect));
}

impl<State, Action> ChangeObserver for StoreEngine<State, Action>
where
    Action: Send,
    State: PartialEq + Clone + Send + Sync,
{
    fn observe(&self) -> broadcast::Receiver<()> {
        let sender = self.data.redraw_sender.read().unwrap();
        match sender.deref() {
            Some(sender) => sender.subscribe(),
            None => {
                let (autoclose_tx, _) = broadcast::channel(0);
                let subscriber = autoclose_tx.subscribe();
                drop(autoclose_tx);

                subscriber
            }
        }
    }
}

impl<State, Action> ActionSender for StoreEngine<State, Action>
where
    Action: Send,
    State: PartialEq + Clone + Send + Sync,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        self.data.event_sender.send(action);
    }
}

impl<State, Action> StateProvider for StoreEngine<State, Action>
where
    Action: Send,
    State: PartialEq + Clone + Send + Sync,
{
    type State = State;

    fn state(&self) -> BorrowedState<'_, State> {
        parking_lot::ReentrantMutexGuard::map(self.data.state.lock(), |s| s)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tokio::sync::Semaphore;

    #[tokio::test]
    async fn reentrant_mutex_not_locks() -> anyhow::Result<()> {
        let semaphore = Arc::new(Semaphore::new(1));
        let semaphore_lock = semaphore.clone().acquire_owned().await?;
        let data = Arc::new(parking_lot::ReentrantMutex::new(0)); // Start with 0 instead of 10 for simplicity

        let iterations = 1_000_000;
        let move_data = data.clone();

        // Spawn a new task that will modify the data in a loop
        let handle = tokio::spawn(async move {
            drop(semaphore_lock);
            for _ in 0..iterations {
                let lock = move_data.lock();
                let raw_data = move_data.data_ptr();
                unsafe { *raw_data += 1 };
                drop(lock);
            }
        });

        // Make sure other thread started
        let _a = semaphore.clone().acquire_owned().await?;

        // Main task also modifies the data in a loop
        for _ in 0..iterations {
            let lock = data.lock();
            let raw_data = data.data_ptr();
            unsafe { *raw_data += 1 };
            drop(lock);
        }

        handle.await?; // Wait for the spawned task to finish

        // Verify the final value of the data
        let final_value = *data.lock();
        assert_eq!(final_value, iterations * 2);

        Ok(())
    }
}
