pub trait Source<M> {
    fn poll(&self) -> Future<Output = M>
}
