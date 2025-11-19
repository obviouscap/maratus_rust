use actix_web::{get, post, web, HttpResponse, Responder};
use bson::doc;
use futures::TryStreamExt;
use mongodb::{
    options::ReturnDocument,
    Database,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::models::{Participant, ParticipantType};

#[derive(Deserialize)]
pub struct CreateParticipantPayload {
    pub address: String,
    pub display_name: Option<String>,
    #[serde(rename = "type")]
    pub participant_type: ParticipantType,
    pub description: Option<String>,
}

#[post("/participants")]
pub async fn create_participant(
    db: web::Data<Database>,
    payload: web::Json<CreateParticipantPayload>,
) -> actix_web::Result<impl Responder> {
    let p = payload.into_inner();
    let part_coll = db.collection::<Participant>("participants");

    let participant_type_bson = bson::to_bson(&p.participant_type)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let part = part_coll
        .find_one_and_update(
            doc! { "address": &p.address },
            doc! {
              "$setOnInsert": {
                "_id": Uuid::new_v4().to_string(),
                "address": &p.address
              },
              "$set": {
                "display_name": &p.display_name,
                "type": participant_type_bson,
                "description": &p.description
              }
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
    path: web::Path<String>,
) -> actix_web::Result<impl Responder> {
    let part_id = path.into_inner();
    let part_coll = db.collection::<Participant>("participants");
    println!("Start to process query {part_id}");

    let part = part_coll
        .find_one(doc! { "_id": &part_id })
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Participant not found"))?;

    Ok(HttpResponse::Ok().json(part))
}