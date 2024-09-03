use crate::{Effect, Store};

pub trait Reducer<State, Action: Send> {
    fn reduce(state: &mut State, action: Action) -> Effect<Action>;

    fn store(initial_state: State) -> Store<State, Action>
    where
        Self: Send + Sync + Sized + 'static,
        Action: std::fmt::Debug + Send + 'static,
        State: PartialEq + Clone + Send + Sync + 'static,
    {
        Store::new::<Self>(initial_state)
    }

    fn default_store() -> Store<State, Action>
    where
        Self: Send + Sync + Sized + 'static,
        Action: std::fmt::Debug + Send + 'static,
        State: Default + PartialEq + Clone + Send + Sync + 'static,
    {
        Store::new::<Self>(State::default())
    }
}
