use crate::DeviceContext;
use core::future::Future;
use embassy::executor::Spawner;

pub mod boards;

/// A top-level BSP-supporting application.
pub trait App: Sized {
    /// Type defining the associated highly-generic types required by the application.
    /// Each BSP for this app will implement it with board-specific pins/etc generics.
    type Configuration;

    /// The resulting device logic for this application.
    type Device;

    /// Given a BSP board, produce the device.
    fn build(components: Self::Configuration) -> Self::Device;

    type MountFuture<'m>: Future<Output = ()>
    where
        Self: 'm;

    /// Normal drogue-device mount cycle.
    fn mount<'m>(device: &'static Self::Device, spawner: Spawner) -> Self::MountFuture<'m>;
}

/// A board capable of providing an `App` with its required `A::Components`.
pub trait Board: Sized {
    type Peripherals;

    fn configure(peripherals: Self::Peripherals) -> Self;
}

// Board configuration for an application, specific to drogue-device
pub trait AppBoard<A: App>: Board {
    fn take(self) -> A::Configuration;
}

/// Boot the application using the provided board, running through
/// the `App` implementation, mixing the board in, producing a drogue-device device
/// configuring it and mounting it.
pub async fn boot<A: App + 'static, B: AppBoard<A>>(
    ctx: &'static DeviceContext<A::Device>,
    peripherals: B::Peripherals,
    spawner: Spawner,
) {
    let board = B::configure(peripherals);
    let components = board.take();
    let device = A::build(components);

    ctx.configure(device);
    ctx.mount(|device| async move { A::mount(device, spawner).await })
        .await;
}

#[macro_export]
macro_rules! bind_bsp {
    ($bsp:ty, $app_bsp:ident) => {
        struct $app_bsp($bsp);
        impl $crate::bsp::Board for BSP {
            type Peripherals = <$bsp as $crate::bsp::Board>::Peripherals;

            fn configure(peripherals: Self::Peripherals) -> Self {
                BSP(<$bsp>::configure(peripherals))
            }
        }
    };
}

#[macro_export]
macro_rules! boot_bsp {
    ($app:ident, $app_bsp:ident, $p:ident, $spawner:ident) => {
        type AppDevice = <$app<$app_bsp> as $crate::bsp::App>::Device;
        static DEVICE: $crate::kernel::device::DeviceContext<AppDevice> = DeviceContext::new();

        // Boot the board with the imbued app.
        boot::<$app<$app_bsp>, $app_bsp>(&DEVICE, $p, $spawner).await;
    };
}
