pub trait ActionSender: Send {
    type SendableAction;

    fn send(&self, action: Self::SendableAction);
}

pub struct AnyActionSender<Action: Send + 'static> {
    value: Box<dyn ActionSender<SendableAction = Action> + Sync>,
}

impl<Action: Send> AnyActionSender<Action> {
    pub fn new(value: Box<dyn ActionSender<SendableAction = Action> + Sync>) -> Self {
        Self { value }
    }
}

impl<Action: Send> ActionSender for AnyActionSender<Action> {
    type SendableAction = Action;
    fn send(&self, action: Action) {
        self.value.send(action)
    }
}
