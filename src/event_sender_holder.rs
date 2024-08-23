use std::sync::Arc;

use crate::action_sender::ActionSender;
use crate::store_event::StoreEvent;

pub type EventSender<T> = tokio::sync::mpsc::UnboundedSender<T>;

pub struct EventSenderHolder<Action>
where
    Action: Send + 'static,
{
    event_sender: EventSender<StoreEvent<Action>>,
}

impl<Action> EventSenderHolder<Action>
where
    Action: Send + 'static,
{
    pub fn new(event_sender: EventSender<StoreEvent<Action>>) -> Self {
        Self { event_sender }
    }

    pub fn send_event(&self, evt: StoreEvent<Action>) {
        self.event_sender.send(evt).unwrap();
    }
}

impl<Action> ActionSender for Arc<EventSenderHolder<Action>>
where
    Action: Send,
{
    type SendableAction = Action;

    fn send(&self, action: Action) {
        self.send_event(StoreEvent::Action(action));
    }
}
