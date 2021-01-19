
pub trait Sink<M> {
    fn notify(&self, message: M);
}