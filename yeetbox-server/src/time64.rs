use std::time::{SystemTime, UNIX_EPOCH};

pub const TIME64_SEC_MASK: u64 = 0xFFFF_FFFF_FFF0_0000;
pub const TIME64_NANOSEC_MASK: u64 = 0x0000_0000_000F_FFFF;
pub const TIME64_UNKNOWN_TIME: Time64 = Time64(0);

/**
 * This type represents a timestamp that has nanosecond precision, but still
 * fits in a 64-bit word and can represent a range of time covering thousands of
 * years.
 *
 * Uses the upper 44 bits for seconds since the Unix Epoch.
 * Uses the lower 20 bits for nanoseconds.
 * (Because 20 bits can encode just over 1M values.)
 *
 * This gives you efficient conversion to protobuf timestamps
 * without multiplication or division, while still being able
 * to represent the foreseeable lifespan of human civilization,
 * and fitting this information in a single word.
 */
#[derive(bytemuck::Pod, bytemuck::Zeroable, Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(C)]
pub struct Time64 (pub u64);

impl Time64 {

    pub fn is_unknown (self) -> bool {
        self == TIME64_UNKNOWN_TIME
    }

    pub fn known (self) -> Option<Self> {
        if self.is_unknown() {
            None
        } else {
            Some(self)
        }
    }

    pub fn now () -> Self {
        let now = SystemTime::now();
        // We can unwrap, because this should never happen.
        let since_the_epoch = now.duration_since(UNIX_EPOCH).unwrap();
        /* We do not include nanoseconds in this timestamp, because it could be
        used to fingerprint the operating system, and its generally unnecessary
        anyway. */
        let nanos: i32 = (since_the_epoch.as_nanos() % 1_000_000) as i32;
        Self::from_parts(since_the_epoch.as_secs(), nanos)
    }

    pub fn from_parts (secs: u64, ns: i32) -> Self {
        let mut ret: u64 = 0;
        ret |= secs as u64;
        ret <<= 20;
        ret |= ns as u64;
        Time64(ret)
    }

}

impl From<prost_types::Timestamp> for Time64 {

    fn from(value: prost_types::Timestamp) -> Self {
        Self::from_parts(value.seconds as u64, value.nanos)
    }

}

impl Into<prost_types::Timestamp> for Time64 {

    fn into(self) -> prost_types::Timestamp {
        prost_types::Timestamp {
            seconds: ((self.0 & TIME64_SEC_MASK) >> 20) as i64,
            nanos: (self.0 & TIME64_NANOSEC_MASK) as i32,
        }
    }

}

impl Default for Time64 {
    fn default() -> Self {
        Time64(0)
    }
}
