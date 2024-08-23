use crate::Effect;

pub enum StoreEvent<Action: std::marker::Send + 'static> {
    RedrawUI,
    Action(Action),
    Effect(Effect<Action>),
    Quit,
}
