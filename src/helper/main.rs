use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use std::fs::OpenOptions;
use std::io::Write;

async fn handle_post(body: String) -> impl Responder {
    let mut file = match OpenOptions::new()
        .write(true) // Enable write mode
        .truncate(true) // Truncate the file if it exists
        .create(true) // Create the file if it doesn't exist
        .open("post_contents.txt")
    {
        Ok(file) => file,
        Err(e) => {
            return HttpResponse::InternalServerError().body(format!("Failed to open file: {}", e))
        }
    };

    if let Err(e) = file.write_all(body.as_bytes()) {
        return HttpResponse::InternalServerError().body(format!("Failed to write to file: {}", e));
    }

    if let Err(e) = file.flush() {
        return HttpResponse::InternalServerError()
            .body(format!("Failed to flush contents: {}", e));
    }

    HttpResponse::Ok().body("POST request processed successfully")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().route("/schema", web::post().to(handle_post)))
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
