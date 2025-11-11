use actix_web::{get, post, put, web, HttpResponse, Responder};
use bson::{doc, DateTime as BsonDateTime};
use chrono::Utc;
use futures::TryStreamExt;
use mongodb::{
    options::{FindOptions, ReturnDocument},
    Database,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::models::{Conversation, Participant, Message};

#[derive(Deserialize)]
pub struct CreateMessagePayload {
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub channel: String,
    pub external_id: Option<String>,
    pub sent_at: chrono::DateTime<Utc>,
    pub content: String,
    pub summary: Option<String>,
    pub context: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateMessageMetadataPayload {
    pub summary: Option<String>,
    pub context: Option<String>,
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

    let conv = conv_coll
        .find_one(doc! { "_id": &conv_id_str })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Conversation not found"))?;

    let part = part_coll
        .find_one(doc! { "_id": &sender_id_str })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Participant not found"))?;

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

    let new_msg = Message {
        id: Uuid::new_v4(),
        conversation_id: p.conversation_id,
        sender_id: p.sender_id,
        channel: p.channel,
        external_id: p.external_id,
        sent_at: BsonDateTime::from_millis(p.sent_at.timestamp_millis()),
        content: p.content,
        summary: p.summary,
        context: p.context,
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

#[put("/messages/{id}/metadata")]
pub async fn update_message_metadata(
    db: web::Data<Database>,
    path: web::Path<Uuid>,
    payload: web::Json<UpdateMessageMetadataPayload>,
) -> actix_web::Result<impl Responder> {
    let msg_id = path.into_inner();
    let p = payload.into_inner();
    let msg_coll = db.collection::<Message>("messages");

    let msg_id_str = msg_id.to_string();

    let mut update_doc = doc! {};
    if let Some(summary) = p.summary {
        update_doc.insert("summary", summary);
    }
    if let Some(context) = p.context {
        update_doc.insert("context", context);
    }

    let msg = msg_coll
        .find_one_and_update(
            doc! { "_id": &msg_id_str },
            doc! { "$set": update_doc },
        )
        .return_document(ReturnDocument::After)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Message not found"))?;

    Ok(HttpResponse::Ok().json(msg))
}