use crate::{Effect, Store};

/// A protocol that describes how to evolve the current state of an application to the next state,
/// given an action, and describes what ``Effect``s should be executed later by the store, if any.
pub trait Reducer<State, Action: Send> {
    fn reduce(state: &mut State, action: Action) -> Effect<Action>;

    /// Heler for quickly initiating store from any `Feature` that implements reducer for any
    /// initial state
    fn store(initial_state: State) -> Store<State, Action>
    where
        Self: Send + Sync + Sized + 'static,
        Action: std::fmt::Debug + Send + 'static,
        State: PartialEq + Clone + Send + Sync + 'static,
    {
        Store::new::<Self>(initial_state)
    }

    /// Heler for quickly initiating store from any `Feature` that implements reducer for default
    /// state
    fn default_store() -> Store<State, Action>
    where
        Self: Send + Sync + Sized + 'static,
        Action: std::fmt::Debug + Send + 'static,
        State: Default + PartialEq + Clone + Send + Sync + 'static,
    {
        Store::new::<Self>(State::default())
    }
}
