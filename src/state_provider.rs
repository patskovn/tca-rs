pub type BorrowedState<'a, State> = parking_lot::MappedReentrantMutexGuard<'a, State>;

pub trait StateProvider: Send {
    type State;

    fn state(&self) -> BorrowedState<'_, Self::State>;
}

pub struct AnyStateProvider<State> {
    value: Box<dyn StateProvider<State = State> + Sync>,
}

impl<State: Send> AnyStateProvider<State> {
    pub fn new(value: Box<dyn StateProvider<State = State> + Sync>) -> Self {
        Self { value }
    }
}

impl<State: Send> StateProvider for AnyStateProvider<State> {
    type State = State;

    fn state(&self) -> BorrowedState<'_, Self::State> {
        self.value.state()
    }
}
