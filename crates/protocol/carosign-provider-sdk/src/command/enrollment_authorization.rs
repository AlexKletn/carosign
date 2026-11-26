/// References to confirmation of registration; the authority itself is checked by the owner.
/// The Provider is obliged to compare this type with its announced opportunity: the absence
/// of links does not allow registration with a confirmation call.
#[derive(Clone, PartialEq)]
pub enum EnrollmentAuthorization {
    None,
    Challenged {
        reference: Vec<u8>,
        challenge_digest: [u8; 32],
    },
}
