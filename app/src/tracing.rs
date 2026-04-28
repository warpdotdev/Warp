use tracing::subscriber;

pub fn init() -> anyhow::Result<()> {
    // Configure the global tracing subscriber to not care about any spans or
    // events.
    //
    // This is done so that we prevent the `tracing` crate from writing out log
    // lines for spans and trace events.
    subscriber::set_global_default(subscriber::NoSubscriber::new())?;

    Ok(())
}
