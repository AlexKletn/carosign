use crate::codec::ClientRequest;

pub fn allowed(before_hello: bool, request: &ClientRequest) -> bool {
    match (before_hello, request) {
        (true, ClientRequest::Hello { .. }) => true,
        (true, _) => false,
        (false, ClientRequest::Hello { .. }) => false,
        (false, _) => true,
    }
}

#[path = "dispatch.tests.rs"]
#[cfg(test)]
mod tests;
