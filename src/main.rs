/// Binary entrypoint that delegates execution to [`annual_training::run`].
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
///     annual_training::run().await;
/// }
/// ```
#[tokio::main]
async fn main() {
    annual_training::run().await;
}
