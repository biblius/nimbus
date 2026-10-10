mod auth;
mod cache;
mod config;
mod db;
mod email;
mod error;
mod file;
mod model;
mod router;
mod state;
mod user;

#[cfg(test)]
mod test;

#[cfg(not(any(feature = "postgres", feature = "sqlite")))]
compile_error!("enable exactly one of the `postgres` or `sqlite` features");

#[cfg(all(feature = "postgres", feature = "sqlite"))]
compile_error!("features `postgres` and `sqlite` are mutually exclusive");

#[cfg(not(any(feature = "valkey", feature = "memory-cache")))]
compile_error!("enable exactly one of the `valkey` or `memory-cache` features");

#[cfg(all(feature = "valkey", feature = "memory-cache"))]
compile_error!("features `valkey` and `memory-cache` are mutually exclusive");

#[tokio::main]
async fn main() {
    let args = <config::StartArgs as clap::Parser>::parse();

    println!("======= ARGS =======\n{args}\n=======");

    let app = state::AppState::new(&args).await;

    let addr = args.address();

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("error while starting TCP listener");

    let router = router::router(app);

    tracing::info!("Listening on {addr}");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .expect("error while starting server");
}
