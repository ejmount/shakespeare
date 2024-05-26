mod actor;
pub use actor::{Handle, Outcome, Shell, Spawn};

mod role;
pub use role::{Channel, Receiver, Role, Sender};

mod returnval;
pub use returnval::{Envelope, ReturnCaster, ReturnEnvelope, ReturnPath};
