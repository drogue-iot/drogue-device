pub mod sensor;
pub mod led;

pub struct ActiveHigh;

pub struct ActiveLow;

pub trait Active {}

impl Active for ActiveHigh {}
impl Active for ActiveLow {}
