use remotefs::file_system_service_client::FileSystemServiceClient;
use remotefs::{
    MakeDirectoryArg,
    UploadArg,
    DownloadArg,
    ListArg,
    AppendArg,
    DeleteArg
};

pub mod remotefs {
    tonic::include_proto!("remotefs");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = FileSystemServiceClient::connect("http://127.0.0.1:50051").await?
        .max_decoding_message_size(8 * 1024 * 1024);

    let request = tonic::Request::new(MakeDirectoryArg {
        gid: 100,
        uid: 100,
        perms: None,
        target: Some(remotefs::FileId{
            path: vec![String::from("foo")],
            version: None,
        }),
    });

    let response = client.make_directory(request).await?;
    println!("RESPONSE={:?}", response);

    let request1 = tonic::Request::new(UploadArg {
        gid: 100,
        uid: 100,
        perms: None,
        target: Some(remotefs::FileId{
            path: vec![
                String::from("foo"),
                String::from("bar.txt"),
            ],
            version: None,
        }),
        data: Vec::from("Hello, world!".as_bytes()),
        incomplete: false,
        next: false,
        ..Default::default()
    });
    let response = client.upload(request1).await?;
    println!("RESPONSE={:?}", response);

    // let request2 = tonic::Request::new(UploadArg {
    //     gid: 100,
    //     uid: 100,
    //     perms: None,
    //     target: Some(remotefs::FileId{
    //         path: vec![
    //             Vec::from("foo"),
    //             Vec::from("bar.txt"),
    //         ],
    //         version: None,
    //     }),
    //     data: Vec::from("\nJust kidding. BYE.".as_bytes()),
    //     continuation: response.get_ref().continuation.clone(),
    //     next: true,
    //     ..Default::default()
    // });
    // let response = client.upload(request2).await?;
    // println!("RESPONSE={:?}", response);

    // tonic::client::Grpc::max_decoding_message_size(self, 8 * 1024 * 1024);

    let request3 = tonic::Request::new(DownloadArg {
        target: Some(remotefs::RequestedFileId {
            path: vec![
                String::from("foo"),
                String::from("bar.txt"),
            ],
            version: None,
        }),
        length: 5,
        offset: 3,
        ..Default::default()
    });

    let response = client.download(request3).await?;
    println!("RESPONSE={:?}", response);

    // let t = std::str::from_utf8(response.get_ref().data.as_slice()).unwrap();
    // println!("{}", t);

    let request4 = tonic::Request::new(ListArg {
        target: Some(remotefs::RequestedFileId {
            path: vec![
                String::from("foo"),
            ],
            version: None,
        }),
        attrs: true,
        ..Default::default()
    });

    let response = client.list(request4).await?;
    println!("RESPONSE={:?}", response);

    let request5 = tonic::Request::new(AppendArg {
        target: Some(remotefs::RequestedFileId {
            path: vec![
                String::from("foo"),
                String::from("bar.txt"),
            ],
            version: None,
        }),
        data: Vec::from(" I am going to bed."),
        ..Default::default()
    });

    let response = client.append(request5).await?;
    println!("RESPONSE={:?}", response);

    let request6 = tonic::Request::new(DeleteArg {
        target: Some(remotefs::RequestedFileId {
            path: vec![
                String::from("foo"),
                String::from("bar.txt"),
            ],
            version: None,
        }),
        ..Default::default()
    });

    let response = client.delete(request6).await?;
    println!("RESPONSE={:?}", response);

    Ok(())
}
