use chrono::prelude::*;
use std::time::SystemTime;

pub fn grpc_timestamp_to_chrono (grpc_time: &prost_types::Timestamp) -> Option<DateTime<Utc>> {
    match Utc.timestamp_opt(grpc_time.seconds, grpc_time.nanos as u32) {
        chrono::LocalResult::Single(dt) => Some(dt),
        _ => None,
    }
}

pub fn system_time_to_grpc_timestamp (systime: SystemTime) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: systime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64,
        nanos: systime.duration_since(SystemTime::UNIX_EPOCH).unwrap().subsec_nanos() as i32,
    }
}
