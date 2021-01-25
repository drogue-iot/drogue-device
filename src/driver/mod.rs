pub mod led;
pub mod sensor;

pub struct ActiveHigh;

pub struct ActiveLow;

pub trait Active {}

impl Active for ActiveHigh {}
impl Active for ActiveLow {}

