use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use std::fs::OpenOptions;
use std::io::Write;

async fn write_to_file(body: String, path: String) -> (bool, String) {
    let mut file = match OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
    {
        Ok(file) => file,
        Err(e) => {
            return (false, format!("Failed to open file: {}", e));
        }
    };

    if let Err(e) = file.write_all(body.as_bytes()) {
        return (false, format!("Failed to write to file: {}", e));
    }

    if let Err(e) = file.flush() {
        return (false, format!("Failed to flush contents: {}", e));
    }

    return (true, "File updated successfully".to_string());
}

async fn update_schema(body: String) -> impl Responder {
    match write_to_file(body, "/dist/schema.graphql".to_owned()).await {
        (true, _) => {
            HttpResponse::Ok().body("POST request processed successfully. Schema updated.")
        }
        (false, message) => {
            return HttpResponse::InternalServerError().body(message);
        }
    }
}

async fn update_config(body: String) -> impl Responder {
    match write_to_file(body, "/dist/config.yaml".to_owned()).await {
        (true, _) => {
            HttpResponse::Ok().body("POST request processed successfully. Config updated.")
        }
        (false, message) => {
            return HttpResponse::InternalServerError().body(message);
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/schema", web::post().to(update_schema))
            .route("/config", web::post().to(update_config))
    })
    .bind("0.0.0.0:9000")?
    .run()
    .await
}
