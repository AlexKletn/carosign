/// How does a permitted descriptor behave in exec?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExecDisposition {
    /// Leave it open after exec.
    Inherit,
    /// Close the core with a successful exec.
    Close,
}
