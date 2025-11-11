use actix_web::{get, post, web, HttpResponse, Responder};
use bson::{doc, Bson, DateTime as BsonDateTime};
use futures::TryStreamExt;
use mongodb::{
    options::FindOptions,
    Database,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::models::{Message, MessageSummary};

#[derive(Deserialize)]
pub struct CreateMessageSummaryPayload {
    pub conversation_id: Uuid,
    pub message_ids: Vec<Uuid>,
    pub summary: String,
    pub context: Option<String>,
}

#[post("/message-summaries")]
pub async fn create_message_summary(
    db: web::Data<Database>,
    payload: web::Json<CreateMessageSummaryPayload>,
) -> actix_web::Result<impl Responder> {
    let p = payload.into_inner();
    let msg_coll = db.collection::<Message>("messages");
    let summary_coll = db.collection::<MessageSummary>("message_summaries");

    let msg_id_strs: Vec<String> = p.message_ids.iter().map(|id| id.to_string()).collect();
    let msg_id_array: Vec<Bson> = msg_id_strs.iter().map(|id| Bson::String(id.clone())).collect();

    let mut cursor = msg_coll
        .find(doc! { "_id": { "$in": Bson::Array(msg_id_array) } })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut messages = Vec::new();
    while let Some(m) = cursor
        .try_next()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
    {
        if m.conversation_id != p.conversation_id {
            return Err(actix_web::error::ErrorBadRequest(
                "All messages must belong to the specified conversation"
            ));
        }
        messages.push(m);
    }

    if messages.is_empty() {
        return Err(actix_web::error::ErrorBadRequest("No valid messages found"));
    }

    let from_date = messages.iter().map(|m| m.sent_at).min().unwrap();
    let to_date = messages.iter().map(|m| m.sent_at).max().unwrap();

    let new_summary = MessageSummary {
        id: Uuid::new_v4(),
        conversation_id: p.conversation_id,
        message_ids: p.message_ids,
        summary: p.summary,
        context: p.context,
        created_at: BsonDateTime::now(),
        from_date,
        to_date,
    };

    summary_coll
        .insert_one(&new_summary)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(new_summary))
}

#[get("/conversations/{id}/summaries")]
pub async fn get_conversation_summaries(
    db: web::Data<Database>,
    path: web::Path<Uuid>,
) -> actix_web::Result<impl Responder> {
    let conv_id = path.into_inner();
    let summary_coll = db.collection::<MessageSummary>("message_summaries");

    let conv_id_str = conv_id.to_string();

    let options = FindOptions::builder()
        .sort(doc! { "from_date": 1 })
        .build();

    let mut cursor = summary_coll
        .find(doc! { "conversation_id": &conv_id_str })
        .with_options(options)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut summaries = Vec::new();
    while let Some(s) = cursor
        .try_next()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
    {
        summaries.push(s);
    }

    Ok(HttpResponse::Ok().json(summaries))
}