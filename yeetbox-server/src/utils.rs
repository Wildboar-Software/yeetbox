use chrono::prelude::*;

pub fn grpc_timestamp_to_chrono (grpc_time: &prost_types::Timestamp) -> Option<DateTime<Utc>> {
    match Utc.timestamp_opt(grpc_time.seconds, grpc_time.nanos as u32) {
        chrono::LocalResult::Single(dt) => Some(dt),
        _ => None,
    }
}