#![macro_use]
#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "std")]
mod tests {
    extern crate std;
    use drogue_device::{testutil::*, *, actors::ticker::*, time::{Duration}};

    #[derive(Device)]
    struct TickerDevice {
        handler: ActorContext<'static, TestHandler>,
        ticker: ActorContext<'static, Ticker<'static, TestHandler>>,
    }

    #[drogue::test]
    async fn test_ticker(mut context: TestContext<TickerDevice>) {
        let notified = context.signal();
        context.configure(TickerDevice {
            handler: ActorContext::new(TestHandler::new(notified)),
            ticker: ActorContext::new(Ticker::new(Duration::from_secs(1), TestMessage(1))),
        });

        context.mount(|device| {
            let handler_addr = device.handler.mount(());
            (device.ticker.mount(handler_addr), handler_addr)
        });

        notified.wait_signaled().await;
        assert_eq!(1, notified.message().unwrap().0);
    }
}
