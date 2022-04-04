pub mod tcpstack;

use core::cell::UnsafeCell;

use embassy_net::{Configurator, Device, StackResources};

use crate::actors::tcp::smoltcp::tcpstack::{EmbassyNetTask, SmolRequest};
use crate::drivers::tcp::smoltcp::SmolTcpStack;
use crate::{ActorContext, ActorSpawner, Address, Package};

pub struct SmolTcp<
    DEVICE: Device,
    CONFIG: Configurator,
    const POOL_SIZE: usize,
    const BACKLOG: usize,
    const BUF_SIZE: usize,
> {
    driver: ActorContext<SmolTcpStack<'static, POOL_SIZE, BACKLOG, BUF_SIZE>, 4>,
    embassy_net: ActorContext<EmbassyNetTask, 1>,
    config: UnsafeCell<CONFIG>,
    resources: UnsafeCell<StackResources<1, 2, 8>>,
    device: UnsafeCell<DEVICE>,
}

impl<
        DEVICE: Device,
        CONFIG: Configurator,
        const POOL_SIZE: usize,
        const BACKLOG: usize,
        const BUF_SIZE: usize,
    > SmolTcp<DEVICE, CONFIG, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    pub fn new(device: DEVICE, config: CONFIG) -> Self {
        Self {
            driver: ActorContext::new(),
            embassy_net: ActorContext::new(),
            config: UnsafeCell::new(config),
            resources: UnsafeCell::new(StackResources::new()),
            device: UnsafeCell::new(device),
        }
    }
}

impl<
        DEVICE: Device,
        CONFIG: Configurator,
        const POOL_SIZE: usize,
        const BACKLOG: usize,
        const BUF_SIZE: usize,
    > Package for SmolTcp<DEVICE, CONFIG, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    type Primary = SmolTcpStack<'static, POOL_SIZE, BACKLOG, BUF_SIZE>;
    type Configuration = ();

    fn mount<S: ActorSpawner>(
        &'static self,
        _: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        unsafe {
            embassy_net::init(
                &mut *self.device.get(),
                &mut *self.config.get(),
                &mut *self.resources.get(),
            );
        }
        let addr = self.driver.mount(spawner, SmolTcpStack::new());
        let _ = addr.notify(SmolRequest::Initialize);
        self.embassy_net.mount(spawner, EmbassyNetTask);
        addr
    }
}
