use remotefs::file_system_service_client::FileSystemServiceClient;
use remotefs::{
    MakeDirectoryArg,
    UploadArg,
};

pub mod remotefs {
    tonic::include_proto!("remotefs");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = FileSystemServiceClient::connect("http://127.0.0.1:50051").await?;

    // let request = tonic::Request::new(MakeDirectoryArg {
    //     gid: 100,
    //     uid: 100,
    //     perms: None,
    //     target: Some(remotefs::FileId{
    //         path: vec![Vec::from("foo".as_bytes())],
    //         version: None,
    //     }),
    // });

    // let response = client.make_directory(request).await?;

    let request1 = tonic::Request::new(UploadArg {
        gid: 100,
        uid: 100,
        perms: None,
        target: Some(remotefs::FileId{
            path: vec![
                Vec::from("foo"),
                Vec::from("bar.txt"),
            ],
            version: None,
        }),
        data: Vec::from("Hello, world!".as_bytes()),
        incomplete: true,
        next: true,
        ..Default::default()
    });

    let response1 = client.upload(request1).await?;

    println!("RESPONSE={:?}", response1);

    let request2 = tonic::Request::new(UploadArg {
        gid: 100,
        uid: 100,
        perms: None,
        target: Some(remotefs::FileId{
            path: vec![
                Vec::from("foo"),
                Vec::from("bar.txt"),
            ],
            version: None,
        }),
        data: Vec::from("\nJust kidding. BYE.".as_bytes()),
        continuation: response1.get_ref().continuation.clone(),
        next: true,
        ..Default::default()
    });

    let response2 = client.upload(request2).await?;

    println!("RESPONSE={:?}", response2);

    Ok(())
}
