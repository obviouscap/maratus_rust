use bson::DateTime as BsonDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ___ participants collection ___
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ParticipantType {
    Human,
    Ai,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Participant {
    #[serde(rename = "_id")]
    pub id: String,
    pub address: String,
    pub display_name: Option<String>,
    #[serde(rename = "type")]
    pub participant_type: ParticipantType,
    pub description: Option<String>,
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
    pub summary: Option<String>,
    pub context: Option<String>,
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
    pub summary: Option<String>,
    pub context: Option<String>,
}

// ___ summaries collection (for storing summarized message ranges) ___
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageSummary {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub message_ids: Vec<Uuid>,
    pub summary: String,
    pub context: Option<String>,
    pub created_at: BsonDateTime,
    pub from_date: BsonDateTime,
    pub to_date: BsonDateTime,
}