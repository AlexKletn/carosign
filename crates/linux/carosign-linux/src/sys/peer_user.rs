/// Who's at the other end of the line.
///
/// ⚠️ Only **identity of connection**: no right to it (document 08). `SO_PEERCRED`As the
/// kernel removed them when connected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PeerUser {
    /// PID defiant.
    pub pid: u32,
    pub uid: u32,
    pub gid: u32,
}
