use std::ops::Deref;
use std::sync::Arc;

use tokio::task::AbortHandle;

use crate::action_mapper::ActionMapper;
use crate::action_sender::{ActionSender, AnyActionSender};
use crate::reducer::Reducer;

use super::engine::StoreEngine;

pub struct Store<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    engine: EngineHolder<State, Action>,
}

enum EngineHolder<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    Engine(StoreEngineData<StoreEngine<State, Action>>),
    Parent(Arc<AnyActionSender<Action>>),
}

struct StoreEngineData<T: Send + Sync> {
    value: Arc<T>,
    handle: AbortHandle,
}

impl<State, Action> Store<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    pub fn new<R: Reducer<State, Action> + Sync + Send + 'static>(
        state: State,
        reducer: R,
    ) -> Self {
        let engine = StoreEngine::new(state, reducer);
        let handle = engine.run_loop();
        let store_engine_data = StoreEngineData {
            value: Arc::new(engine),
            handle,
        };
        Self {
            engine: EngineHolder::Engine(store_engine_data),
        }
    }

    pub fn new_from_parent(parent: Arc<AnyActionSender<Action>>) -> Self {
        Self {
            engine: EngineHolder::Parent(parent),
        }
    }

    pub fn state(&self) -> std::sync::MutexGuard<'_, State> {
        match &self.engine {
            EngineHolder::Parent(_parent) => todo!("Implement for scoped Store"),
            EngineHolder::Engine(engine) => engine.value.state(),
        }
    }

    pub async fn observe_changes(&self) {}

    pub fn scope<ChildAction>(
        &self,
        action: impl Fn(ChildAction) -> Action + Send + Sync + 'static,
    ) -> Store<State, ChildAction>
    where
        ChildAction: std::fmt::Debug + std::marker::Send + 'static,
    {
        match &self.engine {
            EngineHolder::Parent(parent) => {
                let parent = Box::new(parent.clone());
                let mapper = ActionMapper::new(parent, action);
                let any_sender = AnyActionSender::new(Box::new(mapper));
                Store::new_from_parent(Arc::new(any_sender))
            }
            EngineHolder::Engine(engine) => {
                let parent = Box::new(engine.value.clone());
                let mapper = ActionMapper::new(parent, action);
                let any_sender = AnyActionSender::new(Box::new(mapper));

                Store::new_from_parent(Arc::new(any_sender))
            }
        }
    }
}

impl<T> ActionSender for Arc<T>
where
    T: ActionSender + std::marker::Sync,
{
    type SendableAction = T::SendableAction;
    fn send(&self, action: Self::SendableAction) {
        self.deref().send(action);
    }
}

impl<State, Action> ActionSender for Store<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        match &self.engine {
            EngineHolder::Engine(engine) => engine.value.send(action),
            EngineHolder::Parent(sender) => sender.send(action),
        }
    }
}

impl<State, Action> Drop for Store<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    fn drop(&mut self) {
        match &self.engine {
            EngineHolder::Engine(engine) => {
                engine.handle.abort();
            }
            EngineHolder::Parent(_) => {}
        };
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::Effect;

    use super::*;

    #[derive(Default, Clone, PartialEq)]
    struct State {
        counter: i32,
    }

    #[derive(Debug)]
    enum Action {
        Increment,
    }

    #[derive(Default)]
    struct Feature {}

    impl Reducer<State, Action> for Feature {
        fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
            match action {
                Action::Increment => {
                    state.counter += 1;
                    Effect::none()
                }
            }
        }
    }

    #[tokio::test]
    async fn test_simple_action() {
        let store = Store::new(Default::default(), Feature::default());
        store.send(Action::Increment);
        tokio::time::sleep(Duration::from_millis(100)).await;
        // let state = store.state();
        //
        // assert_eq!(state.counter, 1);
    }
}
