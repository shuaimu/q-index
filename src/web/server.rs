use actix_web::{web, App, HttpServer, middleware};
use actix_files::Files;
use log::info;
use std::sync::Arc;

use crate::parser::BibParser;
use crate::web::state::{AppState, APP_STATE};
use crate::web::handlers::*;

pub async fn start_server(bib_dir: &str, port: u16) -> std::io::Result<()> {
    info!("Loading bibliography data from {}...", bib_dir);
    
    // Parse bibliography
    let mut parser = BibParser::new();
    let graph = parser.parse_directory(bib_dir)
        .expect("Failed to parse bibliography");
    
    info!("Loaded {} papers, {} venues, {} scholars",
          graph.papers.len(), graph.venues.len(), graph.scholars.len());
    
    // Initialize application state
    let state = Arc::new(AppState::new(graph));
    APP_STATE.set(state.clone()).expect("Failed to set application state");
    
    info!("Starting web server on http://127.0.0.1:{}", port);
    
    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            
            // HTML routes
            .route("/", web::get().to(index_handler))
            .route("/venues", web::get().to(venues_handler))
            .route("/scholars", web::get().to(scholars_handler))
            .route("/search", web::get().to(search_handler))
            .route("/about", web::get().to(about_handler))
            .route("/statistics", web::get().to(statistics_handler))
            
            // API routes
            .route("/api/venues", web::get().to(api_venues))
            .route("/api/scholars", web::get().to(api_scholars))
            .route("/api/search", web::get().to(api_search))
            .route("/api/stats", web::get().to(api_stats))
            .route("/api/stats/fields", web::get().to(api_stats_fields))
            
            // Static files
            .service(Files::new("/static", "./static").show_files_listing())
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}