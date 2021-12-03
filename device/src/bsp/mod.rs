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
    type Config;

    fn new(config: Self::Config) -> Self;
}

// Board configuration for an application, specific to drogue-device
pub trait AppBoard<A: App>: Board {
    fn configure(self) -> A::Configuration;
}

/// Boot the application using the provided board, running through
/// the `App` implementation, mixing the board in, producing a drogue-device device
/// configuring it and mounting it.
pub async fn boot<A: App + 'static, B: AppBoard<A>>(
    ctx: &'static DeviceContext<A::Device>,
    board: B,
    spawner: Spawner,
) {
    let components = board.configure();
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
            type Config = <$bsp as $crate::bsp::Board>::Config;

            fn new(config: Self::Config) -> Self {
                BSP(<$bsp>::new(config))
            }
        }
    };
}

#[macro_export]
macro_rules! boot_bsp {
    ($app:ident, $app_bsp:ident, $c:ident) => {
        type AppDevice = <$app<$app_bsp> as $crate::bsp::App>::Device;
        static DEVICE: $crate::kernel::device::DeviceContext<AppDevice> = DeviceContext::new();
        static EXECUTOR: embassy::util::Forever<embassy::executor::Executor> =
            embassy::util::Forever::new();

        #[embassy::task]
        async fn device_main(
            spawner: embassy::executor::Spawner,
            board: $app_bsp,
            device: &'static DeviceContext<AppDevice>,
        ) {
            // Boot the board with the imbued app.
            boot::<$app<$app_bsp>, $app_bsp>(device, board, spawner).await;
        }

        let board = $app_bsp::new($c);

        let executor = EXECUTOR.put(embassy::executor::Executor::new());
        executor.run(|spawner| {
            spawner.spawn(device_main(spawner, board, &DEVICE)).unwrap();
        })
    };
}
