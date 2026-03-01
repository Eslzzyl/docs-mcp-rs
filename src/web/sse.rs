//! Server-Sent Events (SSE) handler for real-time updates.

use crate::web::handlers::AppState;
use axum::{
    extract::State,
    response::{
        IntoResponse,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
};
use std::convert::Infallible;

/// GET /api/events - SSE endpoint for real-time events.
pub async fn sse_handler(State(state): State<AppState>) -> impl IntoResponse {
    let receiver = state.event_bus.subscribe().await;

    // Create a stream from the receiver
    let stream = async_stream::stream! {
        let mut rx = receiver;
        loop {
            match rx.recv().await {
                Ok(event) => {
                    match serde_json::to_string(&event) {
                        Ok(json) => yield Ok::<_, Infallible>(SseEvent::default().data(json)),
                        Err(_) => continue,
                    }
                }
                Err(_) => {
                    // Channel closed or lagged, break the stream
                    break;
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
