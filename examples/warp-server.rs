use tokio;
use warp::{self, Filter};

#[tokio::main]
async fn main() {
    warp::serve(warp::path::end().map(move || "ok")).run(([127, 0, 0, 1], 8080)).await;
}
