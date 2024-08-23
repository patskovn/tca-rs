use crate::Effect;

pub enum StoreEvent<Action: std::marker::Send + 'static> {
    Action(Action),
    Effect(Effect<Action>),
}
