pub trait Led {
    type Error;
    fn on(&mut self) -> Result<(), Self::Error>;
    fn off(&mut self) -> Result<(), Self::Error>;
    fn toggle(&mut self) -> Result<(), Self::Error>;
    fn state(&self) -> Result<bool, Self::Error>;
}

pub trait TextDisplay {
    type Error;

    type ScrollFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn scroll<'m>(&'m mut self, text: &'m str) -> Self::ScrollFuture<'m>;

    fn putc(&mut self, c: char) -> Result<(), Self::Error>;
}
