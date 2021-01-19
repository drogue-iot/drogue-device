
pub trait Sink<M> {
    fn tell(&self, message: M);
}