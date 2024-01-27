use crate::grpc::remotefs::{
    AbortTransactionArg, AbortTransactionResult, AppendArg, AppendResult, AuthenticateArg, AuthenticateResult, CommitTransactionArg, CommitTransactionResult, CopyArg, CopyResult, CreateLinkArg, CreateLinkResult, DeleteArg, DeleteManyArg, DeleteManyResult, DeleteResult, DownloadArg, DownloadResult, FileSystemEvent, FileVersion, GetAttributesArg, GetAttributesResult, GetAuditTrailArg, GetAuditTrailResult, GetAvailableSaslMechanismsResult, GetPresignedDownloadArg, GetPresignedDownloadResult, GetPresignedUploadArg, GetPresignedUploadResult, GetServiceInfoArg, GetServiceInfoResult, ListArg, ListIncompleteUploadsArg, ListIncompleteUploadsResult, ListResult, MakeDirectoryArg, MakeDirectoryResult, MoveArg, MoveResult, PatchArg, PatchResult, SetAttributesArg, SetAttributesResult, StartTransactionArg, StartTransactionResult, UnlinkArg, UnlinkResult, UploadArg, UploadResult, WatchManyArg, WatchOnceArg, WatchOnceResult
};
use crate::storage::Storage;
use std::cmp::min;
use std::io::Seek;
use std::path::PathBuf;
use tokio::fs::{create_dir, create_dir_all, read_link, rename, symlink, try_exists, File, OpenOptions, read};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, AsyncReadExt};
use ulid::Ulid;

const HEAD_FILE_NAME: &str = "_head";
const BLOBS_DIR_NAME: &str = "_blobs";
const LATEST_FILE_NAME: &str = "_latest";
const VERSIONS_DIR_NAME: &str = "_vers";
const MAJOR_VERSION_1: &str = "000000000001";
const MINOR_VERSION_0: &str = "000000000000";

/// This is to prevent a malicious client from sending a huge length and
/// filling up the server's memory.
/// This used to be set to 8_000_000, but I had to reduce it to 1MB because of
/// limits that Tonic puts on message decoding sizes.
const MAX_READ_SIZE: usize = 8 * 1024 * 1024;

fn strs_to_path (strs: &[Vec<u8>]) -> PathBuf {
    let mut path = PathBuf::new();
    for s in strs {
        // FIXME: Don't just trust this input.
        path.push(std::str::from_utf8(&s).unwrap());
    }
    path
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileStorage {
    pub path: std::path::PathBuf,
}

impl FileStorage {

    pub fn new () -> Self {
        FileStorage{
            path: std::path::PathBuf::from("/tmp/yeetbox"),
        }
    }

}

type WatchManyStream = tonic::codec::Streaming<FileSystemEvent>;

#[tonic::async_trait]
impl Storage for FileStorage {

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
        let relpath = req.target.unwrap().path;
        let fullpath = strs_to_path(&relpath);
        let path = self.path.join(fullpath);
        // TODO: I think this is unnecessary.
        // if try_exists(&path).await? {
        //     return Err(tonic::Status::already_exists("target already exists"));
        // }
        create_dir_all(path).await?;
        Ok(tonic::Response::new(MakeDirectoryResult{
            ..Default::default()
        }))
    }

    /*
    The process of creating a new version will be like this:
    - Just save the bytes to a randomly-named file until the upload is complete.
    - Resolve the `latest` symlink to the newest version, say 2.3.
    - Try to create a folder at 00000003/.
    - If that fails, it may be because another process already reserved that
      version. Keep incrementing the major version until we find one that works.
    - You now have your new version.
    - Set the symlink to that version.
    - Check if a newer version exists. If so, re-link to that version instead.
      (This avoids race conditions where latest is set to an old version.)

     */
    async fn upload(
        &self,
        request: tonic::Request<UploadArg>,
    ) -> std::result::Result<tonic::Response<UploadResult>, tonic::Status> {
        let req = request.into_inner();
        if req.target.is_none() {
            return Err(tonic::Status::invalid_argument("target is required"));
        }
        let file_id = req.target.unwrap();
        let relpath = file_id.path;
        let mut blob_path = self.path.join(strs_to_path(&relpath)); // FIXME: Uncomment after testing.
        // let mut blob_path = strs_to_path(&relpath);
        blob_path.push(BLOBS_DIR_NAME);
        create_dir_all(&blob_path).await?;

        // Determine or generate the name of the blob file, which is a ULID + ".blob".
        let ulid = if req.continuation.len() == 0 {
            // If the client did not supply a continuation token, this is a new
            // upload, so we create a new blob.
            Ulid::new()
        } else {
            // Otherwise, we use the continuation token as the blob name.
            let a: [u8; 16] = req.continuation[..].try_into()
                .map_err(|_| tonic::Status::invalid_argument("invalid continuation token"))?;
            Ulid::try_from(a)
                .map_err(|_| tonic::Status::invalid_argument("invalid continuation token"))?
        };

        let blob_file_name = format!("{}.blob", ulid);
        blob_path.push(blob_file_name);
        let mut f = OpenOptions::new()
            .append(true) // We have to seek to the end to append.
            .create(true)
            .open(&blob_path).await?;
        // TODO: https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.set_len
        // TODO: https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.sync_all
        f.write(&req.data).await?;
        drop(f);

        if req.incomplete {
            let ulid_bytes = ulid.to_bytes();
            return Ok(tonic::Response::new(UploadResult{
                continuation: ulid_bytes.to_vec(),
                ..Default::default()
            }));
        }

        let mut head_path = blob_path.clone();
        head_path.pop();
        head_path.pop();
        let mut serial_path = head_path.clone(); // There doesn't seem to be a way to only clone from [0..-2]
        head_path.push(HEAD_FILE_NAME);

        if !req.next {
            serial_path.push(MAJOR_VERSION_1);
            create_dir_all(&serial_path).await?; // TODO: More efficient version.
            serial_path.push(MINOR_VERSION_0);
            rename(&blob_path, &serial_path).await?;
            symlink(&serial_path, &head_path).await?;
            // TODO: Check if there is a newer symlink to it.
            return Ok(tonic::Response::new(UploadResult{
                ..Default::default()
            }));
        }

        // If no such symlink, just create the file.
        let latest_real_path = match read_link(&head_path).await {
            Ok(rp) => rp,
            Err(e) => {
                // If the symlink is not found, we assume that the file was
                // never created (IOW, never even had a version 1.0.)
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(e.into());
                }
                serial_path.push(MAJOR_VERSION_1);
                create_dir_all(&serial_path).await?; // TODO: More efficient version.
                serial_path.push(MINOR_VERSION_0);
                rename(&blob_path, &serial_path).await?;
                symlink(&serial_path, &head_path).await?;
                // TODO: Check if there is a newer symlink to it.
                return Ok(tonic::Response::new(UploadResult{
                    ..Default::default()
                }));
            },
        };
        let mut latest_real_path_components = latest_real_path.components().rev();
        // TODO: Handle missing latest error
        let _ = latest_real_path_components.next();
        let latest_major_str = latest_real_path_components.next();
        if latest_major_str.is_none() {
            return Err(tonic::Status::internal(".head symlink is invalid"));
        }
        let mut latest_major = latest_major_str.unwrap()
            .as_os_str()
            .to_str()
            .ok_or_else(|| tonic::Status::internal("major file name invalid"))?
            .parse::<u64>()
            .map_err(|_| tonic::Status::internal("major file name invalid"))?;

        let mut inc_tries = 1;
        let mut serial_path = head_path.clone();
        while inc_tries <= 1000 {
            inc_tries += 1;
            latest_major += 1;
            serial_path.pop();
            serial_path.push(format!("{:012}", latest_major));
            match create_dir(&serial_path).await {
                Ok(_) => break,
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(e.into());
                    }
                },
            };
        }
        if inc_tries == 1000 {
            return Err(tonic::Status::internal("failed to create new version"));
        }
        serial_path.push(format!("{:012}", 0));
        rename(&blob_path, &serial_path).await?;
        symlink(&serial_path, &head_path).await?;
        // TODO: Check if there is a newer symlink to it.
        return Ok(tonic::Response::new(UploadResult{
            ..Default::default()
        }));
        // TODO: Use req.reserve to reserve space for the file.
    }

    async fn append(
        &self,
        request: tonic::Request<AppendArg>,
    ) -> std::result::Result<tonic::Response<AppendResult>, tonic::Status> {
        unimplemented!()
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
        // TODO: Use the sendfile system call on Linux and Mac to pipe the file directly to the socket. (TransmitFile on Windows)
        let req = request.into_inner();
        if req.target.is_none() {
            return Err(tonic::Status::invalid_argument("target is required"));
        }
        let file_id = req.target.unwrap();
        let relpath = file_id.path;
        let mut path = self.path.join(strs_to_path(&relpath));
        if let Some(version) = file_id.version {
            path.push(VERSIONS_DIR_NAME);
            path.push(format!("{:012}", version.major));
            if let Some(minor_version) = version.minor {
                path.push(format!("{:012}", minor_version));
            } else {
                path.push(LATEST_FILE_NAME);
            }
        } else {
            path.push(HEAD_FILE_NAME);
        }
        let mut f = File::open(path).await?;
        if req.offset > 0 {
            f.seek(std::io::SeekFrom::Start(req.offset as u64)).await?;
        }
        let alloc_size: usize = if req.length > 0 {
            min(req.length as usize, MAX_READ_SIZE)
        } else {
            MAX_READ_SIZE
        };
        let mut data = Vec::with_capacity(alloc_size);
        // We have to set the length to the capacity because the read function
        // uses the length to determine how many bytes to read, not the capacity.
        unsafe { data.set_len(alloc_size); }
        let bytes_read = f.read(&mut data).await?;
        // Then we truncate it to the actual number of bytes read.
        data.truncate(bytes_read);
        Ok(tonic::Response::new(DownloadResult{
            data,
            more: bytes_read == alloc_size,
            ..Default::default()
        }))
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteArg>,
    ) -> std::result::Result<tonic::Response<DeleteResult>, tonic::Status> {
        // remove_dir_all
        unimplemented!()
    }

    async fn list(
        &self,
        request: tonic::Request<ListArg>,
    ) -> std::result::Result<tonic::Response<ListResult>, tonic::Status> {
        // https://doc.rust-lang.org/nightly/std/fs/fn.read_dir.html
        unimplemented!()
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
