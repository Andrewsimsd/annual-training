#![allow(
    clippy::format_push_string,
    reason = "HTML/PDF template assembly benefits from straightforward string appends."
)]
#![allow(
    clippy::cast_precision_loss,
    reason = "Quiz scores are tiny bounded values; conversion to float is safe for percentage display."
)]
#![allow(
    clippy::uninlined_format_args,
    reason = "Keeping format placeholders positional improves readability in long template literals."
)]
#![allow(
    clippy::format_collect,
    reason = "Map/collect style keeps rendering logic concise and maintainable for small template output."
)]

pub mod app;

pub use app::run;
