use chrono::prelude::*;
use std::time::SystemTime;
use crate::grpc::remotefs::UnixPermissions;

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

pub fn unix_perms_to_u16 (perms: &UnixPermissions) -> u16 {
    let u_r: u16 = if perms.u_r { 0o0400 } else { 0 };
    let u_w: u16 = if perms.u_w { 0o0200 } else { 0 };
    let u_x: u16 = if perms.u_x { 0o0100 } else { 0 };
    let g_r: u16 = if perms.g_r { 0o0040 } else { 0 };
    let g_w: u16 = if perms.g_w { 0o0020 } else { 0 };
    let g_x: u16 = if perms.g_x { 0o0010 } else { 0 };
    let o_r: u16 = if perms.o_r { 0o0004 } else { 0 };
    let o_w: u16 = if perms.o_w { 0o0002 } else { 0 };
    let o_x: u16 = if perms.o_x { 0o0001 } else { 0 };
    let sticky: u16 = if perms.sticky { 0o1000 } else { 0 };
    let setgid: u16 = if perms.setgid { 0o2000 } else { 0 };
    let setuid: u16 = if perms.setuid { 0o4000 } else { 0 };
    u_r
    | u_w
    | u_x
    | g_r
    | g_w
    | g_x
    | o_r
    | o_w
    | o_x
    | sticky
    | setgid
    | setuid
}
