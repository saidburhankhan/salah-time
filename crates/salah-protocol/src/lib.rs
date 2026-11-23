//! Salah protocol version 1: JSON-RPC 2.0 NDJSON over any byte stream.
//!
//! The crate is transport-independent. [`Dispatcher`] takes a parsed request
//! and a [`Context`] and performs no I/O of its own; `salah rpc` supplies
//! stdin/stdout and `salah serve` a Unix socket.
//!
//! See `docs/work/architecture/protocol-v1.md` for the method table.

pub mod dispatcher;
pub mod error;
mod goals;
pub mod message;
pub mod methods;
mod reads;
mod resources;
mod writes;

pub use dispatcher::{ConfigRepository, Context, Dispatcher};
pub use error::{
    APPLICATION_ERROR_BASE, ErrorData, INTERNAL_ERROR, INVALID_PARAMS, INVALID_REQUEST,
    METHOD_NOT_FOUND, PARSE_ERROR, RpcError, code_for_kind,
};
pub use message::{
    CHANGED_METHOD, CLOCK_BOUNDARY_METHOD, ChangeScope, ChangedNotification, ClockBoundaryNext,
    ClockBoundaryNotification, JSONRPC_VERSION, MAX_LINE_BYTES, Notification, PROTOCOL_VERSION,
    Request, Response, SERVER_NAME,
};
