use crate::action_mapper::ActionMapper;
use crate::action_sender::AnyActionSender;
use futures::FutureExt;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

pub struct Effect<Action: Send + 'static> {
    pub value: EffectValue<Action>,
}

pub type AsyncActionJob<Action> =
    Box<dyn FnOnce(AnyActionSender<Action>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

pub enum EffectValue<Action: Send + 'static> {
    None,
    Send(Action),
    Async(AsyncActionJob<Action>),
    Quit,
}

impl<Action> Effect<Action>
where
    Action: Send + 'static,
{
    pub fn map<F, MappedAction>(self, map: F) -> Effect<MappedAction>
    where
        MappedAction: Send + 'static,
        F: Fn(Action) -> MappedAction + Send + Sync + 'static,
    {
        match self.value {
            EffectValue::None => Effect::none(),
            EffectValue::Quit => Effect::quit(),
            EffectValue::Send(a) => Effect::send(map(a)),
            EffectValue::Async(a) => Effect::<MappedAction>::run(|sender| {
                let mapper = ActionMapper::new(Box::new(sender), map);
                let sender = AnyActionSender::new(Box::new(mapper));

                async move { a(sender).await }.boxed()
            }),
        }
    }

    pub fn run<T>(job: T) -> Self
    where
        T: FnOnce(AnyActionSender<Action>) -> Pin<Box<dyn Future<Output = ()> + Send>>
            + Send
            + 'static,
    {
        Self {
            value: EffectValue::Async(Box::new(job)),
        }
    }

    pub fn none() -> Self {
        Self {
            value: EffectValue::None,
        }
    }

    pub fn quit() -> Self {
        Self {
            value: EffectValue::Quit,
        }
    }

    pub fn send(action: Action) -> Self {
        Self {
            value: EffectValue::Send(action),
        }
    }
}

impl<Action: std::marker::Send> Debug for EffectValue<Action>
where
    Action: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Send(action) => f.write_str(&format!("Send {:#?}", action)),
            Self::Async(_) => f.write_str("Async"),
            Self::Quit => f.write_str("Quit"),
        }
    }
}
