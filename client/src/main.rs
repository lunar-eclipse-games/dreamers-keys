fn main() -> common::Result<()> {
    tracing_subscriber::fmt::init();

    if let Err(err) = client::run() {
        tracing::error!("Crashed due to error: {err}");
        Err(err)
    } else {
        tracing::info!("Exited without error");
        Ok(())
    }
}
