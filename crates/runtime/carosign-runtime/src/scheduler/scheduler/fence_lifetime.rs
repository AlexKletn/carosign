pub(super) struct FenceLifetime {
    pub(super) _owner: Box<dyn Send + Sync>,
}

impl core::fmt::Debug for FenceLifetime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FenceLifetime(..)")
    }
}
