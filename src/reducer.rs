use crate::Effect;

pub trait Reducer<State, Action: Send> {
    fn reduce(&self, state: &mut State, action: Action) -> Effect<Action>;
}
