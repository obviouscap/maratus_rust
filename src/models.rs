use bson::DateTime as BsonDateTime;
use serde::{Deserialize, Serialize};
use bson::Uuid;

// ___ participants collection ___
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Participant {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub address: String,
    pub display_name: Option<String>,
}

// ___ embedded in Conversation.participants ___
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConvParticipant {
    pub participant_id: Uuid,
    pub joined_at: BsonDateTime,
}

// ___ conversations collection ___
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Conversation {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub external_id: String,
    pub topic: Option<String>,
    pub started_at: BsonDateTime,
    pub participants: Vec<ConvParticipant>,
}

// ___ messages collection ___
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub channel: String,
    pub external_id: Option<String>,
    pub sent_at: BsonDateTime,
    pub content: String,
}