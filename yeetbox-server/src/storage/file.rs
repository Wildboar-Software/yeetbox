use crate::grpc::remotefs::{
    AbortTransactionArg, AbortTransactionResult, AppendArg, AppendResult, AuthenticateArg, AuthenticateResult, CommitTransactionArg, CommitTransactionResult, CopyArg, CopyResult, CreateLinkArg, CreateLinkResult, DeleteArg, DeleteManyArg, DeleteManyResult, DeleteResult, DownloadArg, DownloadResult, FileSystemEvent, FileVersion, GetAttributesArg, GetAttributesResult, GetAuditTrailArg, GetAuditTrailResult, GetAvailableSaslMechanismsResult, GetPresignedDownloadArg, GetPresignedDownloadResult, GetPresignedUploadArg, GetPresignedUploadResult, GetServiceInfoArg, GetServiceInfoResult, ListArg, ListIncompleteUploadsArg, ListIncompleteUploadsResult, ListResult, MakeDirectoryArg, MakeDirectoryResult, MoveArg, MoveResult, PatchArg, PatchResult, SetAttributesArg, SetAttributesResult, StartTransactionArg, StartTransactionResult, UnlinkArg, UnlinkResult, UploadArg, UploadResult, WatchManyArg, WatchOnceArg, WatchOnceResult
};
use crate::storage::Storage;
use std::io::Seek;
use std::path::PathBuf;
use tokio::fs::{create_dir, create_dir_all, read_link, rename, symlink, try_exists, File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use ulid::Ulid;

fn strs_to_path (strs: &[Vec<u8>]) -> PathBuf {
    let mut path = PathBuf::new();
    for s in strs {
        // FIXME: Don't just trust this input.
        path.push(std::str::from_utf8(&s).unwrap());
    }
    path
}

fn vers_to_file_name (vers: FileVersion) -> String {
    format!("v{:08}.{:08}.blob", vers.major, vers.minor)
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
        // let mut blob_path = self.path.join(strs_to_path(&relpath)); // FIXME: Uncomment after testing.
        let mut blob_path = strs_to_path(&relpath);
        blob_path.push("_blobs");
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
        f.write(&req.data).await?;
        drop(f);

        if req.incomplete {
            let ulid_bytes = ulid.to_bytes();
            return Ok(tonic::Response::new(UploadResult{
                continuation: ulid_bytes.to_vec(),
                ..Default::default()
            }));
        }

        let mut latest_path = blob_path.clone();
        latest_path.pop();
        latest_path.pop();
        let mut serial_path = latest_path.clone(); // There doesn't seem to be a way to only clone from [0..-2]
        latest_path.push("latest");

        if !req.next {
            serial_path.push("000000000001");
            create_dir_all(&serial_path).await?; // TODO: More efficient version.
            serial_path.push("000000000000");
            rename(&blob_path, &serial_path).await?;
            symlink(&serial_path, &latest_path).await?;
            // TODO: Check if there is a newer symlink to it.
            return Ok(tonic::Response::new(UploadResult{
                ..Default::default()
            }));
        }

        // If no such symlink, just create the file.
        let latest_real_path = match read_link(&latest_path).await {
            Ok(rp) => rp,
            Err(e) => {
                // If the symlink is not found, we assume that the file was
                // never created (IOW, never even had a version 1.0.)
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(e.into());
                }
                serial_path.push("000000000001");
                create_dir_all(&serial_path).await?; // TODO: More efficient version.
                serial_path.push("000000000000");
                rename(&blob_path, &serial_path).await?;
                symlink(&serial_path, &latest_path).await?;
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
            return Err(tonic::Status::internal("latest symlink is invalid"));
        }
        let mut latest_major = latest_major_str.unwrap()
            .as_os_str()
            .to_str()
            .ok_or_else(|| tonic::Status::internal("major file name invalid"))?
            .parse::<u64>()
            .map_err(|_| tonic::Status::internal("major file name invalid"))?;

        let mut inc_tries = 1;
        let mut serial_path = latest_path.clone();
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
        symlink(&serial_path, &latest_path).await?;
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
        unimplemented!()
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteArg>,
    ) -> std::result::Result<tonic::Response<DeleteResult>, tonic::Status> {
        unimplemented!()
    }

    async fn list(
        &self,
        request: tonic::Request<ListArg>,
    ) -> std::result::Result<tonic::Response<ListResult>, tonic::Status> {
        unimplemented!()
    }

    async fn r#move(
        &self,
        request: tonic::Request<MoveArg>,
    ) -> std::result::Result<tonic::Response<MoveResult>, tonic::Status> {
        unimplemented!()
    }

    async fn copy(
        &self,
        request: tonic::Request<CopyArg>,
    ) -> std::result::Result<tonic::Response<CopyResult>, tonic::Status> {
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
        unimplemented!()
    }

    async fn set_attributes(
        &self,
        request: tonic::Request<SetAttributesArg>,
    ) -> std::result::Result<tonic::Response<SetAttributesResult>, tonic::Status> {
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
