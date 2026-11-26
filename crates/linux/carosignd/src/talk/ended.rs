use super::fmt;

/// How the connection ended.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ended {
    /// The client shut down the connection.
    Closed,
    /// Protocol failure: connection closed, response (if any) sent.
    Refused(String),
    /// The connection broke on the shipment.
    Broken(i32),
}

impl fmt::Display for Ended {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("клиент закрыл соединение"),
            Self::Refused(reason) => write!(f, "отказ: {reason}"),
            Self::Broken(errno) => write!(f, "обрыв на отправке: {errno}"),
        }
    }
}
