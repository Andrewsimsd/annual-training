# Annual Training (Parody Cybersecurity Module)

A local Rust web app that mimics mandatory annual cybersecurity training with meme/video modules, a quiz, and pass/fail certificate output.

## Features

- Local web server on `127.0.0.1:3000`
- Training landing page with embedded videos
- Quiz workflow with a required passing score of **80%**
- Randomized quiz generation from a larger question bank (new random subset each attempt)
- PDF + JSON certificate generation for passing attempts, including a verification code
- Optional debug-only **Skip** button for developers (easy to disable)
- Seeded video/question content that can be extended for future modules and memes

## Requirements

- Rust toolchain (recommended via `rustup`)
- Cargo

## Run the app

```bash
cargo run
```

When started, open:

- <http://127.0.0.1:3000>

## App flow

1. Open the landing page and review the training media modules.
2. Click **Proceed to Compliance Quiz**.
3. Enter your name and answer the randomized quiz questions.
4. Submit the exam.
5. If score is `>= 80%`, a certificate file is written to:
   - `./certificates/certificate-<uuid>.pdf`
   - `./certificates/certificate-<uuid>.json`

## Certificate format

A passing certificate is generated as both PDF (for download) and JSON (for recordkeeping). The certificate includes:

- `cert_id`
- `employee_name`
- `issued_at_utc`
- `score_percent`
- `score`
- `total`
- `digest`
- `verification_code`

## Customize for more videos/memes

Edit the seed functions in `src/app.rs`:

- `seed_videos()` to add/remove video modules (now includes Module 3 with the new required video)
- `seed_questions()` to add/remove quiz items

### Add a video

Append another `Video` entry in `seed_videos()`:

- `title`
- `description`
- `url` (YouTube embed format like `https://www.youtube.com/embed/<video_id>`)

### Add a question

Append another `Question` entry in `seed_questions()` with:

- `prompt`
- exactly 4 `choices`
- `correct` index (0-based)

The app chooses a random subset each time from the larger question bank. Change `QUIZ_QUESTION_COUNT` in `src/app.rs` to control how many are used per quiz attempt.

## Debug skip button

During development, the quiz page includes a **Skip (Debug)** button that submits a passing attempt without answering questions. Disable it by setting:

- `ENABLE_DEBUG_SKIP` to `false` in `src/app.rs`

> Note: Quiz answers are parsed dynamically from `q{index}` form field names, so you can add or remove questions without changing the form type.

## Development checks

```bash
cargo fmt
cargo check
```

If `cargo check` fails in restricted environments, it may be due to blocked access to `crates.io`.
