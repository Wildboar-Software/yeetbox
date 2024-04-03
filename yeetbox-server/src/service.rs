use crate::grpc::remotefs::{
    AbortTransactionArg, AbortTransactionResult, AppendArg, AppendResult,
    CommitTransactionArg, CommitTransactionResult, CopyArg, CopyResult, CreateLinkArg,
    CreateLinkResult, DeleteArg, DeleteManyArg, DeleteManyResult, DeleteResult, DownloadArg,
    DownloadResult, FileSystemEvent, GetAttributesArg, GetAttributesResult, GetAuditTrailArg,
    GetAuditTrailResult, GetPresignedDownloadArg, GetPresignedDownloadResult,
    GetPresignedUploadArg, GetPresignedUploadResult, GetServiceInfoArg, GetServiceInfoResult,
    ListArg, ListIncompleteUploadsArg, ListIncompleteUploadsResult, ListResult, MakeDirectoryArg,
    MakeDirectoryResult, MoveArg, MoveResult, PatchArg, PatchResult, SetAttributesArg,
    SetAttributesResult, StartTransactionArg, StartTransactionResult, UnlinkArg, UnlinkResult,
    UploadArg, UploadResult, WatchManyArg, WatchOnceArg, WatchOnceResult,
    GetAvailableSaslMechanismsResult, AuthenticateArg, AuthenticateResult,
};
use crate::{FileSystemServiceProvider, FileSystemService, Storage};

#[tonic::async_trait]
impl FileSystemService for FileSystemServiceProvider {
    type WatchManyStream = tonic::codec::Streaming<FileSystemEvent>;

    async fn get_available_sasl_mechanisms(
        &self,
        request: tonic::Request<()>,
    ) -> std::result::Result<
        tonic::Response<GetAvailableSaslMechanismsResult>,
        tonic::Status,
    > {
        unimplemented!()
    }

    async fn authenticate(
        &self,
        request: tonic::Request<AuthenticateArg>,
    ) -> std::result::Result<
        tonic::Response<AuthenticateResult>,
        tonic::Status,
    > {
        unimplemented!()
    }

    async fn watch_many(
        &self,
        request: tonic::Request<WatchManyArg>,
    ) -> std::result::Result<tonic::Response<Self::WatchManyStream>, tonic::Status> {
        unimplemented!()
    }

    async fn make_directory(
        &self,
        request: tonic::Request<MakeDirectoryArg>,
    ) -> std::result::Result<tonic::Response<MakeDirectoryResult>, tonic::Status> {
        let store = self.storage.lock().await;
        store.make_directory(request).await
    }

    async fn upload(
        &self,
        request: tonic::Request<UploadArg>,
    ) -> std::result::Result<tonic::Response<UploadResult>, tonic::Status> {
        let store = self.storage.lock().await;
        store.upload(request).await
    }

    async fn append(
        &self,
        request: tonic::Request<AppendArg>,
    ) -> std::result::Result<tonic::Response<AppendResult>, tonic::Status> {
        let store = self.storage.lock().await;
        store.append(request).await
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
        let store = self.storage.lock().await;
        store.download(request).await
    }

    async fn delete(
        &self,
        request: tonic::Request<DeleteArg>,
    ) -> std::result::Result<tonic::Response<DeleteResult>, tonic::Status> {
        let store = self.storage.lock().await;
        store.delete(request).await
    }

    async fn list(
        &self,
        request: tonic::Request<ListArg>,
    ) -> std::result::Result<tonic::Response<ListResult>, tonic::Status> {
        let store = self.storage.lock().await;
        store.list(request).await
    }

    async fn r#move(
        &self,
        request: tonic::Request<MoveArg>,
    ) -> std::result::Result<tonic::Response<MoveResult>, tonic::Status> {
        let store = self.storage.lock().await;
        store.r#move(request).await
    }

    async fn copy(
        &self,
        request: tonic::Request<CopyArg>,
    ) -> std::result::Result<tonic::Response<CopyResult>, tonic::Status> {
        let store = self.storage.lock().await;
        store.copy(request).await
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
