# Remote File System Protocol

**Work in progress**

This is a gRPC-based remote file system protocol. This is not exactly meant to
be a replacement for protocols like SMB, NFS, or iSCSI. Instead, it is instended
to be more of an application-layer object storage like AWS S3 or Minio, but with
the following features in mind:

- The API is specified in gRPC / Protocol Buffers, meaning that it is trivial to
  generate clients in all languages for it.
- Symbolic links
- Versioning
- Transactions
- Storage Tiers
- Extensible authentication via SASL
- Multiple different authorization schemes, including Google's Zanzibar and XACML.
- Configuration via scripting: instead of static variables, you can write
  functions (probably in Rhai (TBD)) to automate the selection of storage tiers,
  make access control decisions, forbid certain file names, etc.
- (Possibly) integrated virus scanning: scan for viruses in files before they
  are ever written to disk / available to download
- Labelling files with attributes. This is important for things like image
  recognition. As I recall, with AWS S3, you can only supply up to 10 attributes
  for an object. This will not be the case here!
- Events: receive file system events in a message queue (most likely ZeroMQ (TBD))
  - There are plans to develop event listeners such as:
    - Virus scanners
    - Text extractors, including speech-to-text extraction from audio and video
    - Image recognition and labeling
    - IETF RFC 3161 Trusted Timestamping
- NFS Server
- SMB/CIFS Server
- WebDAV Server
- RSS Feed of changes
- SNMP Traps and Monitoring
- Prometheus Metrics
- A Web Interface

On top of this, there will be an HTTP / REST API that will attempt to be
compatible with the APIs of AWS, Google Cloud, and Azure for the most important
core operations, such as download and upload.

It may be understood like so: whereas a native file system is more like a
storage primitive, Yeetbox is a storage _application_.

There are plans to develop web, desktop, mobile, and terminal interfaces for
Yeetbox.

## Performance

It is **NOT** a goal of this project to be the fastest file system. If you want
speed, you should probably just use iSCSI or Ceph or something like that. This
protocol / server exists to make a file system easily available to client
applications with a lot of features.

## File Storage

Currently, there are plans for two file storage backends:

### Simple

All files are stored in a hierarchical structure that aligns with the user's
expectations, and named according to user inputs. Versioning is available, but
there are no storage tiers or transactions. This is probably slower, and less
feature rich, but your files get organized and named according to user input,
which makes it easy to find files.

This storage interface is being developed first just because it is the simplest
and gets Yeetbox to the level of a proof-of-concept quicker.

### Database

All files are stored with hashes as names in a flat folder (not technically
true, but close), and a database relates user-supplied names and version numbers
to the blobs as though they were the file names. This will support all of the
features and probably be faster, but the trade-off is that the files are
organized in a less exportable manner.

## To Do

- [ ] Best Practices
  - [ ] Use [Field Masks](https://protobuf.dev/programming-guides/api/#include-field-read-mask)
  - [ ] [Explicit Deadlines](https://protobuf.dev/programming-guides/api/#every-rpc-deadline)
  - [ ] Size Limits: "We recommend a bound in the ballpark of 8 MiB, and 2 GiB is a hard limit at which many proto implementations break."
- [ ] Authentication
  - [x] Simple
  - [ ] SASL
- [ ] Authorization
  - [x] Simple
  - [ ] XACML
  - [ ] Possibly some X.509 PKI / PMI scheme?
  - [ ] X.509 Clearance
  - [ ] X.509 Permissions
  - [ ] X.500 RBAC
  - [ ] Zanzibar / OpenFGA
    - This looks fairly straightforward: https://openfga.dev/docs/getting-started/perform-check
  - [ ] Rhai script
- [ ] Blob Storage Interface
  - [ ] Memory
  - [ ] File System
- [ ] Metadata Storage
  - [ ] Memory
  - [ ] File System (stores a `.metadata` protobuf-encoded file in each folder)
  - [ ] RocksDB (Probably first key-value store, since it is best-documented, stable, well-supported, etc.)
  - [ ] ReDB (Less supported and stable, but written in Rust and looks great)
- [ ] Operations
  - [x] GetAvailableSaslMechanisms
  - [ ] Authenticate
  - [ ] WatchMany
  - [x] MakeDirectory
  - [x] Upload
  - [x] Append
  - [ ] Patch
  - [x] Download
  - [ ] Pop
  - [ ] Delete
  - [x] List
  - [ ] Move
  - [ ] Copy
  - [ ] ListIncompleteUploads
  - [ ] GetPresignedDownload
  - [ ] GetPresignedUpload
  - [ ] WatchOnce
  - [ ] GetAttributes
  - [ ] SetAttributes
  - [ ] DeleteMany
  - [ ] GetServiceInfo
  - [ ] GetAuditTrail
  - [x] ~~StartTransaction~~
  - [x] ~~CommitTransaction~~
  - [x] ~~AbortTransaction~~
  - [ ] CreateLink
  - [ ] Unlink
  - [ ] List Versions
  - [ ] Checkout (Reassign `head` symbolic link to a different version)
  - [ ] Prepend?
  - [ ] Delete Version?
  - [ ] Search
    - [ ] JSON Query
    - [ ] XML Path
    - [ ] Fuzzy Search
    - [ ] Rows and Columns Count
    - [ ] Field Names
  - [ ] Truncate (https://doc.rust-lang.org/nightly/std/fs/struct.File.html#method.set_len)
- [ ] Consolidate Appends / Patches
- [ ] Do not increment version if hash is the same
- [ ] `latest` per major version
- [ ] Events, probably via ZeroMQ
- [ ] Download Folder as a Zip (And signature)
- [ ] Download Encrypted Archive
- [ ] Merkle Tree?
- [ ] Configuration
- [ ] Different Logging Interfaces
  - [ ] Syslog
  - [ ] Journald
  - [ ] Windows Events
- [ ] Web Interface
  - [ ] List
  - [ ] Download
  - [ ] Presigned URL
  - [ ] Delete
  - [ ] Move
  - [ ] Copy
  - [ ] Incomplete Uploads List
  - [ ] Audit Log
  - [ ] Unlink
  - [ ] Link
  - [ ] Service Info
  - [ ] Get Attributes
- [ ] RSS Feed
- [ ] Microstamping
- [ ] Trusted Timestamping
- [ ] Real IP Logging
- [ ] NFS Server
- [ ] FTP Server
- [ ] SMB/CIFS Server
- [ ] WebDAV
- [ ] Prometheus Endpoint
- [ ] SNMP
- [ ] IPFS Backups / Storage
- [ ] On-Demand Format Translation
  - [ ] Get JSON as XML
  - [ ] Get XML as JSON
  - [ ] Get JSON as YAML
  - [ ] Get JSON as TOML
- [ ] CLI
  - [ ] `mkdir`
  - [ ] `link`
  - [ ] `unlink`
  - [ ] `upload`
  - [ ] `download`
  - [ ] `getattrs`
  - [ ] `setattrs`
  - [ ] `list`
  - [ ] `psu`
  - [ ] `psd`
  - [ ] `delete`
  - [ ] `move`
  - [ ] `copy`
  - [ ] `append`
  - [ ] `patch`
  - [ ] `liu`
  - [ ] `delmany`
  - [ ] `svcinfo`
  - [ ] `audit`
  - [ ] `watch` (Covers Watch and WatchMany)
- [ ] Desktop App

## Notes

The authorizer interface is going to have pretty much every method of the FS
API, but take a `session` parameter, but every method will return a `true` or
`false`.

The blob storage interface is going to have pretty much the same API as the FS
API, but every method will also take the `session` parameter and return async
IO result. The same is true for the metadata storage interface.

I am currently blocked on learning about XACML to determine how to implement the
access control.

## Development

On a Debian distro, run:

```bash
sudo apt install -y protobuf-compiler libprotobuf-dev
```

(I got these instructions from [here](https://github.com/hyperium/tonic).)

### Testing

Run the server with:

```bash
cargo run --bin yeetbox-server
```

Run the CLI with:

```bash
cargo run --bin yeetbox-cli
```

Currently, there is no configuration, so the CLI should be able to reach the
hard-coded socket on which the server listens.
