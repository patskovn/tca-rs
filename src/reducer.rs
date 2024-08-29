use crate::Effect;

pub trait Reducer<State, Action: Send> {
    fn reduce(state: &mut State, action: Action) -> Effect<Action>;
}
