use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Error,
    Status,
    System,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Message {
    payload: Vec<u8>,
    channel: Vec<u8>,
    message_type: MessageType,
}

pub fn status_msg(msg: &str) -> Message {
    Message { 
        payload: msg.into(),
        channel: String::from("SYSTEM").into_bytes(),
        message_type: MessageType::System,
    }
}
