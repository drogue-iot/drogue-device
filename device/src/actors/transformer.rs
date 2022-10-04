use core::{convert::TryFrom, future::Future};
use ector::{Actor, Address, Inbox};

pub struct Transformer<F, T>
where
    T: TryFrom<F> + 'static,
    F: 'static,
{
    dest: Address<T>,
    _f: core::marker::PhantomData<&'static F>,
}

impl<F, T> Transformer<F, T>
where
    T: TryFrom<F> + 'static,
{
    pub fn new(dest: Address<T>) -> Self {
        Self {
            dest,
            _f: core::marker::PhantomData,
        }
    }
}

impl<F, T> Actor for Transformer<F, T>
where
    T: TryFrom<F> + 'static,
    F: 'static,
{
    type Message<'m> = F where Self: 'm;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self::Message<'m>>;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self::Message<'m>>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        async move {
            loop {
                let m = inbox.next().await;
                if let Ok(c) = T::try_from(m) {
                    self.dest.notify(c).await
                }
            }
        }
    }
}
