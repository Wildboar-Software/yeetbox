pub mod simple;
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
use crate::authn::Session;

type WatchManyStream = tonic::codec::Streaming<FileSystemEvent>;

#[tonic::async_trait]
pub trait Authorizer {

    async fn is_authz_watch_many(
        &self,
        session: &Session,
        request: &tonic::Request<WatchManyArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_make_directory(
        &self,
        session: &Session,
        request: &tonic::Request<MakeDirectoryArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_upload(
        &self,
        session: &Session,
        request: &tonic::Request<UploadArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_append(
        &self,
        session: &Session,
        request: &tonic::Request<AppendArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_patch(
        &self,
        session: &Session,
        request: &tonic::Request<PatchArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_download(
        &self,
        session: &Session,
        request: &tonic::Request<DownloadArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_delete(
        &self,
        session: &Session,
        request: &tonic::Request<DeleteArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_list(
        &self,
        session: &Session,
        request: &tonic::Request<ListArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_move(
        &self,
        session: &Session,
        request: &tonic::Request<MoveArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_copy(
        &self,
        session: &Session,
        request: &tonic::Request<CopyArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_list_incomplete_uploads(
        &self,
        session: &Session,
        request: &tonic::Request<ListIncompleteUploadsArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_get_presigned_download(
        &self,
        session: &Session,
        request: &tonic::Request<GetPresignedDownloadArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_get_presigned_upload(
        &self,
        session: &Session,
        request: &tonic::Request<GetPresignedUploadArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_watch_once(
        &self,
        session: &Session,
        request: &tonic::Request<WatchOnceArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_get_attributes(
        &self,
        session: &Session,
        request: &tonic::Request<GetAttributesArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_set_attributes(
        &self,
        session: &Session,
        request: &tonic::Request<SetAttributesArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_delete_many(
        &self,
        session: &Session,
        request: &tonic::Request<DeleteManyArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_get_service_info(
        &self,
        session: &Session,
        request: &tonic::Request<GetServiceInfoArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_get_audit_trail(
        &self,
        session: &Session,
        request: &tonic::Request<GetAuditTrailArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_start_transaction(
        &self,
        session: &Session,
        request: &tonic::Request<StartTransactionArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_commit_transaction(
        &self,
        session: &Session,
        request: &tonic::Request<CommitTransactionArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_abort_transaction(
        &self,
        session: &Session,
        request: &tonic::Request<AbortTransactionArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_create_link(
        &self,
        session: &Session,
        request: &tonic::Request<CreateLinkArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

    async fn is_authz_unlink(
        &self,
        session: &Session,
        request: &tonic::Request<UnlinkArg>,
    ) -> std::io::Result<bool> {
        unimplemented!()
    }

}
