use std::ops::Deref;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::task::AbortHandle;

use crate::action_mapper::ActionMapper;
use crate::action_sender::{ActionSender, AnyActionSender};
use crate::change_observer::ChangeObserver;
use crate::reducer::Reducer;
use crate::state_mapper::StateMapper;
use crate::state_provider::{AnyStateProvider, BorrowedState};
use crate::StateProvider;

use super::engine::StoreEngine;

pub struct Store<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    engine: EngineHolder<State, Action>,
}

#[derive(Clone)]
enum EngineHolder<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    Engine(StoreEngineData<StoreEngine<State, Action>>),
    Parent(AnyStoreLike<State, Action>),
}

#[derive(Clone)]
struct AnyStoreLike<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: Send + 'static,
{
    state_provider: Arc<AnyStateProvider<State>>,
    action_sender: Arc<AnyActionSender<Action>>,
    change_observer: Arc<dyn ChangeObserver + Send + Sync>,
}

impl<State, Action> AnyStoreLike<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: Send + 'static,
{
    fn new(
        state_provider: Arc<AnyStateProvider<State>>,
        action_sender: Arc<AnyActionSender<Action>>,
        change_observer: Arc<dyn ChangeObserver + Send + Sync>,
    ) -> Self {
        Self {
            state_provider,
            action_sender,
            change_observer,
        }
    }
}

#[derive(Clone)]
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

    fn new_from_parent(parent: AnyStoreLike<State, Action>) -> Self {
        Self {
            engine: EngineHolder::Parent(parent),
        }
    }

    pub fn state(&self) -> BorrowedState<'_, State> {
        match &self.engine {
            EngineHolder::Parent(_parent) => todo!("Implement for scoped Store"),
            EngineHolder::Engine(engine) => engine.value.state(),
        }
    }

    pub fn scope<ChildState, ChildAction>(
        &self,
        state: impl Fn(&State) -> &ChildState + Clone + Send + Sync + 'static,
        action: impl Fn(ChildAction) -> Action + Send + Sync + 'static,
    ) -> Store<ChildState, ChildAction>
    where
        ChildState: Send + Clone + std::cmp::PartialEq + 'static,
        ChildAction: std::fmt::Debug + Send + 'static,
    {
        match &self.engine {
            EngineHolder::Parent(parent) => {
                let mapped_state = StateMapper::new(Box::new(parent.state_provider.clone()), state);
                let any_mapped_state = AnyStateProvider::new(Box::new(mapped_state));

                let parent_sender = Box::new(parent.action_sender.clone());
                let mapped_sender = ActionMapper::new(parent_sender, action);
                let any_sender = AnyActionSender::new(Box::new(mapped_sender));
                let parent_store_like = AnyStoreLike::new(
                    Arc::new(any_mapped_state),
                    Arc::new(any_sender),
                    parent.change_observer.clone(),
                );
                Store::new_from_parent(parent_store_like)
            }
            EngineHolder::Engine(engine) => {
                let mapped_state = StateMapper::new(Box::new(engine.value.clone()), state);
                let any_mapped_state = AnyStateProvider::new(Box::new(mapped_state));

                let parent = Box::new(engine.value.clone());
                let mapped_sender = ActionMapper::new(parent, action);
                let any_sender = AnyActionSender::new(Box::new(mapped_sender));
                let parent_store_like = AnyStoreLike::new(
                    Arc::new(any_mapped_state),
                    Arc::new(any_sender),
                    engine.value.clone(),
                );

                Store::new_from_parent(parent_store_like)
            }
        }
    }
}

impl<State, Action> Clone for Store<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    fn clone(&self) -> Self {
        self.scope(|s| s, |a| a)
    }
}

impl<State, Action> ChangeObserver for Store<State, Action>
where
    Action: std::fmt::Debug + Send + 'static,
    State: PartialEq + Clone + Send + 'static,
{
    fn observe(&self) -> broadcast::Receiver<()> {
        match &self.engine {
            EngineHolder::Parent(parent) => parent.change_observer.observe(),
            EngineHolder::Engine(engine) => engine.value.observe(),
        }
    }
}

impl<T> ActionSender for Arc<T>
where
    T: ActionSender + Sync,
{
    type SendableAction = T::SendableAction;
    fn send(&self, action: Self::SendableAction) {
        self.deref().send(action);
    }
}

impl<T> StateProvider for Arc<T>
where
    T: StateProvider + Sync,
{
    type State = T::State;

    fn state(&self) -> BorrowedState<'_, Self::State> {
        self.deref().state()
    }
}

impl<T> ChangeObserver for Arc<T>
where
    T: ChangeObserver,
{
    fn observe(&self) -> broadcast::Receiver<()> {
        self.deref().observe()
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
            EngineHolder::Parent(sender) => sender.action_sender.send(action),
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
    use super::*;
    use crate::Effect;

    #[derive(Default, Clone, PartialEq)]
    struct State {
        counter: i32,
    }

    #[derive(Debug)]
    enum Action {
        Increment,
        Quit,
    }

    #[derive(Default)]
    struct Feature {}
    impl Feature {
        fn store() -> Store<State, Action> {
            Store::new(Default::default(), Self::default())
        }
    }

    impl Reducer<State, Action> for Feature {
        fn reduce(&self, state: &mut State, action: Action) -> Effect<Action> {
            match action {
                Action::Increment => {
                    state.counter += 1;
                    Effect::none()
                }
                Action::Quit => Effect::quit(),
            }
        }
    }

    #[tokio::test]
    async fn test_simple_action() -> anyhow::Result<()> {
        let store = Feature::store();
        store.send(Action::Increment);
        store.observe().recv().await?;
        assert_eq!(store.state().counter, 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_closes_on_quit() -> anyhow::Result<()> {
        let store = Feature::store();
        store.send(Action::Increment);
        store.observe().recv().await?;

        let mut long_observe = store.observe();
        store.send(Action::Quit);

        assert_eq!(
            long_observe.recv().await,
            Err(broadcast::error::RecvError::Closed)
        );

        Ok(())
    }
}
