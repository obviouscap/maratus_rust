mod handlers;
mod models;

use actix_web::{App, HttpServer, web};
use mongodb::{Client, Database};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mongo_uri = std::env::var("MONGO_URL")
        .unwrap_or_else(|_| "mongodb://root:example@mongo:27017/mydb?authSource=admin".into());
    let client: Client = Client::with_uri_str(&mongo_uri)
        .await
        .expect("failed to connect to MongoDB");
    let db: Database = client.database("unimsg");

    println!("Server running at http://127.0.0.1:8080");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(db.clone()))
            // Participant handlers
            .service(handlers::create_participant)
            .service(handlers::get_all_participants)
            .service(handlers::get_participant)
            // Conversation handlers
            .service(handlers::create_conversation)
            .service(handlers::get_all_conversations)
            .service(handlers::get_conversation)
            .service(handlers::update_conversation_metadata)
            // Message handlers
            .service(handlers::create_message)
            .service(handlers::get_all_messages)
            .service(handlers::get_message)
            .service(handlers::update_message_metadata)
            // Message summary handlers
            .service(handlers::create_message_summary)
            .service(handlers::get_conversation_summaries)
    })
        .bind(("0.0.0.0", 8080))?
        .run()
        .await
}