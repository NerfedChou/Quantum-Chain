//! Infrastructure Adapters
//! 
//! Implementations of infrastructure traits (Time, Checksum).

mod checksum;
mod time;

pub use checksum::DefaultChecksumProvider;
pub use time::SystemTimeSource;
