use crate::action_sender::ActionSender;
use crate::action_sender::AnyActionSender;
use crate::change_observer::ChangeObserver;
use crate::effect::AsyncActionJob;
use crate::effect::Effect;
use crate::effect::EffectValue;
use crate::event_sender_holder::EventSenderHolder;
use crate::reducer::Reducer;
use crate::store_event::StoreEvent;
use futures::lock::Mutex;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinSet;

type EventReceiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

struct EngineData<State, Action: Send + 'static> {
    state: std::sync::Mutex<State>,
    reducer: Box<dyn Reducer<State, Action> + Sync + Send + 'static>,
    event_sender: Arc<EventSenderHolder<Action>>,
    event_reciever: Mutex<EventReceiver<StoreEvent<Action>>>,
    redraw_sender: std::sync::RwLock<Option<broadcast::Sender<()>>>,
}

pub struct StoreEngine<State, Action>
where
    Action: Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    data: Arc<EngineData<State, Action>>,
}

impl<State, Action> StoreEngine<State, Action>
where
    Action: std::fmt::Debug + Send,
    State: PartialEq + Clone + Send + 'static,
{
    pub fn new(state: State, reducer: impl Reducer<State, Action> + Sync + Send + 'static) -> Self {
        let (event_sender, event_reciever) =
            tokio::sync::mpsc::unbounded_channel::<StoreEvent<Action>>();

        let (tx, _) = broadcast::channel::<()>(10);

        let data = EngineData {
            state: std::sync::Mutex::new(state),
            reducer: Box::new(reducer),
            event_sender: Arc::new(EventSenderHolder::new(event_sender)),
            event_reciever: Mutex::new(event_reciever),
            redraw_sender: std::sync::RwLock::new(Some(tx)),
        };

        Self {
            data: Arc::new(data),
        }
    }

    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.data.state.lock().unwrap()
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
                                    let _andle = join_set.spawn(handle_async(job, any_sender));
                                }
                            }
                        }
                        StoreEvent::Action(action) => {
                            // TODO: Should we handle it here cause slightly faster?
                            data.event_sender.send_event(StoreEvent::Effect(Effect::send(action)));
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
    State: Clone + Send + PartialEq,
    Action: Send,
{
    let effect = {
        let mut state = data.state.lock().unwrap();
        let state_before = state.clone();
        let effect = data.reducer.reduce(&mut state, action);
        let has_changes = state_before != *state;
        drop(state);

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

async fn handle_async<Action: Send + 'static>(
    job: AsyncActionJob<Action>,
    event_sender: AnyActionSender<Action>,
) {
    job(event_sender).await
}

impl<State, Action> ChangeObserver for StoreEngine<State, Action>
where
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
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
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        self.data.event_sender.send(action);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Default, Clone, PartialEq)]
    struct State {}

    #[derive(Debug)]
    enum Action {}

    #[derive(Default)]
    struct Feature {}

    impl Reducer<State, Action> for Feature {
        fn reduce(&self, _state: &mut State, _action: Action) -> Effect<Action> {
            Effect::none()
        }
    }

    #[tokio::test]
    async fn test_run_loop_is_parallel() {
        let _engine = StoreEngine::new(State::default(), Feature::default());
    }
}
