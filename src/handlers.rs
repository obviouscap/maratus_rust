use actix_web::{get, post, web, HttpResponse, Responder};
use bson::{doc, Bson, DateTime as BsonDateTime};
use chrono::Utc;
use futures::TryStreamExt;
use mongodb::{
    options::{FindOneAndUpdateOptions, FindOptions, ReturnDocument, UpdateOptions},
    Database,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{Participant, Conversation, ConvParticipant, Message};

#[derive(Deserialize)]
pub struct CreateParticipantPayload {
    pub address: String,
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateConversationPayload {
    pub external_id: Uuid,
    pub topic: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateMessagePayload {
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub channel: String,
    pub external_id: Option<String>,
    pub sent_at: chrono::DateTime<Utc>,
    pub content: String,
}

#[post("/participants")]
pub async fn create_participant(
    db: web::Data<Database>,
    payload: web::Json<CreateParticipantPayload>,
) -> actix_web::Result<impl Responder> {
    let p = payload.into_inner();
    let part_coll = db.collection::<Participant>("participants");

    // Upsert participant
    let part = part_coll
        .find_one_and_update(
            doc! { "address": &p.address },
            doc! {
              "$setOnInsert": { 
                "_id": bson::Uuid::from_bytes(Uuid::new_v4().into_bytes()),
                "address": &p.address 
              },
              "$set": { "display_name": &p.display_name }
            },
        )
        .upsert(true)
        .return_document(ReturnDocument::After)
        .await
        .map_err(|e| {
            eprintln!("MongoDB error in create_participant: {:?}", e);
            eprintln!("Error details: {}", e);
            actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
        })?
        .ok_or_else(|| {
            eprintln!("Failed to get participant after upsert");
            actix_web::error::ErrorInternalServerError("Failed to create or retrieve participant")
        })?;

    Ok(HttpResponse::Ok().json(part))
}

#[get("/participants")]
pub async fn get_all_participants(
    db: web::Data<Database>,
) -> actix_web::Result<impl Responder> {
    let part_coll = db.collection::<Participant>("participants");

    let mut cursor = part_coll
        .find(doc! {})
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut participants = Vec::new();
    while let Some(p) = cursor
        .try_next()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
    {
        participants.push(p);
    }

    Ok(HttpResponse::Ok().json(participants))
}

#[get("/participants/{id}")]
pub async fn get_participant(
    db: web::Data<Database>,
    path: web::Path<Uuid>,
) -> actix_web::Result<impl Responder> {
    let part_id = path.into_inner();
    let part_coll = db.collection::<Participant>("participants");

    let part_id_str = part_id.to_string();

    let part = part_coll
        .find_one(doc! { "_id": &part_id_str })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Participant not found"))?;

    Ok(HttpResponse::Ok().json(part))
}

#[post("/conversations")]
pub async fn create_conversation(
    db: web::Data<Database>,
    payload: web::Json<CreateConversationPayload>,
) -> actix_web::Result<impl Responder> {
    let p = payload.into_inner();
    let conv_coll = db.collection::<Conversation>("conversations");

    let ext_id_str = p.external_id.to_string();
    let now = BsonDateTime::now();

    // Upsert conversation by external_id
    let conv = conv_coll
        .find_one_and_update(
            doc! { "external_id": &ext_id_str },
            doc! {
              "$setOnInsert": {
                "_id": Uuid::new_v4().to_string(),
                "external_id": &ext_id_str,
                "topic": &p.topic.map(Bson::String).unwrap_or(Bson::Null),
                "started_at": &now,
                "participants": []
              }
            },
        )
        .upsert(true)
        .return_document(ReturnDocument::After)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .expect("just inserted or found");

    Ok(HttpResponse::Ok().json(conv))
}

#[get("/conversations")]
pub async fn get_all_conversations(
    db: web::Data<Database>,
) -> actix_web::Result<impl Responder> {
    let conv_coll = db.collection::<Conversation>("conversations");

    let options = FindOptions::builder()
        .sort(doc! { "started_at": -1 })
        .build();

    let mut cursor = conv_coll
        .find(doc! {})
        .with_options(options)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut conversations = Vec::new();
    while let Some(c) = cursor
        .try_next()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
    {
        conversations.push(c);
    }

    Ok(HttpResponse::Ok().json(conversations))
}

#[get("/conversations/{id}")]
pub async fn get_conversation(
    db: web::Data<Database>,
    path: web::Path<Uuid>,
) -> actix_web::Result<impl Responder> {
    let conv_id = path.into_inner();
    let conv_coll = db.collection::<Conversation>("conversations");
    let part_coll = db.collection::<Participant>("participants");
    let msg_coll = db.collection::<Message>("messages");

    // Convert UUID to string for BSON compatibility
    let conv_id_str = conv_id.to_string();

    // load conversation
    let conv = conv_coll
        .find_one(doc! { "_id": &conv_id_str })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Conversation not found"))?;

    // load participants
    let ids: Vec<String> = conv.participants.iter()
        .map(|cp| cp.participant_id.to_string())
        .collect();

    let id_array = ids.iter().map(|id| Bson::String(id.clone())).collect();

    let mut cursor = part_coll
        .find(doc! { "_id": { "$in": Bson::Array(id_array) } })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut parts = Vec::new();
    while let Some(p) = cursor
        .try_next()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
    {
        parts.push(p);
    }

    // load messages
    let options = FindOptions::builder()
        .sort(doc! { "sent_at": 1 })
        .build();

    let mut mc = msg_coll
        .find(doc! { "conversation_id": &conv_id_str })
        .with_options(options)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut msgs = Vec::new();
    while let Some(m) = mc
        .try_next()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
    {
        msgs.push(m);
    }

    #[derive(Serialize)]
    struct FullConversation {
        conversation: Conversation,
        participants: Vec<Participant>,
        messages: Vec<Message>,
    }

    Ok(HttpResponse::Ok().json(FullConversation {
        conversation: conv,
        participants: parts,
        messages: msgs,
    }))
}

#[post("/messages")]
pub async fn create_message(
    db: web::Data<Database>,
    payload: web::Json<CreateMessagePayload>,
) -> actix_web::Result<impl Responder> {
    let p = payload.into_inner();
    let conv_coll = db.collection::<Conversation>("conversations");
    let part_coll = db.collection::<Participant>("participants");
    let msg_coll = db.collection::<Message>("messages");

    let conv_id_str = p.conversation_id.to_string();
    let sender_id_str = p.sender_id.to_string();

    // Verify conversation exists
    let conv = conv_coll
        .find_one(doc! { "_id": &conv_id_str })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Conversation not found"))?;

    // Verify participant exists
    let part = part_coll
        .find_one(doc! { "_id": &sender_id_str })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Participant not found"))?;

    // Ensure the participant is in conversation.participants
    let join_link = doc! {
      "participant_id": &sender_id_str,
      "joined_at": BsonDateTime::now()
    };

    let _ = conv_coll
        .update_one(
            doc! { 
                "_id": &conv_id_str, 
                "participants.participant_id": { "$ne": &sender_id_str } 
            },
            doc! { "$push": { "participants": join_link } },
        )
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Insert the message
    let new_msg = Message {
        id: bson::Uuid::from_bytes(Uuid::new_v4().into_bytes()),
        conversation_id: bson::Uuid::from_bytes(p.conversation_id.into_bytes()),
        sender_id: bson::Uuid::from_bytes(p.sender_id.into_bytes()),
        channel: p.channel,
        external_id: p.external_id,
        sent_at: BsonDateTime::from_millis(p.sent_at.timestamp_millis()),
        content: p.content,
    };

    msg_coll
        .insert_one(&new_msg)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(new_msg))
}

#[get("/messages")]
pub async fn get_all_messages(
    db: web::Data<Database>,
) -> actix_web::Result<impl Responder> {
    let msg_coll = db.collection::<Message>("messages");

    let options = FindOptions::builder()
        .sort(doc! { "sent_at": -1 })
        .build();

    let mut cursor = msg_coll
        .find(doc! {})
        .with_options(options)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut messages = Vec::new();
    while let Some(m) = cursor
        .try_next()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
    {
        messages.push(m);
    }

    Ok(HttpResponse::Ok().json(messages))
}

#[get("/messages/{id}")]
pub async fn get_message(
    db: web::Data<Database>,
    path: web::Path<Uuid>,
) -> actix_web::Result<impl Responder> {
    let msg_id = path.into_inner();
    let msg_coll = db.collection::<Message>("messages");

    let msg_id_str = msg_id.to_string();

    let msg = msg_coll
        .find_one(doc! { "_id": &msg_id_str })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Message not found"))?;

    Ok(HttpResponse::Ok().json(msg))
}