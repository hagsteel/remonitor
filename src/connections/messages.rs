use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Error,
    Status
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Message {
    payload: String,
    message_type: MessageType,
}

pub fn status_msg(msg: &str) -> Message {
    Message { 
        payload: msg.to_owned(),
        message_type: MessageType::Status,
    }
}


