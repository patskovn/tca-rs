use crate::action_sender::ActionSender;
use crate::action_sender::AnyActionSender;
use crate::effect::AsyncActionJob;
use crate::effect::Effect;
use crate::effect::EffectValue;
use crate::event_sender_holder::EventSenderHolder;
use crate::reducer::Reducer;
use crate::store_event::StoreEvent;
use futures::lock::Mutex;
use std::sync::Arc;
use tokio::task::JoinSet;

type EventReceiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;

#[derive(Clone)]
pub struct StoreEngine<State, Action>
where
    Action: Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    state: Arc<std::sync::Mutex<State>>,
    reducer: Arc<dyn Reducer<State, Action> + Sync + Send + 'static>,
    event_sender: Arc<EventSenderHolder<Action>>,
    event_reciever: Arc<Mutex<EventReceiver<StoreEvent<Action>>>>,
}

impl<State, Action> StoreEngine<State, Action>
where
    Action: std::fmt::Debug + Send,
    State: PartialEq + Clone + Send + 'static,
{
    pub fn new(state: State, reducer: impl Reducer<State, Action> + Sync + Send + 'static) -> Self {
        let (event_sender, event_reciever) =
            tokio::sync::mpsc::unbounded_channel::<StoreEvent<Action>>();

        Self {
            state: Arc::new(std::sync::Mutex::new(state)),
            reducer: Arc::new(reducer),
            event_sender: Arc::new(EventSenderHolder::new(event_sender)),
            event_reciever: Arc::new(Mutex::new(event_reciever)),
        }
    }

    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    pub fn run_loop(&self) -> tokio::task::AbortHandle {
        let sender = self.event_sender.clone();

        let receiver = self.event_reciever.clone();
        let reducer = self.reducer.clone();

        let state = self.state.clone();

        let handle = tokio::spawn(async move {
            let mut event_receiver = receiver.lock().await;
            let mut join_set: JoinSet<()> = JoinSet::new();

            let sender = sender;
            let reducer = reducer.clone();

            loop {
                tokio::select! {
                    Some(event) = event_receiver.recv() => {
                        match event {
                        StoreEvent::RedrawUI => {
                            // let state = self.state.lock().await;
                            break
                        },
                        StoreEvent::Effect(effect) => {
                            log::debug!("Handling {:#?}", effect.value);
                            match effect.value {
                                EffectValue::None => {}
                                EffectValue::Send(action) => {
                                    process(state.clone(), action, reducer.clone(), sender.clone()).await;
                                }
                                EffectValue::Quit => {
                                    sender.send_event(StoreEvent::Quit);
                                }
                                EffectValue::Async(job) => {
                                    let any_sender = AnyActionSender::new(Box::new(sender.clone()));
                                    let _andle = join_set.spawn(handle_async(job, any_sender));
                                }
                            }
                        }
                        StoreEvent::Action(action) => {
                            // TODO: Should we handle it here cause slightly faster?
                            sender.send_event(StoreEvent::Effect(Effect::send(action)));
                        },
                        StoreEvent::Quit => { break }
                        }
                    }
                }
            }
        });

        handle.abort_handle()
    }
}

async fn process<State, Action>(
    state: Arc<std::sync::Mutex<State>>,
    action: Action,
    reducer: Arc<dyn Reducer<State, Action> + Sync + Send + 'static>,
    event_sender: Arc<EventSenderHolder<Action>>,
) where
    State: Clone + Send + std::cmp::PartialEq,
    Action: Send,
{
    let effect = {
        let mut state = state.lock().unwrap();
        let state_before = state.clone();
        let effect = reducer.reduce(&mut state, action);
        if state_before != *state {
            event_sender.send_event(StoreEvent::RedrawUI);
        }
        effect
    };
    event_sender.send_event(StoreEvent::Effect(effect));
}

async fn handle_async<Action: Send + 'static>(
    job: AsyncActionJob<Action>,
    event_sender: AnyActionSender<Action>,
) {
    job(event_sender).await
}

impl<State, Action> ActionSender for StoreEngine<State, Action>
where
    Action: std::marker::Send,
    State: PartialEq + Clone + std::marker::Send,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        self.event_sender.send(action);
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
