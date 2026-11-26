use super::{
    ClientFrame, Connection, ConversationEnd, Message, Payload, check_client_frame,
    check_frame_size, version,
};

/// Responds to the first message.
///
/// ⚠️ The first message has to be a greeting, otherwise it's a rejection, and it's not a
/// trifle, and bypassing version approval would open up the reception of commands with a
/// different meaning of the fields.
pub fn greet(connection: &mut Connection, frame: &[u8]) -> Result<Vec<u8>, ConversationEnd> {
    check_frame_size(frame)?;
    let request = ClientFrame::decode(frame)
        .map_err(|_| ConversationEnd::Refused("кадр не разбирается".to_string()))?;
    check_client_frame(&request).map_err(|error| ConversationEnd::Refused(error.to_string()))?;

    let Some(Payload::Hello(hello)) = &request.payload else {
        // ⚠️ We check with the session: he knows that the greeting has not yet taken place.
        let _ = connection.session.accept(frame);
        return Err(ConversationEnd::Refused(
            "первое сообщение должно быть приветствием".to_string(),
        ));
    };

    version::negotiate(hello.major, hello.min_minor, hello.max_minor)
        .map_err(|error| ConversationEnd::Refused(error.to_string()))?;

    // ⚠️ The session takes the shot itself: one phase and version check, not two.
    connection
        .session
        .accept(frame)
        .map_err(|error| ConversationEnd::Refused(error.to_string()))?;

    connection
        .session
        .hello_ack()
        .ok_or_else(|| ConversationEnd::Refused("version unavailable".into()))
}
