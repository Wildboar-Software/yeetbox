use crate::grpc::remotefs::{
    AbortTransactionArg, AbortTransactionResult, AppendArg, AppendResult, AuthenticateArg,
    AuthenticateResult, CommitTransactionArg, CommitTransactionResult, CopyArg, CopyResult,
    CreateLinkArg, CreateLinkResult, DeleteArg, DeleteManyArg, DeleteManyResult, DeleteResult,
    DownloadArg, DownloadResult, FileSystemEvent, FileVersion, GetAttributesArg,
    GetAttributesResult, GetAuditTrailArg, GetAuditTrailResult, GetAvailableSaslMechanismsResult,
    GetPresignedDownloadArg, GetPresignedDownloadResult, GetPresignedUploadArg,
    GetPresignedUploadResult, GetServiceInfoArg, GetServiceInfoResult, ListArg,
    ListIncompleteUploadsArg, ListIncompleteUploadsResult, ListResult, MakeDirectoryArg,
    MakeDirectoryResult, MoveArg, MoveResult, PatchArg, PatchResult, SetAttributesArg,
    SetAttributesResult, StartTransactionArg, StartTransactionResult, UnlinkArg, UnlinkResult,
    UploadArg, UploadResult, WatchManyArg, WatchOnceArg, WatchOnceResult, ListEntry,
    FsAttributes, UnixPermissions, ObjectType
};
use crate::storage::Storage;
use std::cmp::min;
use std::mem;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use tokio::fs::{self, read};
use tokio::fs::{
    create_dir, create_dir_all, read_dir, read_link, rename, symlink, try_exists,
    File, OpenOptions, remove_dir_all, metadata, remove_file
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use ulid::Ulid;
use crate::utils::{system_time_to_grpc_timestamp, unix_perms_to_u16};
use redb::{Database, ReadOnlyTable, ReadableTable, Table, TableDefinition, WriteTransaction, Error as RedbError};
use unicode_normalization::UnicodeNormalization;
use crate::time64::{Time64, TIME64_UNKNOWN_TIME};

const FS_TABLE_NAME: &str = "fs_table";
const SEQ_TABLE_NAME: &str = "seq";
const VER_TABLE_NAME: &str = "ver";

// This is the table where file system objects are stored.
const FS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new(FS_TABLE_NAME);

// This is the table where sequence states are stored.
const SEQ_TABLE: TableDefinition<&str, u64> = TableDefinition::new(SEQ_TABLE_NAME);

// This is the table where file versions are stored.
const VER_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new(VER_TABLE_NAME);

const HEAD_FILE_NAME: &str = "_head";
const BLOBS_DIR_NAME: &str = "_blobs";
const LATEST_FILE_NAME: &str = "_latest";
const VERSIONS_DIR_NAME: &str = "_vers";
const THUMBNAIL_FILE_NAME: &str = "_thumb";
const MAJOR_VERSION_1: &str = "000000000001";
const MINOR_VERSION_0: &str = "000000000000";

/// This is to prevent a malicious client from sending a huge length and
/// filling up the server's memory.
/// This used to be set to 8_000_000, but I had to reduce it to 1MB because of
/// limits that Tonic puts on message decoding sizes.
const MAX_READ_SIZE: usize = 8 * 1024 * 1024;

/**
 * Uses the upper 44 bits for seconds since the Unix Epoch.
 * Uses the lower 20 bits for nanoseconds.
 * (Because 20 bits can encode just over 1M values.)
 *
 * This gives you efficient conversion to protobuf timestamps
 * without multiplication or division, while still being able
 * to represent the foreseeable lifespan of human civilization,
 * and fitting this information in a single word.
 */
// pub type time64 = u64;

// pub const UNKNOWN_TIME: time64 = 0;

pub type FileSystemId = u64;

pub const ROOT_FSID: FileSystemId = 0;

/**
 * I highly doubt it would ever be viable to define more than 2^16
 * storage tiers. Human users could never reason about that many
 * tiers or decide between them.
 */
pub type StorageTierId = u16;

// FIXME: Actually allow the user to request a storage tier and route the blob accordingly.
pub const DEFAULT_STORAGE_TIER: StorageTierId = 0;

// #[derive(bytemuck::NoUninit, Clone)]
// #[repr(C)]
pub struct FsRecordKey {
    pub parent_id: FileSystemId,
    pub relative_key: String,
}

// Used by the flags field.
// TODO: Mask off only the LS nibble for file type. The MS nibble could be used for more flags.
pub const OBJ_TYPE_FILE: u8 = 0;
pub const OBJ_TYPE_FOLDER: u8 = 1;
pub const OBJ_TYPE_SYMLINK: u8 = 2;
pub const OBJ_TYPE_FIFO: u8 = 3;
pub const OBJ_TYPE_SOCKET: u8 = 4;
pub const OBJ_TYPE_APPEND_BLOB: u8 = 5; // append operation will not create a new version.
pub const OBJ_TYPE_BLOCK_BLOB: u8 = 6;
pub const UNIX_PERM_U_R: u16 = 0o0400;
pub const UNIX_PERM_U_W: u16 = 0o0200;
pub const UNIX_PERM_U_X: u16 = 0o0100;
pub const UNIX_PERM_G_R: u16 = 0o0040;
pub const UNIX_PERM_G_W: u16 = 0o0020;
pub const UNIX_PERM_G_X: u16 = 0o0010;
pub const UNIX_PERM_O_R: u16 = 0o0004;
pub const UNIX_PERM_O_W: u16 = 0o0002;
pub const UNIX_PERM_O_X: u16 = 0o0001;
pub const UNIX_PERM_STICKY: u16 = 0o1000;
pub const UNIX_PERM_SETGID: u16 = 0o2000;
pub const UNIX_PERM_SETUID: u16 = 0o4000;
pub const UNKNOWN_SIZE: u64 = 0xFFFF_FFFF_FFFF_FFFF;
// TODO: UTF-8 flag
// TODO: empty flag

// TODO: Optimization flag indicating empty file, or perhaps inlined file contents

// FIXME: Actually, I think these fields should go in the versions.
// Then again, they need to be in here for things not versioned, like folders.
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
#[repr(C)]
pub struct FsRecordValue {
    pub id: FileSystemId,

    /// The time at which the first version of the object was created.
    pub create_time: Time64,

    /// The time at which the object contents were last modified.
    pub modify_time: Time64,

    /// The time at which the object was last read.
    pub access_time: Time64,

    /// The time at which object permissions or metadata was last changed.
    pub change_time: Time64,

    /// The time at which the object was deleted.
    pub delete_time: Time64,

    pub r#type: u8, // Used for File Type
    pub other1: u8, // for future use + alignment.
    pub other2: u16, // for future use + alignment.
    pub other3: u32, // for future use + alignment.
    pub latest_version: u64,
    // Actually, variable length part will be the non-normalized file name.
    // Variable length part:
    // File: latest/head blob path
    // Symlink: destination path
}

impl FsRecordValue {

    // pub fn is_unknown_size (&self) -> bool {
    //     self.size == UNKNOWN_SIZE
    // }

    // pub fn known_size (&self) -> Option<u64> {
    //     if self.is_unknown_size() {
    //         None
    //     } else {
    //         Some(self.size)
    //     }
    // }

    pub fn obj_type (&self) -> Option<ObjectType> {
        let obj_type: u8 = self.r#type;
        match obj_type {
            OBJ_TYPE_FILE => Some(ObjectType::File),
            OBJ_TYPE_FOLDER => Some(ObjectType::Folder),
            OBJ_TYPE_SYMLINK => Some(ObjectType::Symlink),
            OBJ_TYPE_FIFO => Some(ObjectType::Fifo),
            OBJ_TYPE_SOCKET => Some(ObjectType::Socket),
            _ => None,
        }
    }

}

impl Default for FsRecordValue {
    fn default() -> Self {
        FsRecordValue {
            id: 0,
            create_time: TIME64_UNKNOWN_TIME,
            modify_time: TIME64_UNKNOWN_TIME,
            access_time: TIME64_UNKNOWN_TIME,
            change_time: TIME64_UNKNOWN_TIME,
            delete_time: TIME64_UNKNOWN_TIME,
            r#type: OBJ_TYPE_FILE,
            other1: 0,
            other2: 0,
            other3: 0,
            latest_version: 1,
        }
    }
}

pub type FsVersion = u64;

#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
#[repr(C)]
pub struct VersionRecordKey {
    pub file_id: FileSystemId,
    pub version: FsVersion,
}

// #[derive(bytemuck::NoUninit, Clone, Copy)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
#[repr(C)]
pub struct VersionRecordValue {
    /// The time at which this record was created.
    pub create_time: Time64,

    /// The time at which the this record was last read.
    pub access_time: Time64,

    /// This is used primarily for implementing append.
    /// Upon appending, the appended data is added to the end of the blob, but
    /// but the previous version still "pretends" the file ends at `length`.
    /// This conserves space, but still conserves the individual patches along
    /// with their timestamps.
    pub length: u64,
    pub uid: u32,
    pub gid: u32,
    pub flags: u16, // Unix Permissions and other flags.
    pub storage_tier: u16,
    pub other: u32, // Reserved for future use + padding

    // TODO: [u8; N] = First N bytes of the file, which can be useful for searching file magic like "ELF" or "JFIF"
    // It must be at least 16 bytes, since bytes 6 through 9 are used for JPEG images.
    // 128 bytes or so might be long enough to store most titles and text previews
}

fn get_next_id (w: &WriteTransaction<'_>, table_name: &str) -> std::result::Result<FileSystemId, RedbError> {
    let mut seq_writer = w.open_table(SEQ_TABLE)?;
    let last_id = seq_writer.get(FS_TABLE_NAME)?
        .map(|s| s.value())
        .unwrap_or(0);
    let next_id = last_id + 1;
    seq_writer.insert(FS_TABLE_NAME, next_id)?;
    Ok(last_id + 1)
}

fn make_key (parent_id: FileSystemId, file_name: &str) -> Vec<u8> {
    [
        parent_id.to_be_bytes().as_slice(),
        file_name.nfkd().collect::<String>().as_bytes(),
    ].concat()
}

fn is_readable_obj_type (obj_type: u8) -> bool {
    [
        OBJ_TYPE_FILE,
        OBJ_TYPE_APPEND_BLOB,
        OBJ_TYPE_BLOCK_BLOB,
        OBJ_TYPE_FIFO,
        OBJ_TYPE_SOCKET,
    ].contains(&obj_type)
}

// TODO: Handle symlinks
// TODO: Use in upload and make_directory
fn descend_path (path: &[String], fs: &ReadOnlyTable<'_, &[u8], &[u8]>)-> std::result::Result<FileSystemId, tonic::Status> {
    let mut parent_id: FileSystemId = ROOT_FSID;
    for pc in path {
        let key = make_key(parent_id, pc);
        let maybe_value = fs.get(key.as_slice())
            .map_err(|_| tonic::Status::internal("error trying to read fs key"))?;
        if maybe_value.is_none() {
            // TODO: Check permissions to create.
            // TODO: Create if permitted.
            return Err(tonic::Status::invalid_argument("no such parent path"));
        }
        let value = maybe_value.unwrap();
        let value = value.value();
        if value.len() < mem::size_of::<FsRecordValue>() {
            return Err(tonic::Status::internal("corrupted fs record"));
        }
        parent_id = u64::from_be_bytes([
            value[0], value[1], value[2], value[3],
            value[4], value[5], value[6], value[7],
        ]);
        let record: FsRecordValue = bytemuck::pod_read_unaligned(&value[0..mem::size_of::<FsRecordValue>()]);
        let obj_type: u8 = record.r#type;
        if obj_type != OBJ_TYPE_FOLDER {
            return Err(tonic::Status::invalid_argument("path contained a non-folder"));
        }
    }
    Ok(parent_id)
}

// TODO: Use try equivalents to handle database corruption a little better.
// TODO: Replace occurrences of this code with this function.
fn fs_record_from_bytes (value: &[u8]) -> FsRecordValue {
    bytemuck::pod_read_unaligned(&value[0..mem::size_of::<FsRecordValue>()])
}

fn version_key_from_bytes (value: &[u8]) -> VersionRecordKey {
    bytemuck::pod_read_unaligned(&value[0..mem::size_of::<VersionRecordKey>()])
}

fn version_value_from_bytes (value: &[u8]) -> VersionRecordValue {
    bytemuck::pod_read_unaligned(&value[0..mem::size_of::<VersionRecordValue>()])
}

#[derive(Debug)]
pub struct DatabaseStorage {
    pub blobs_path: std::path::PathBuf,
    pub db_path: std::path::PathBuf,
    pub db: Database,
}

impl DatabaseStorage {
    pub fn new() -> Self {
        let blobs_path = std::path::PathBuf::from("/tmp/yeetbox/blobs");
        let db_path = std::path::PathBuf::from("/tmp/yeetbox/yeetbox.db");
        let db = Database::create(&db_path).expect("Unable to open DB");
        let w = db.begin_write().expect("failed to open writer");
        w.open_table(FS_TABLE).expect("failed to create fs table");
        w.open_table(SEQ_TABLE).expect("failed to create seq table");
        w.commit().expect("failed to create tables");
        // if cfg!(debug) {
        //     let r = db.begin_read().unwrap();
        //     let fs = r.open_table(FS_TABLE).unwrap();
        //     let iter = fs.range(0u64.to_be_bytes().as_slice()..u64::MAX.to_be_bytes().as_slice()).unwrap();
        //     for i in iter {
        //         let (k, v) = i.unwrap();
        //         dbg!(k.value());
        //         dbg!(v.value());
        //     }
        // }
        DatabaseStorage {
            blobs_path,
            db_path,
            db,
        }
    }
}

type WatchManyStream = tonic::codec::Streaming<FileSystemEvent>;

#[tonic::async_trait]
impl Storage for DatabaseStorage {
    async fn watch_many(
        &self,
        request: tonic::Request<WatchManyArg>,
    ) -> std::result::Result<tonic::Response<WatchManyStream>, tonic::Status> {
        unimplemented!()
    }

    async fn make_directory(
        &self,
        request: tonic::Request<MakeDirectoryArg>,
    ) -> std::result::Result<tonic::Response<MakeDirectoryResult>, tonic::Status> {
        let req = request.into_inner();
        if req.target.is_none() {
            return Err(tonic::Status::invalid_argument("target is required"));
        }
        let mut fullpath = req.target.unwrap().path;
        if fullpath.len() == 0 {
            return Err(tonic::Status::invalid_argument("target may not be empty"));
        }
        // let fullpath = strs_to_path(&req.target.unwrap().path);
        let r = self.db.begin_read()
            .map_err(|_| tonic::Status::internal("could not read from database"))?;
        let fs = r.open_table(FS_TABLE)
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        let mut parent_id: FileSystemId = ROOT_FSID;
        let folder_name = fullpath.pop().unwrap();
        let folder_name = folder_name.trim();
        for pc in fullpath {
            let key = [
                parent_id.to_be_bytes().to_vec(),
                pc.into_bytes(),
            ].concat();
            let maybe_value = fs.get(key.as_slice())
                .map_err(|_| tonic::Status::internal("error trying to read fs key"))?;
            if maybe_value.is_none() {
                // TODO: Check permissions to create.
                // TODO: Create if permitted.
                return Err(tonic::Status::invalid_argument("no such parent path"));
            }
            let value = maybe_value.unwrap();
            let value = value.value();
            if value.len() < mem::size_of::<FsRecordValue>() {
                return Err(tonic::Status::internal("corrupted fs record"));
            }
            parent_id = u64::from_be_bytes([
                value[0], value[1], value[2], value[3],
                value[4], value[5], value[6], value[7],
            ]);
            let record: FsRecordValue = bytemuck::pod_read_unaligned(&value[0..mem::size_of::<FsRecordValue>()]);
            let obj_type: u8 = record.r#type;
            if obj_type != OBJ_TYPE_FOLDER {
                return Err(tonic::Status::invalid_argument("cannot place under non-folder"));
            }
        }

        let w = self.db.begin_write()
            .map_err(|_| tonic::Status::internal("could not write to database"))?;
        {
            let mut fs_writer = w.open_table(FS_TABLE)
                .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
            let key = [
                parent_id.to_be_bytes().as_slice(),
                folder_name.nfkd().collect::<String>().as_bytes(),
            ].concat();
            {
                let maybe_existing = fs_writer.get(key.as_slice())
                    .map_err(|_| tonic::Status::internal("error trying to check if dir exists already"))?;
                if maybe_existing.is_some() {
                    return Err(tonic::Status::invalid_argument("object already exists with that name"));
                }
            }
            let seq_writer = w.open_table(SEQ_TABLE)
                .map_err(|_| tonic::Status::internal("could not read from seq table"))?;
            let last_id = seq_writer.get(FS_TABLE_NAME)
                .map_err(|_| tonic::Status::internal("error trying to read seq key"))?
                .map(|s| s.value())
                .unwrap_or(0);
            let next_id = last_id + 1;
            let new_dir_record = FsRecordValue {
                id: next_id,
                r#type: OBJ_TYPE_FOLDER,
                ..Default::default() // TODO: Fill in more details.
            };
            let new_value = [
                bytemuck::bytes_of(&new_dir_record),
                folder_name.as_bytes(),
            ].concat();
            let maybe_existing_value = fs_writer.insert(key.as_slice(), new_value.as_slice())
                .map_err(|_| tonic::Status::internal("could not write to fs table"))?;
            if maybe_existing_value.is_some() {
                // Nothing needs to be done here. The write wasn't committed yet.
                return Err(tonic::Status::internal("replaced existing folder"));
            }
        }
        w.commit()
            .map_err(|_| tonic::Status::internal("could not commit changes"))?;
        Ok(tonic::Response::new(MakeDirectoryResult {
            ..Default::default()
        }))
    }

    async fn upload(
        &self,
        request: tonic::Request<UploadArg>,
    ) -> std::result::Result<tonic::Response<UploadResult>, tonic::Status> {
        let req = request.into_inner();
        if req.target.is_none() {
            return Err(tonic::Status::invalid_argument("target is required"));
        }
        let mut fullpath = req.target.unwrap().path;
        if fullpath.len() == 0 {
            return Err(tonic::Status::invalid_argument("target may not be empty"));
        }

        let ulid = if req.continuation.len() == 0 {
            // If the client did not supply a continuation token, this is a new
            // upload, so we create a new blob.
            Ulid::new()
        } else {
            // Otherwise, we use the continuation token as the blob name.
            let a: [u8; 16] = req.continuation[..]
                .try_into()
                .map_err(|_| tonic::Status::invalid_argument("invalid continuation token"))?;
            Ulid::try_from(a)
                .map_err(|_| tonic::Status::invalid_argument("invalid continuation token"))?
        };

        let blob_file_name = format!("{}.blob", ulid);
        let mut blob_path = self.blobs_path.clone();
        blob_path.push(blob_file_name);
        let mut f = OpenOptions::new()
            .append(true) // We have to seek to the end to append.
            .create(true)
            .open(&blob_path)
            .await?;
        // TODO: https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.set_len
        // TODO: https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.sync_all
        // TODO: What if the write is 0-length?
        f.write(&req.data).await?;
        drop(f);

        if req.incomplete {
            let ulid_bytes = ulid.to_bytes();
            return Ok(tonic::Response::new(UploadResult {
                continuation: ulid_bytes.to_vec(),
                ..Default::default()
            }));
        }

        let r = self.db.begin_read()
            .map_err(|_| tonic::Status::internal("could not read from database"))?;
        let fs = r.open_table(FS_TABLE)
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        let mut parent_id: FileSystemId = ROOT_FSID;
        let file_name = fullpath.pop().unwrap();
        let file_name = file_name.trim(); // TODO: Cow trim
        for pc in fullpath {
            let key = [
                parent_id.to_be_bytes().to_vec(),
                pc.into_bytes(),
            ].concat();
            let maybe_value = fs.get(key.as_slice())
                .map_err(|_| tonic::Status::internal("error trying to read fs key"))?;
            if maybe_value.is_none() {
                // TODO: Check permissions to create.
                // TODO: Create if permitted.
                return Err(tonic::Status::invalid_argument("no such parent path"));
            }
            let value = maybe_value.unwrap();
            let value = value.value();
            if value.len() < mem::size_of::<FsRecordValue>() {
                return Err(tonic::Status::internal("corrupted fs record"));
            }
            parent_id = u64::from_be_bytes([
                value[0], value[1], value[2], value[3],
                value[4], value[5], value[6], value[7],
            ]);
            let record: FsRecordValue = bytemuck::pod_read_unaligned(&value[0..mem::size_of::<FsRecordValue>()]);
            let obj_type: u8 = record.r#type;
            if obj_type != OBJ_TYPE_FOLDER {
                return Err(tonic::Status::invalid_argument("cannot place under non-folder"));
            }
        }

        let w = self.db.begin_write()
            .map_err(|_| tonic::Status::internal("could not write to database"))?;
        {
            let mut fs_writer = w.open_table(FS_TABLE)
                .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
            let key = make_key(parent_id, &file_name);
            let mut existing_file_id: Option<FileSystemId> = None;
            let mut create_time: Option<Time64> = None;
            let mut access_time: Option<Time64> = None;
            let mut latest_version: Option<FsVersion> = None;
            {
                let maybe_existing = fs_writer.get(key.as_slice())
                    .map_err(|_| tonic::Status::internal("error trying to check if file exists already"))?;
                if maybe_existing.is_some() {
                    if !req.next { // If we are not explicitly trying to create a new version...
                        return Err(tonic::Status::invalid_argument("object already exists with that name"));
                    }
                    let existing = maybe_existing.unwrap();
                    let bytes = existing.value();
                    let prev_fs_record = fs_record_from_bytes(bytes);
                    existing_file_id = Some(prev_fs_record.id);
                    create_time = Some(prev_fs_record.create_time);
                    access_time = Some(prev_fs_record.access_time);
                    latest_version = Some(prev_fs_record.latest_version);
                }
            }

            let file_id = if let Some(fid) = existing_file_id {
                fid
            } else {
                get_next_id(&w, FS_TABLE_NAME)
                    .map_err(|_| tonic::Status::internal("could not read and/or increment sequence for FS table"))?
            };

            let current_version = latest_version.map(|v| v + 1).unwrap_or(1);
            let new_file_record = FsRecordValue {
                id: file_id,
                r#type: OBJ_TYPE_FILE, // FIXME: Allow creating different file types.
                create_time: create_time.unwrap_or(Time64::now()),
                modify_time: Time64::now(),
                access_time: access_time.unwrap_or(Time64::now()),
                change_time: TIME64_UNKNOWN_TIME,
                delete_time: TIME64_UNKNOWN_TIME,
                latest_version: current_version,
                ..Default::default()
            };
            let new_value = [
                bytemuck::bytes_of(&new_file_record),
                file_name.as_bytes(),
            ].concat();
            let maybe_existing_value = fs_writer.insert(key.as_slice(), new_value.as_slice())
                .map_err(|_| tonic::Status::internal("could not write to fs table"))?;
            if maybe_existing_value.is_some() {
                // Nothing needs to be done here. The write wasn't committed yet.
                return Err(tonic::Status::internal("replaced existing folder"));
            }

            // The entire blob was uploaded in a single message, not fragmented.
            let single_message_blob: bool = !req.incomplete && req.continuation.len() == 0;
            let new_file_version = VersionRecordValue {
                create_time: Time64::now(),
                access_time: TIME64_UNKNOWN_TIME,
                uid: req.uid,
                gid: req.gid,
                flags: req.perms.as_ref().map(unix_perms_to_u16).unwrap_or(0o755),
                storage_tier: DEFAULT_STORAGE_TIER,
                // TODO: Lazy-load this, or increment the blob size in a record
                length: if single_message_blob { req.data.len() as u64 } else { UNKNOWN_SIZE },
                other: 0,
            };
            let saved_blob_path = if !single_message_blob {
                // If the blob was fragmented among requests, we have to rename
                // the blob so the previous ULID cannot be abused to append more
                // data.
                let new_ulid = Ulid::new();
                let blob_file_name = format!("{}.blob", new_ulid);
                let mut new_blob_path = self.blobs_path.clone();
                new_blob_path.push(blob_file_name);
                rename(&blob_path, &new_blob_path).await?;
                new_blob_path
            } else {
                blob_path
            };
            let version_key = VersionRecordKey {
                file_id,
                version: current_version,
            };
            let version_value_bytes = [
                bytemuck::bytes_of(&new_file_version),
                saved_blob_path.to_str().unwrap().as_bytes(),
            ].concat();
            let mut v_writer = w.open_table(VER_TABLE)
                .map_err(|_| tonic::Status::internal("could not read from versions table"))?;
            v_writer.insert(bytemuck::bytes_of(&version_key), version_value_bytes.as_slice())
                .map_err(|_| tonic::Status::internal("could not write to fs table"))?;
        }
        w.commit()
            .map_err(|_| tonic::Status::internal("could not commit changes"))?;
        return Ok(tonic::Response::new(UploadResult {
            ..Default::default()
        }));
        // TODO: Use req.reserve to reserve space for the file.
    }

    async fn append(
        &self,
        request: tonic::Request<AppendArg>,
    ) -> std::result::Result<tonic::Response<AppendResult>, tonic::Status> {
        let req = request.into_inner();
        if req.target.is_none() {
            return Err(tonic::Status::invalid_argument("target is required"));
        }
        let target = req.target.as_ref().unwrap();
        let mut fullpath = target.path.clone();
        if fullpath.len() == 0 {
            return Err(tonic::Status::invalid_argument("target may not be empty"));
        }
        let r = self.db.begin_read()
            .map_err(|_| tonic::Status::internal("could not read from database"))?;
        let fs = r.open_table(FS_TABLE)
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        let file_name = fullpath.pop().unwrap();
        let dir_name = fullpath;
        let parent_id = descend_path(&dir_name, &fs)?;
        let file_key = make_key(parent_id, &file_name);

        /* We open a write transaction here to avoid a TOCTOU bug where the
        latest version of a blob could change between checking it and appending. */
        let w = self.db.begin_write()
            .map_err(|_| tonic::Status::internal("could not write to database"))?;
        let fs = w.open_table(FS_TABLE)
            .map_err(|_| tonic::Status::internal("could not write to fs table"))?;
        let maybe_file_rec = fs.get(&file_key.as_slice())
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        if maybe_file_rec.is_none() {
            return Err(tonic::Status::invalid_argument("no such file"));
        }
        let file_rec = maybe_file_rec.unwrap();
        let file_rec = fs_record_from_bytes(file_rec.value());
        if !is_readable_obj_type(file_rec.r#type) {
            return Err(tonic::Status::invalid_argument("not a readable object"));
        }
        if target.version.as_ref().is_some_and(|v| v.major != file_rec.latest_version) {
            return Err(tonic::Status::invalid_argument("not appending to latest version"));
        }
        match file_rec.r#type {
            OBJ_TYPE_FILE => {
                let version_key = VersionRecordKey {
                    file_id: file_rec.id,
                    version: file_rec.latest_version,
                };
                let new_version_key;
                let new_version_rec;
                let mut v = w.open_table(VER_TABLE)
                    .map_err(|_| tonic::Status::internal("could not read from versions table"))?;
                { // Scoped so immutable ref to v is dropped before mutable ref is needed.
                    let maybe_version_rec = v.get(bytemuck::bytes_of(&version_key))
                        .map_err(|_| tonic::Status::internal("failed to read requested version"))?;
                    if maybe_version_rec.is_none() {
                        return Err(tonic::Status::internal("database corrupted: missing version"));
                    }
                    let version_rec_bytes = maybe_version_rec.unwrap();
                    let version_rec_bytes = version_rec_bytes.value();
                    let version_rec = version_value_from_bytes(version_rec_bytes);
                    let known_size: bool = version_rec.length != UNKNOWN_SIZE;
                    let version_blob = unsafe {
                        std::str::from_utf8_unchecked(&version_rec_bytes[mem::size_of::<VersionRecordValue>()..])
                    };
                    let mut blob_path = self.blobs_path.clone();
                    blob_path.push(version_blob);

                    let mut f = OpenOptions::new()
                        .append(true) // We have to seek to the end to append.
                        .open(&blob_path)
                        .await?;
                    f.write(&req.data).await?;
                    let new_size: u64 = if known_size {
                        version_rec.length + req.data.len() as u64
                    } else {
                        f.metadata().await?.size()
                    };
                    drop(f);
                    // TODO: https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.sync_all
                    // TODO: What if the write is 0-length?

                    new_version_key = VersionRecordKey {
                        file_id: file_rec.id,
                        version: file_rec.latest_version + 1,
                    };
                    new_version_rec = VersionRecordValue {
                        create_time: Time64::now(),
                        length: new_size,
                        ..version_rec
                    };
                }
                let new_version_key = bytemuck::bytes_of(&new_version_key);
                let new_version_rec = bytemuck::bytes_of(&new_version_rec);
                v.insert(&new_version_key, &new_version_rec)
                    .map_err(|_| tonic::Status::internal("could not write to fs table"))?;
                // TODO: Write new version
                Ok(tonic::Response::new(AppendResult {
                    ..Default::default()
                }))
            },
            _ => unimplemented!()
        }
    }

    async fn patch(
        &self,
        request: tonic::Request<PatchArg>,
    ) -> std::result::Result<tonic::Response<PatchResult>, tonic::Status> {
        unimplemented!()
    }

    async fn download(
        &self,
        request: tonic::Request<DownloadArg>,
    ) -> std::result::Result<tonic::Response<DownloadResult>, tonic::Status> {
        let req = request.into_inner();
        if req.target.is_none() {
            return Err(tonic::Status::invalid_argument("target is required"));
        }
        let target = req.target.as_ref().unwrap();
        let mut fullpath = target.path.clone();
        if fullpath.len() == 0 {
            return Err(tonic::Status::invalid_argument("target may not be empty"));
        }
        let r = self.db.begin_read()
            .map_err(|_| tonic::Status::internal("could not read from database"))?;
        let fs = r.open_table(FS_TABLE)
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        let file_name = fullpath.pop().unwrap();
        let dir_name = fullpath;
        let parent_id = descend_path(&dir_name, &fs)?;
        let file_key = make_key(parent_id, &file_name);
        let maybe_file_rec = fs.get(&file_key.as_slice())
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        if maybe_file_rec.is_none() {
            return Err(tonic::Status::invalid_argument("no such file"));
        }
        let file_rec = maybe_file_rec.unwrap();
        let file_rec = fs_record_from_bytes(file_rec.value());
        if !is_readable_obj_type(file_rec.r#type) {
            return Err(tonic::Status::invalid_argument("not a readable object"));
        }
        match file_rec.r#type {
            OBJ_TYPE_FILE => {
                let requested_version = target.version.as_ref().map(|v| v.major).unwrap_or(file_rec.latest_version);
                let version_key = VersionRecordKey {
                    file_id: file_rec.id,
                    version: requested_version,
                };
                let v = r.open_table(VER_TABLE)
                    .map_err(|_| tonic::Status::internal("could not read from versions table"))?;
                let maybe_version_rec = v.get(bytemuck::bytes_of(&version_key))
                    .map_err(|_| tonic::Status::internal("failed to read requested version"))?;
                if maybe_version_rec.is_none() {
                    return Err(tonic::Status::internal("database corrupted: missing version"));
                }
                let version_rec_bytes = maybe_version_rec.unwrap();
                let version_rec_bytes = version_rec_bytes.value();
                let version_rec = version_value_from_bytes(version_rec_bytes);
                let known_size: bool = version_rec.length != UNKNOWN_SIZE;
                if req.offset > 0 && known_size && req.offset > version_rec.length {
                    return Err(tonic::Status::invalid_argument("offset beyond end of file"));
                }
                let version_blob = unsafe {
                    std::str::from_utf8_unchecked(&version_rec_bytes[mem::size_of::<VersionRecordValue>()..])
                };
                let mut blob_path = self.blobs_path.clone();
                blob_path.push(version_blob);
                let mut f = OpenOptions::new()
                    .write(false)
                    .read(true)
                    .open(&blob_path)
                    .await?;
                if req.offset > 0 {
                    f.seek(std::io::SeekFrom::Start(req.offset as u64)).await?;
                }
                let mut req_length: u64 = req.length;
                // If the requested length exceeds the bounds of the file, truncate
                if known_size && (req.offset + req.length) > version_rec.length {
                    req_length = version_rec.length - req.offset;
                }
                let alloc_size: usize = min(req_length as usize, MAX_READ_SIZE);
                let mut data = Vec::with_capacity(alloc_size);
                // We have to set the length to the capacity because the read function
                // uses the length to determine how many bytes to read, not the capacity.
                unsafe {
                    data.set_len(alloc_size);
                }
                let bytes_read = f.read(&mut data).await?;
                // Then we truncate it to the actual number of bytes read.
                data.truncate(bytes_read);
                Ok(tonic::Response::new(DownloadResult {
                    data,
                    more: bytes_read == alloc_size,
                    ..Default::default()
                }))
            },
            _ => unimplemented!()
        }
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteArg>,
    ) -> std::result::Result<tonic::Response<DeleteResult>, tonic::Status> {
        let req = request.into_inner();
        if req.target.is_none() {
            return Err(tonic::Status::invalid_argument("target is required"));
        }
        let target = req.target.as_ref().unwrap();
        let mut fullpath = target.path.clone();
        if fullpath.len() == 0 {
            return Err(tonic::Status::invalid_argument("target may not be empty"));
        }
        let r = self.db.begin_read()
            .map_err(|_| tonic::Status::internal("could not read from database"))?;
        let fs = r.open_table(FS_TABLE)
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        let file_name = fullpath.pop().unwrap();
        let dir_name = fullpath;
        let parent_id = descend_path(&dir_name, &fs)?;
        let file_key = make_key(parent_id, &file_name);

        let w = self.db.begin_write()
            .map_err(|_| tonic::Status::internal("could not write to database"))?;
        let mut fs = w.open_table(FS_TABLE)
            .map_err(|_| tonic::Status::internal("could not write to fs table"))?;
        let maybe_file_rec = fs.remove(&file_key.as_slice())
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        if maybe_file_rec.is_none() {
            return Err(tonic::Status::invalid_argument("no such file"));
        }
        let file_rec_guard = maybe_file_rec.unwrap();
        let file_rec_bytes = file_rec_guard.value();
        let file_rec = fs_record_from_bytes(&file_rec_bytes);
        if target.version.as_ref().is_some_and(|v| v.major != file_rec.latest_version) {
            return Err(tonic::Status::invalid_argument("not deleting the latest version"));
        }
        if file_rec.r#type != OBJ_TYPE_FILE {
            // TODO: Check if the folder is empty. Recurse if requested.
            drop(file_rec_guard);
            drop(fs);
            w.commit()
                .map_err(|_| tonic::Status::internal("could not delete folder"))?;
            // Only files have versions.
            return Ok(tonic::Response::new(DeleteResult {
                shredded: false, // TODO: Implement shredding.
                ..Default::default()
            }));
        }
        let mut v = w.open_table(VER_TABLE)
            .map_err(|_| tonic::Status::internal("could not read from versions table"))?;
        let mut latest_version = file_rec.latest_version;

        // Pay close attention: this loop is constructed to not underflow the latest_version variable.
        loop {
            let version_key = VersionRecordKey {
                file_id: file_rec.id,
                version: latest_version,
            };

            let version_key = bytemuck::bytes_of(&version_key);
            let deleted_v = v.remove(&version_key)
                .map_err(|_| tonic::Status::internal("could not delete from version table"))?;
            if let Some(deleted_v) = deleted_v {
                let d = deleted_v.value();
                if d.len() > mem::size_of::<VersionRecordValue>() {
                    let blob_name = unsafe {
                        std::str::from_utf8_unchecked(&d[mem::size_of::<VersionRecordValue>()..])
                    };
                    let mut blob_path = self.blobs_path.clone();
                    blob_path.push(blob_name);
                    remove_file(&blob_path).await?;
                    // There does not seem to be a good async shredding library for Rust anywhere.
                }
            }
            if latest_version == 0 {
                break;
            }
            latest_version -= 1;
        }

        Ok(tonic::Response::new(DeleteResult {
            shredded: false, // TODO: Implement shredding.
            ..Default::default()
        }))
    }

    // FIXME: Normalize file names in the key. Save the non-normalized equivalent in the record.
    async fn list(
        &self,
        request: tonic::Request<ListArg>,
    ) -> std::result::Result<tonic::Response<ListResult>, tonic::Status> {
        let req = request.into_inner();
        if req.target.is_none() {
            return Err(tonic::Status::invalid_argument("target is required"));
        }
        let fullpath = req.target.unwrap().path;
        // let fullpath = strs_to_path(&req.target.unwrap().path);
        let r = self.db.begin_read()
            .map_err(|_| tonic::Status::internal("could not read from database"))?;
        let fs = r.open_table(FS_TABLE)
            .map_err(|_| tonic::Status::internal("could not read from fs table"))?;
        let mut parent_id: FileSystemId = ROOT_FSID;
        for pc in fullpath {
            let key = [
                parent_id.to_be_bytes().to_vec(),
                pc.nfkc().collect::<String>().into_bytes(),
            ].concat();
            let maybe_value = fs.get(key.as_slice())
                .map_err(|_| tonic::Status::internal("error trying to read fs key"))?;
            if maybe_value.is_none() {
                // TODO: Check permissions to create.
                // TODO: Create if permitted.
                return Err(tonic::Status::invalid_argument("no such parent path"));
            }
            let value = maybe_value.unwrap();
            let value = value.value();
            if value.len() < mem::size_of::<FsRecordValue>() {
                return Err(tonic::Status::internal("corrupted fs record"));
            }
            parent_id = u64::from_be_bytes([
                value[0], value[1], value[2], value[3],
                value[4], value[5], value[6], value[7],
            ]);
            let record: FsRecordValue = bytemuck::pod_read_unaligned(&value[0..mem::size_of::<FsRecordValue>()]);
            let obj_type: u8 = record.r#type;
            if obj_type != OBJ_TYPE_FOLDER {
                return Err(tonic::Status::invalid_argument("cannot list under non-folder"));
            }
        }
        let next_id = parent_id + 1;
        let mut entries: Vec<ListEntry> = Vec::new();
        let fs_iter = fs.range(parent_id.to_be_bytes().as_slice()..next_id.to_be_bytes().as_slice())
            .map_err(|_| tonic::Status::internal("error trying to list subordinate entries"))?;
        for iteration in fs_iter {
            if let Err(e) = &iteration {
                return Err(tonic::Status::internal("failed to continue listing entries"));
            }
            let (key, value) = iteration.unwrap();
            let key = key.value();
            let value = value.value();
            if key.len() < mem::size_of::<FileSystemId>() {
                return Err(tonic::Status::internal("corrupted fs record"));
            }
            if value.len() < mem::size_of::<FsRecordValue>() {
                return Err(tonic::Status::internal("corrupted fs record"));
            }
            let record: FsRecordValue = bytemuck::pod_read_unaligned(&value[0..mem::size_of::<FsRecordValue>()]);
            let name_bytes = &value[mem::size_of::<FsRecordValue>()..];
            // Unsafe is reasonable here, because we only put UTF-8 strings here.
            let friendly_file_name = unsafe { std::str::from_utf8_unchecked(name_bytes) };
            // let obj_type: u8 = record.r#type;
            let attrs = FsAttributes{
                // There is no efficient way to determine the file type, because
                // you would have to issue a read request for each folder
                // beneath to determine if the file is a file or folder.
                // The only thing we can definitely determine is if it is a
                // symbolic link.
                r#type: record.obj_type().map(|x| x.into()),
                // uid: Some(record.uid),
                // gid: Some(record.gid),
                // perms: Some(UnixPermissions{
                //     u_r:    record.flags & 0o0400 > 0,
                //     u_w:    record.flags & 0o0200 > 0,
                //     u_x:    record.flags & 0o0100 > 0,
                //     g_r:    record.flags & 0o0040 > 0,
                //     g_w:    record.flags & 0o0020 > 0,
                //     g_x:    record.flags & 0o0010 > 0,
                //     o_r:    record.flags & 0o0004 > 0,
                //     o_w:    record.flags & 0o0002 > 0,
                //     o_x:    record.flags & 0o0001 > 0,
                //     sticky: record.flags & 0o1000 > 0,
                //     setgid: record.flags & 0o2000 > 0,
                //     setuid: record.flags & 0o4000 > 0,
                // }),
                create_time: record.create_time.known().map(|t| t.into()),
                modify_time: record.modify_time.known().map(|t| t.into()),
                access_time: record.access_time.known().map(|t| t.into()),
                change_time: record.change_time.known().map(|t| t.into()),
                delete_time: record.delete_time.known().map(|t| t.into()), // Not currently supported.
                // size: if obj_type != OBJ_TYPE_FOLDER { record.known_size() } else { None },
                // entries: if obj_type == OBJ_TYPE_FOLDER { record.known_size() } else { None },
                // storage_tier_id: record.storage_tier as u32, // Not supported in this driver.
                ..Default::default()
            };
            entries.push(ListEntry {
                relative_name: friendly_file_name.to_owned(),
                attrs: Some(attrs),
                ..Default::default()
            });
        }
        Ok(tonic::Response::new(ListResult {
            entries,
            ..Default::default()
        }))
    }

    async fn r#move(
        &self,
        request: tonic::Request<MoveArg>,
    ) -> std::result::Result<tonic::Response<MoveResult>, tonic::Status> {
        // https://docs.rs/tokio/latest/tokio/fs/fn.rename.html
        unimplemented!()
    }

    async fn copy(
        &self,
        request: tonic::Request<CopyArg>,
    ) -> std::result::Result<tonic::Response<CopyResult>, tonic::Status> {
        // https://docs.rs/tokio/latest/tokio/fs/fn.copy.html
        unimplemented!()
    }

    async fn list_incomplete_uploads(
        &self,
        request: tonic::Request<ListIncompleteUploadsArg>,
    ) -> std::result::Result<tonic::Response<ListIncompleteUploadsResult>, tonic::Status> {
        unimplemented!()
    }

    async fn get_presigned_download(
        &self,
        request: tonic::Request<GetPresignedDownloadArg>,
    ) -> std::result::Result<tonic::Response<GetPresignedDownloadResult>, tonic::Status> {
        unimplemented!()
    }

    async fn get_presigned_upload(
        &self,
        request: tonic::Request<GetPresignedUploadArg>,
    ) -> std::result::Result<tonic::Response<GetPresignedUploadResult>, tonic::Status> {
        unimplemented!()
    }

    async fn watch_once(
        &self,
        request: tonic::Request<WatchOnceArg>,
    ) -> std::result::Result<tonic::Response<WatchOnceResult>, tonic::Status> {
        unimplemented!()
    }

    async fn get_attributes(
        &self,
        request: tonic::Request<GetAttributesArg>,
    ) -> std::result::Result<tonic::Response<GetAttributesResult>, tonic::Status> {
        // https://doc.rust-lang.org/nightly/std/fs/struct.Metadata.html
        // https://doc.rust-lang.org/nightly/std/fs/fn.symlink_metadata.html
        unimplemented!()
    }

    async fn set_attributes(
        &self,
        request: tonic::Request<SetAttributesArg>,
    ) -> std::result::Result<tonic::Response<SetAttributesResult>, tonic::Status> {
        // https://doc.rust-lang.org/nightly/std/fs/fn.set_permissions.html
        // https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.set_modified
        // https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.set_times
        // https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.set_permissions
        unimplemented!()
    }

    async fn delete_many(
        &self,
        request: tonic::Request<DeleteManyArg>,
    ) -> std::result::Result<tonic::Response<DeleteManyResult>, tonic::Status> {
        unimplemented!()
    }

    async fn get_service_info(
        &self,
        request: tonic::Request<GetServiceInfoArg>,
    ) -> std::result::Result<tonic::Response<GetServiceInfoResult>, tonic::Status> {
        unimplemented!()
    }

    async fn get_audit_trail(
        &self,
        request: tonic::Request<GetAuditTrailArg>,
    ) -> std::result::Result<tonic::Response<GetAuditTrailResult>, tonic::Status> {
        unimplemented!()
    }

    async fn start_transaction(
        &self,
        request: tonic::Request<StartTransactionArg>,
    ) -> std::result::Result<tonic::Response<StartTransactionResult>, tonic::Status> {
        unimplemented!()
    }

    async fn commit_transaction(
        &self,
        request: tonic::Request<CommitTransactionArg>,
    ) -> std::result::Result<tonic::Response<CommitTransactionResult>, tonic::Status> {
        unimplemented!()
    }

    async fn abort_transaction(
        &self,
        request: tonic::Request<AbortTransactionArg>,
    ) -> std::result::Result<tonic::Response<AbortTransactionResult>, tonic::Status> {
        unimplemented!()
    }

    /* If no explicit version is supplied, this gets linked to the folder above
    `latest` */
    async fn create_link(
        &self,
        request: tonic::Request<CreateLinkArg>,
    ) -> std::result::Result<tonic::Response<CreateLinkResult>, tonic::Status> {
        unimplemented!()
    }

    async fn unlink(
        &self,
        request: tonic::Request<UnlinkArg>,
    ) -> std::result::Result<tonic::Response<UnlinkResult>, tonic::Status> {
        unimplemented!()
    }
}
