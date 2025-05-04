use tracing::info;

fn main() -> common::Result<()> {
    tracing_subscriber::fmt::init();

    let addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
    let _server = puffin_http::Server::new(&addr).unwrap();
    info!("Puffin profiling running.");
    puffin::set_scopes_on(true);

    if let Err(err) = client::run() {
        tracing::error!("Crashed due to error: {err}");
        Err(err)
    } else {
        tracing::info!("Exited without error");
        Ok(())
    }
}
