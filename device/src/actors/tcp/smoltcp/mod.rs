pub mod tcpstack;

use core::cell::RefCell;
use core::cell::UnsafeCell;
use core::future::Future;

use embassy_net::{Configurator, Device, StackResources};

use crate::actors::tcp::smoltcp::tcpstack::{EmbassyNetTask, SmolRequest};
use crate::drivers::tcp::smoltcp::SmolTcpStack;
use crate::traits::ip::{IpProtocol, SocketAddress};
use crate::traits::tcp::{TcpError, TcpStack};
use crate::{Actor, ActorContext, ActorSpawner, Address, Inbox, Package};

pub struct SmolTcp<
    DEVICE: Device,
    CONFIG: Configurator,
    const POOL_SIZE: usize,
    const BACKLOG: usize,
    const BUF_SIZE: usize,
> {
    driver: ActorContext<'static, SmolTcpStack<'static, POOL_SIZE, BACKLOG, BUF_SIZE>, 4>,
    embassy_net: ActorContext<'static, EmbassyNetTask, 1>,
    config: UnsafeCell<Option<CONFIG>>,
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
    pub fn new(device: DEVICE) -> Self {
        Self {
            driver: ActorContext::new(SmolTcpStack::new()),
            embassy_net: ActorContext::new(EmbassyNetTask),
            config: UnsafeCell::new(None),
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
    type Configuration = CONFIG;

    fn mount<S: ActorSpawner>(
        &'static self,
        config: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        unsafe {
            (&mut *self.config.get()).replace(config);
            embassy_net::init(
                &mut *self.device.get(),
                (&mut *self.config.get()).as_mut().unwrap(),
                &mut *self.resources.get(),
            );
        }
        let addr = self.driver.mount((), spawner);
        addr.notify(SmolRequest::Initialize);
        self.embassy_net.mount((), spawner);
        addr
    }
}
