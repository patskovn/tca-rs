use crate::state_provider::BorrowedState;
use crate::StateProvider;

pub struct StateMapper<State, MappedState, F>
where
    State: Send,
    MappedState: Send,
    F: Fn(&State) -> &MappedState + Clone + Send + 'static,
{
    _parent: Box<dyn StateProvider<State = State> + Sync>,
    _map: F,
    _phantom: std::marker::PhantomData<fn(State)>,
}

impl<State, MappedState, F> StateMapper<State, MappedState, F>
where
    State: Send,
    MappedState: Send,
    F: Fn(&State) -> &MappedState + Send + Clone,
{
    pub fn new(parent: Box<dyn StateProvider<State = State> + Sync>, map: F) -> Self {
        Self {
            _parent: parent,
            _map: map,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<State, MappedState, F> StateProvider for StateMapper<State, MappedState, F>
where
    State: Send + 'static,
    MappedState: Send + 'static,
    F: Fn(&State) -> &MappedState + Clone + Send + 'static,
{
    type State = MappedState;

    fn state(&self) -> BorrowedState<'_, Self::State> {
        todo!("Not implemented")
    }
}
