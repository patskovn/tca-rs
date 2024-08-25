use tokio::sync::broadcast;

pub trait ChangeObserver {
    fn observe(&self) -> broadcast::Receiver<()>;
}
