use actix_web::{get, post, put, web, HttpResponse, Responder};
use bson::{doc, Bson, DateTime as BsonDateTime};
use futures::TryStreamExt;
use mongodb::{
    options::{FindOptions, ReturnDocument},
    Database,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{Conversation, Participant, Message};

#[derive(Deserialize)]
pub struct CreateConversationPayload {
    pub external_id: Uuid,
    pub topic: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateConversationMetadataPayload {
    pub summary: Option<String>,
    pub context: Option<String>,
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

    let conv_id_str = conv_id.to_string();

    let conv = conv_coll
        .find_one(doc! { "_id": &conv_id_str })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Conversation not found"))?;

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

#[put("/conversations/{id}/metadata")]
pub async fn update_conversation_metadata(
    db: web::Data<Database>,
    path: web::Path<Uuid>,
    payload: web::Json<UpdateConversationMetadataPayload>,
) -> actix_web::Result<impl Responder> {
    let conv_id = path.into_inner();
    let p = payload.into_inner();
    let conv_coll = db.collection::<Conversation>("conversations");

    let conv_id_str = conv_id.to_string();

    let mut update_doc = doc! {};
    if let Some(summary) = p.summary {
        update_doc.insert("summary", summary);
    }
    if let Some(context) = p.context {
        update_doc.insert("context", context);
    }

    let conv = conv_coll
        .find_one_and_update(
            doc! { "_id": &conv_id_str },
            doc! { "$set": update_doc },
        )
        .return_document(ReturnDocument::After)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Conversation not found"))?;

    Ok(HttpResponse::Ok().json(conv))
}