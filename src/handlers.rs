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
pub struct IngestPayload {
    pub channel: String,
    pub conversation_ext_id: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub sent_at: chrono::DateTime<Utc>,
    pub content: String,
}

#[post("/ingest")]
pub async fn ingest(
    db: web::Data<Database>,
    payload: web::Json<IngestPayload>,
) -> actix_web::Result<impl Responder> {
    let p = payload.into_inner();
    let part_coll = db.collection::<Participant>("participants");
    let conv_coll = db.collection::<Conversation>("conversations");
    let msg_coll = db.collection::<Message>("messages");

    // 1) upsert participant
    let part = part_coll
        .find_one_and_update(
            doc! { "address": &p.from_address },
            doc! {
              "$setOnInsert": { "address": &p.from_address },
              "$set": { "display_name": &p.from_name }
            },
        )
        .upsert(true)
        .return_document(ReturnDocument::After)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .expect("just inserted or found");

    // 2) upsert conversation by external_id
    let now = BsonDateTime::now();
    let conv = conv_coll
        .find_one_and_update(
            doc! { "external_id": &p.conversation_ext_id },
            doc! {
              "$setOnInsert": {
                "external_id": &p.conversation_ext_id,
                "topic": Bson::Null,
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

    // 3) ensure the participant is in conversation.participants
    // Convert UUID to string for BSON compatibility
    let part_id_str = part.id.to_string();
    let conv_id_str = conv.id.to_string();
    let join_link = doc! {
      "participant_id": &part_id_str,
      "joined_at": BsonDateTime::now()
    };
    
    let _ = conv_coll
        .update_one(
            doc! { 
                "_id": &conv_id_str, 
                "participants.participant_id": { "$ne": &part_id_str } 
            },
            doc! { "$push": { "participants": join_link } },
        )
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // 4) insert the message
    let new_msg = Message {
        id: Uuid::new_v4(),
        conversation_id: conv.id,
        sender_id: part.id,
        channel: p.channel,
        external_id: Some(p.conversation_ext_id),
        // Convert chrono DateTime to BsonDateTime
        sent_at: BsonDateTime::from_millis(p.sent_at.timestamp_millis()),
        content: p.content,
    };
    
    msg_coll
        .insert_one(new_msg)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json("Message inserted successfully"))
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