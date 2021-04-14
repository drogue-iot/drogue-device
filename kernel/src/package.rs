use crate::util::ImmediateFuture;
use embassy::executor::Spawner;

pub trait Package {
    fn start(&'static self, spawner: Spawner) -> ImmediateFuture;
}
