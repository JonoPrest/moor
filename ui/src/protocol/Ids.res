// Identifiers and scalars (moor-protocol `ids.rs`, `version.rs`). All
// serde-transparent on the wire: ULIDs and OIDs are strings, counters are
// numbers. u64 counters may exceed 2^31, so they are floats here.

type workspaceId = string
type repoId = string
type reviewId = string
type commentId = string
type threadId = string
type clientId = string

type blobOid = string
type commitOid = string
type treeOid = string

type seq = float
type clientSeq = float
type requestId = float
/// Milliseconds since the Unix epoch.
type timestamp = float

type protocolVersion = string
type schemaVersion = int

let ulid = S.string->S.stringLength(26)
let oid = S.string->S.stringLength(40)

let workspaceId: S.t<workspaceId> = ulid
let repoId: S.t<repoId> = ulid
let reviewId: S.t<reviewId> = ulid
let commentId: S.t<commentId> = ulid
let threadId: S.t<threadId> = ulid
let clientId: S.t<clientId> = ulid

let blobOid: S.t<blobOid> = oid
let commitOid: S.t<commitOid> = oid
let treeOid: S.t<treeOid> = oid

let seq: S.t<seq> = S.float
let clientSeq: S.t<clientSeq> = S.float
let requestId: S.t<requestId> = S.float
let timestamp: S.t<timestamp> = S.float

let protocolVersion: S.t<protocolVersion> = S.string
let schemaVersion: S.t<schemaVersion> = S.int
