use super::{AuthenticatorId, ClientRequest};

/// The authenticator number from the request.
pub fn authenticator_of(request: &ClientRequest) -> Option<AuthenticatorId> {
    let bytes = match request {
        ClientRequest::BeginVerification { authenticator, .. } => authenticator,
        ClientRequest::BeginEnrollment { authenticator, .. } => authenticator,
        ClientRequest::BeginRemoval { authenticator, .. } => authenticator,
        ClientRequest::GetAuthenticator { authenticator } => authenticator,
        ClientRequest::ListEnrollments { authenticator, .. } => authenticator,
        _ => return None,
    };
    AuthenticatorId::try_from(&bytes[..]).ok()
}
