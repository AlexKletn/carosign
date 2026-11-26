/// Announce a separate 16-byte identifier.
///
/// Macros is because there are as many bodies as there are names, and each one has to be the
/// same byte byte, and manual copying would be a source of discrepancy.
macro_rules! opaque_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        ///
        /// 16 bytes, zero is rejected. You can only create it by using
        /// a`from_bytes`](Self::from_bytes) or through`IdSource`](super::IdSource).
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; $crate::ids::ID_LEN]);

        impl $name {
            /// Accepts canonical bytes, rejects zero.
            pub fn from_bytes(bytes: [u8; $crate::ids::ID_LEN]) -> Result<Self, $crate::ids::InvalidId> {
                if bytes == [0u8; $crate::ids::ID_LEN] {
                    return Err($crate::ids::InvalidId);
                }
                Ok(Self(bytes))
            }

            /// Canonical bytes as they are.
            pub fn as_bytes(&self) -> &[u8; $crate::ids::ID_LEN] {
                &self.0
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(concat!(stringify!($name), "(..)"))
            }
        }

        impl TryFrom<&[u8]> for $name {
            type Error = $crate::ids::InvalidId;

            fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
                let bytes: [u8; $crate::ids::ID_LEN] = value.try_into().map_err(|_| $crate::ids::InvalidId)?;
                Self::from_bytes(bytes)
            }
        }
    };
}

pub(super) use opaque_id;
