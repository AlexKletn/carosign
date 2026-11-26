//! Large call body: description in the frame, bytes in a separate descriptor.
//!
//! ⚠️ Rule 32: the mediator frame remains 64 KB, the large body lives in one sealed
//! `memfd`and only a description in the frame (a description)`ExternalBody`No slicing into
//! multiple frames and no “slipping in” `payload`».
//!
//! ⚠️ The description is checked **fully**: offset, length, file size and SHA-256. This is a
//! consistency check, not trust: the accepted body still passes the typed handler analysis.
//!
//! ⚠️ Not here. `libc`: sealing and `fstat` live `carosign-linux`.
//! This file only knows the *rules*.

mod body;
mod body_error;

pub use body::BODY_MAX;
pub use body::ExternalBody;
pub use body::PAYLOAD_MAX;
pub use body::check_exclusive;
pub use body::check_fd_count;
pub use body_error::BodyError;
