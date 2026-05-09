# Annual Training Portal

A local Rust web application that delivers annual software development training modules, administers a required quiz, and issues completion certificates for passing attempts.

## Project Purpose

This project models a mandatory annual training workflow in a way that is simple to run locally, straightforward to maintain, and easy to extend with additional training content.

The portal is intentionally designed for:

- deterministic quiz behavior,
- explicit pass/fail scoring,
- auditable certificate artifacts, and
- maintainable Rust code with clear domain boundaries.

## Key Features

- Local HTTP server using Axum (`127.0.0.1:3000` by default).
- Training landing page with embedded media modules.
- Quiz workflow with a fixed passing threshold of **80%**.
- Full question-bank delivery on each attempt (no random sampling).
- Certificate generation for passing submissions:
  - downloadable PDF certificate,
  - JSON metadata artifact for audit/recordkeeping,
  - deterministic verification code and digest fields.
- Optional debug skip path for local development.

## Current Behavior (Important)

The application **does not randomly sample questions**. Each quiz attempt includes the full seeded question bank in a stable order.

## Architecture Overview

The codebase is organized by responsibility:

- `src/main.rs`: thin binary entrypoint.
- `src/lib.rs`: crate-level module exports and lint policy.
- `src/app.rs`: HTTP routing, page rendering, submission flow, and result/certificate endpoints.
- `src/quiz.rs`: quiz domain logic (answer parsing, question selection by ID, scoring).
- `src/certificate.rs`: certificate domain logic and low-level PDF artifact generation.

This separation keeps domain logic testable and reduces coupling between transport/UI and core behavior.

## Requirements

- Rust toolchain (stable, installed via `rustup` recommended)
- Cargo

## Build

```bash
cargo build
```

## Run

```bash
cargo run
```

Then open:

- <http://127.0.0.1:3000>

## User Workflow

1. Review all training modules on the landing page.
2. Select **Proceed to Development Practices Quiz**.
3. Enter employee name and submit responses.
4. View result page with score and pass/fail status.
5. If passed (>= 80%), download the generated PDF certificate.

## Certificate Artifacts

For passing attempts, the application writes files to `./certificates/`:

- `certificate-<uuid>.pdf`
- `certificate-<uuid>.json`

The JSON payload contains:

- `cert_id`
- `employee_name`
- `issued_at_utc`
- `score_percent`
- `score`
- `total`
- `digest`
- `verification_code`

## Configuration and Customization

### Training videos

Update `seed_videos()` in `src/app.rs` and provide:

- `title`
- `description`
- `url` (YouTube embed URL)

### Quiz questions

Update `seed_questions()` in `src/quiz.rs` and provide for each question:

- stable `id`
- `prompt`
- exactly four `choices`
- zero-based `correct` choice index

### Debug skip behavior

`ENABLE_DEBUG_SKIP` in `src/app.rs` controls whether a developer-only skip button is rendered on the quiz form.

## Testing and Validation

The project standard is to run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic
cargo test --all-features
cargo test --doc
cargo doc --all-features --no-deps
```

## Security and Operational Notes

- This app is intended for local/training use and binds to loopback by default.
- Certificate verification code generation is deterministic and intended for validation workflows, not as a security boundary.
- The app stores generated certificate artifacts on local disk.

## Development Notes

- Keep `main.rs` thin and place behavior in modules.
- Keep domain logic independent from web concerns where practical.
- Preserve explicit error handling and avoid panics for recoverable flows.

## License

Licensed under the terms of the repository `LICENSE` file.
