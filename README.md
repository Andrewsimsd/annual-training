# Annual Training (Parody Cybersecurity Module)

A local Rust web app that mimics mandatory annual cybersecurity training with meme/video modules, a quiz, and pass/fail certificate output.

## Features

- Local web server on `127.0.0.1:3000`
- Training landing page with embedded videos
- Quiz workflow with a required passing score of **80%**
- PDF + JSON certificate generation for passing attempts, including a verification code
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
3. Enter your name and answer all quiz questions.
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

Edit the seed functions in `src/main.rs`:

- `seed_videos()` to add/remove video modules
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

> Note: `QuizForm` is currently fixed to 5 fields (`q0..q4`). If you change question count, update `QuizForm` and answer collection logic accordingly.

## Development checks

```bash
cargo fmt
cargo check
```

If `cargo check` fails in restricted environments, it may be due to blocked access to `crates.io`.
