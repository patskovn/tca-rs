use crate::action_mapper::ActionMapper;
use crate::action_sender::AnyActionSender;
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
            EffectValue::Async(a) => Effect::<MappedAction>::run(|sender| async move {
                let mapper = ActionMapper::new(Box::new(sender), map);
                let sender = AnyActionSender::new(Box::new(mapper));
                a(sender).await
            }),
        }
    }

    pub fn run<T, Fut>(job: T) -> Self
    where
        Fut: Future<Output = ()> + Send + 'static,
        T: FnOnce(AnyActionSender<Action>) -> Fut + Send + 'static,
    {
        let boxed_job: AsyncActionJob<Action> = Box::new(move |sender: AnyActionSender<Action>| {
            // Call the original `job` to get the future
            let fut = job(sender);
            // Box the future and pin it
            Box::pin(fut)
        });
        Self {
            value: EffectValue::Async(boxed_job),
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
