use crate::action_sender::ActionSender;

pub struct ActionMapper<Action, MappedAction, F>
where
    Action: Send,
    MappedAction: Send,
    F: Fn(Action) -> MappedAction + Send + 'static,
{
    parent: Box<dyn ActionSender<SendableAction = MappedAction> + Sync>,
    map: F,
    _phantom: std::marker::PhantomData<fn(Action)>,
}

impl<Action, MappedAction, F> ActionMapper<Action, MappedAction, F>
where
    Action: Send,
    MappedAction: Send,
    F: Fn(Action) -> MappedAction + Send,
{
    pub fn new(
        parent: Box<dyn ActionSender<SendableAction = MappedAction> + Sync>,
        map: F,
    ) -> Self {
        Self {
            parent,
            map,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<Action, MappedAction, F> ActionSender for ActionMapper<Action, MappedAction, F>
where
    Action: Send + 'static,
    MappedAction: Send + 'static,
    F: Fn(Action) -> MappedAction + Send + 'static,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        let mapped = (self.map)(action);
        self.parent.send(mapped);
    }
}
