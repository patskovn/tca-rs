use crate::state_provider::BorrowedState;
use crate::StateProvider;

pub struct StateMapper<State, MappedState, F>
where
    State: Send,
    MappedState: Send,
    F: Fn(&State) -> &MappedState + Clone + Send + 'static,
{
    parent: Box<dyn StateProvider<State = State> + Sync>,
    map: F,
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
            parent,
            map,
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
        BorrowedState::map(self.parent.state(), self.map.clone())
    }
}
