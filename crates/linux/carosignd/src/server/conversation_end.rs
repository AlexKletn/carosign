/// How did the greeting end?
///
/// ⚠️ The agreed-upon greeting here is no: to be on the list of “what didn’t end” is to say
/// “all is well” where everything is bad.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConversationEnd {
    /// The client left without saying hello.
    ClosedEarly,
    /// Protocol failure.
    Refused(String),
}
