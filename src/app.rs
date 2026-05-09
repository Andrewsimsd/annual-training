use axum::{
    Router,
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use image::ImageReader;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::Cursor,
    net::SocketAddr,
    path::{Path as StdPath, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

const PASS_THRESHOLD: f32 = 0.80;
const ENABLE_DEBUG_SKIP: bool = true;

#[derive(Clone)]
struct AppState {
    videos: Arc<Vec<Video>>,
    quizzes: Arc<Vec<Question>>,
    results: Arc<RwLock<HashMap<String, ExamAttempt>>>,
    cert_dir: Arc<PathBuf>,
}

#[derive(Clone, Serialize)]
struct Video {
    title: &'static str,
    url: &'static str,
    description: &'static str,
}

#[derive(Clone)]
struct Question {
    id: &'static str,
    prompt: &'static str,
    choices: [&'static str; 4],
    correct: usize,
}

#[derive(Clone)]
struct ExamAttempt {
    score: usize,
    total: usize,
    passed: bool,
    cert_id: Option<String>,
}

#[derive(Serialize)]
struct Certificate {
    cert_id: String,
    employee_name: String,
    issued_at_utc: String,
    score_percent: f32,
    score: usize,
    total: usize,
    digest: String,
    verification_code: String,
}

#[derive(Deserialize)]
struct QuizForm {
    employee_name: String,
    selected_question_ids: String,
    debug_skip: Option<String>,
    #[serde(flatten)]
    answers: HashMap<String, String>,
}

#[derive(Debug)]
struct QuizEvaluation {
    score: usize,
    total: usize,
    passed: bool,
}

pub async fn run() {
    let cert_dir = PathBuf::from("certificates");
    if let Err(err) = tokio::fs::create_dir_all(&cert_dir).await {
        eprintln!("could not create certificates directory: {err}");
        return;
    }

    let state = AppState {
        videos: Arc::new(seed_videos()),
        quizzes: Arc::new(seed_questions()),
        results: Arc::new(RwLock::new(HashMap::new())),
        cert_dir: Arc::new(cert_dir),
    };

    let app = Router::new()
        .route("/", get(home))
        .route("/quiz", get(quiz_page).post(submit_quiz))
        .route("/result", get(result_page))
        .route("/certificate/{cert_id}", get(download_certificate))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let app_url = format!("http://{addr}");
    println!("training portal running at {app_url}");

    if let Err(err) = webbrowser::open(&app_url) {
        eprintln!("could not open browser automatically: {err}");
    }

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("could not bind listener: {err}");
            return;
        }
    };

    if let Err(err) = axum::serve(listener, app).await {
        eprintln!("server error: {err}");
    }
}

async fn home(State(state): State<AppState>) -> Html<String> {
    /* unchanged */
    let videos_html = state
        .videos
        .iter()
        .map(|v| {
            format!(
                r"<section class='card'>
<h3>{}</h3>
<p>{}</p>
<iframe width='560' height='315' src='{}' title='{}' allowfullscreen></iframe>
</section>",
                v.title, v.description, v.url, v.title
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Html(format!(
        r"<!doctype html>
<html>
  <head>
    <meta charset='utf-8'>
    <title>Mandatory Software Development Annual Training</title>
    <style>
      body {{ font-family: Arial, sans-serif; max-width: 980px; margin: 2rem auto; background: #f2f4f8; }}
      .banner {{ background: #112f6f; color: white; padding: 1rem; border-radius: .5rem; }}
      .card {{ background: white; padding: 1rem; margin: 1rem 0; border-radius: .5rem; box-shadow: 0 1px 4px #00000022; }}
      .btn {{ display: inline-block; margin-top: 1rem; background: #0a6; color: white; text-decoration: none; padding: .7rem 1rem; border-radius: .4rem; }}
      iframe {{ width: 100%; min-height: 315px; border: 0; }}
    </style>
  </head>
  <body>
    <div class='banner'>
      <h1>Software Development Annual Training</h1>
      <p>Please complete all required media modules and proceed to the quiz. Passing score: 80%.</p>
    </div>
    {}
    <a class='btn' href='/quiz'>Proceed to Development Practices Quiz</a>
  </body>
</html>",
        videos_html
    ))
}

async fn quiz_page(State(state): State<AppState>) -> Html<String> {
    let selected_questions = choose_quiz_questions(&state.quizzes);
    let selected_ids = selected_questions
        .iter()
        .map(|question| question.id)
        .collect::<Vec<_>>()
        .join(",");
    let question_markup = selected_questions
        .iter()
        .enumerate()
        .map(|(idx, q)| {
            let options = q
                .choices
                .iter()
                .enumerate()
                .map(|(choice_idx, choice)| {
                    format!(
                        "<label><input required type='radio' name='q{idx}' value='{choice_idx}'> {choice}</label><br>"
                    )
                })
                .collect::<String>();
            format!(
                "<fieldset><legend><strong>Q{}:</strong> {}</legend>{}</fieldset><br>",
                idx + 1,
                q.prompt,
                options
            )
        })
        .collect::<String>();
    let skip_button = if ENABLE_DEBUG_SKIP {
        "<button type='submit' name='debug_skip' value='1' formnovalidate style='margin-left: .75rem;'>Skip (Debug)</button>"
    } else {
        ""
    };
    Html(format!(
        r"<!doctype html>
<html><head><meta charset='utf-8'><title>Quiz</title></head>
<body style='font-family: Arial, sans-serif; max-width: 850px; margin: 2rem auto;'>
<h1>Software Development Knowledge Check</h1>
<form method='post' action='/quiz'>
<label>Your full name: <input type='text' name='employee_name' required></label><br><br>
<input type='hidden' name='selected_question_ids' value='{}'>
{}
<button type='submit'>Submit Exam</button>
{}
</form>
</body></html>",
        selected_ids, question_markup, skip_button
    ))
}

async fn submit_quiz(
    State(state): State<AppState>,
    Form(payload): Form<QuizForm>,
) -> impl IntoResponse {
    let skip_requested = ENABLE_DEBUG_SKIP && payload.debug_skip.is_some();
    let selected_questions = select_questions_by_id(&state.quizzes, &payload.selected_question_ids);
    if selected_questions.is_empty() {
        return Redirect::to("/quiz");
    }
    let parsed_answers = if skip_requested {
        selected_questions
            .iter()
            .enumerate()
            .map(|(idx, question)| (format!("q{idx}"), question.correct))
            .collect()
    } else {
        parse_answers(&payload.answers)
    };
    let evaluation = evaluate_quiz(&selected_questions, &parsed_answers);
    let employee_name = if skip_requested {
        "debug".to_owned()
    } else {
        payload.employee_name
    };
    let mut cert_id = None;
    if evaluation.passed {
        let id = Uuid::new_v4().to_string();
        let cert = build_certificate(&id, &employee_name, evaluation.score, evaluation.total);
        if let Err(err) = write_certificate_files(&state.cert_dir, &cert).await {
            eprintln!("failed to persist certificate files: {err}");
        } else {
            cert_id = Some(id);
        }
    }
    let ticket = Uuid::new_v4().to_string();
    state.results.write().await.insert(
        ticket.clone(),
        ExamAttempt {
            score: evaluation.score,
            total: evaluation.total,
            passed: evaluation.passed,
            cert_id,
        },
    );
    Redirect::to(&format!("/result?ticket={ticket}"))
}

fn parse_answers(raw_answers: &HashMap<String, String>) -> HashMap<String, usize> {
    raw_answers
        .iter()
        .filter_map(|(key, value)| {
            if !key.starts_with('q') {
                return None;
            }
            value
                .parse::<usize>()
                .ok()
                .map(|parsed| (key.clone(), parsed))
        })
        .collect()
}
fn choose_quiz_questions(question_bank: &[Question]) -> Vec<Question> {
    question_bank.to_vec()
}
fn select_questions_by_id(question_bank: &[Question], raw_ids: &str) -> Vec<Question> {
    raw_ids
        .split(',')
        .filter_map(|id| question_bank.iter().find(|question| question.id == id))
        .cloned()
        .collect()
}
fn evaluate_quiz(questions: &[Question], answers: &HashMap<String, usize>) -> QuizEvaluation {
    let score = questions
        .iter()
        .enumerate()
        .filter(
            |(idx, q)| matches!(answers.get(&format!("q{idx}")), Some(ans) if *ans == q.correct),
        )
        .count();
    let total = questions.len();
    let pct = score as f32 / total as f32;
    QuizEvaluation {
        score,
        total,
        passed: pct >= PASS_THRESHOLD,
    }
}
fn build_certificate(
    cert_id: &str,
    employee_name: &str,
    score: usize,
    total: usize,
) -> Certificate {
    let digest = format!(
        "{:x}",
        Sha256::digest(format!("{cert_id}:{employee_name}:{score}:{total}"))
    );
    Certificate {
        cert_id: cert_id.to_owned(),
        employee_name: employee_name.to_owned(),
        issued_at_utc: chrono::Utc::now().to_rfc3339(),
        score_percent: (score as f32 / total as f32) * 100.0,
        score,
        total,
        digest,
        verification_code: verification_code(cert_id, employee_name, score, total),
    }
}
async fn write_certificate_files(cert_dir: &StdPath, cert: &Certificate) -> std::io::Result<()> {
    let badge_path = StdPath::new("resources").join("badge.png");
    let badge_bytes = tokio::fs::read(&badge_path).await?;
    let cert_json = serde_json::to_string_pretty(cert)
        .map_err(|err| std::io::Error::other(format!("serialization error: {err}")))?;
    let json_path = cert_dir.join(format!("certificate-{}.json", cert.cert_id));
    tokio::fs::write(json_path, cert_json).await?;
    let pdf_path = cert_dir.join(format!("certificate-{}.pdf", cert.cert_id));
    tokio::fs::write(pdf_path, build_certificate_pdf(cert, &badge_bytes)?).await
}
async fn result_page(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    let Some(ticket) = params.get("ticket") else {
        return Html("<h1>Missing ticket.</h1>".to_string());
    };
    let result = state.results.read().await.get(ticket).cloned();
    let Some(result) = result else {
        return Html("<h1>Unknown result ticket.</h1>".to_string());
    };
    let pct = (result.score as f32 / result.total as f32) * 100.0;
    let status = if result.passed { "PASS" } else { "FAIL" };
    let cert_msg = if let Some(cert_id) = result.cert_id {
        format!(
            "<p>Certificate issued. ID: <code>{cert_id}</code></p><p>Your completion certificate is ready as a PDF. It includes a verification code that can be used to validate training completion.</p><p><a href='/certificate/{cert_id}' download>Download completion certificate (.pdf)</a></p><script>setTimeout(function () {{  if (confirm('You passed! Download your completion certificate PDF now?')) {{    window.location.href = '/certificate/{cert_id}';  }}}}, 300);</script>"
        )
    } else {
        "<p>No certificate issued. Please retake the training and achieve at least 80%.</p>"
            .to_string()
    };
    Html(format!(
        "<h1>Exam Result: {status}</h1><p>Score: {}/{} ({:.1}%)</p>{}<p><a href='/'>Back to development training portal</a></p>",
        result.score, result.total, pct, cert_msg
    ))
}
async fn download_certificate(
    State(state): State<AppState>,
    Path(cert_id): Path<String>,
) -> Response {
    let path = state.cert_dir.join(format!("certificate-{cert_id}.pdf"));
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let mut res = Response::new(Body::from(bytes));
            *res.status_mut() = StatusCode::OK;
            res.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/pdf"),
            );
            let content_disposition = format!("attachment; filename=\"certificate-{cert_id}.pdf\"");
            if let Ok(value) = HeaderValue::from_str(&content_disposition) {
                res.headers_mut().insert(header::CONTENT_DISPOSITION, value);
            }
            res
        }
        Err(_) => (StatusCode::NOT_FOUND, "Certificate not found").into_response(),
    }
}
fn verification_code(cert_id: &str, employee_name: &str, score: usize, total: usize) -> String {
    let digest = Sha256::digest(format!("verify:{cert_id}:{employee_name}:{score}:{total}"));
    let hex = format!("{:x}", digest);
    hex[..12].to_uppercase()
}
fn escape_pdf_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}
fn build_certificate_pdf(cert: &Certificate, badge_png: &[u8]) -> std::io::Result<Vec<u8>> {
    let content = format!(
        "q
1 1 1 rg
0 0 792 612 re
f
Q
\
q
1 1 1 rg
0 0 792 612 re
f
Q
\
q
1 1 1 rg
0 0 792 612 re
f
Q
\
q
1 1 1 rg
28 28 736 556 re
f
Q
\
q
0.73 0.82 0.96 rg
40 40 712 532 re
S
Q
\
q
0.65 0.76 0.95 rg
52 52 688 508 re
S
Q
\
q
0.84 0.91 1 rg
56 56 680 500 re
f
Q
\
q
0.75 0.86 0.99 rg
70 70 652 472 re
f
Q
\
q
0.82 0.90 0.99 rg
80 500 632 3 re
f
80 118 632 3 re
f
Q
\
q
0.79 0.87 0.98 rg
120 450 560 2 re
f
120 170 560 2 re
f
Q
\
q
0.20 0.35 0.66 RG
6 w
30 30 732 552 re
S
Q
\
q
0.27 0.46 0.80 RG
2 w
48 48 696 516 re
S
Q
\
q
0.76 0.65 0.37 RG
6 w
30 30 732 552 re
S
Q
\
q
0.86 0.77 0.50 RG
2 w
48 48 696 516 re
S
Q
\
q
170 0 0 170 545 85 cm
/Im1 Do
Q
\
BT
/F1 38 Tf
80 500 Td
(Completion Certificate) Tj
\
0 -44 Td
/F1 16 Tf
(Awarded for successful completion of Annual Software Development Training) Tj
\
0 -68 Td
/F1 20 Tf
(Presented to) Tj
\
0 -42 Td
/F1 28 Tf
({}) Tj
\
0 -54 Td
/F1 18 Tf
(Completed At \\(UTC\\): {}) Tj
\
0 -30 Td
(Certificate ID: {}) Tj
\
0 -30 Td
(Score: {}/{} \\({:.1}%\\)) Tj
\
0 -30 Td
(Verification Code: {}) Tj
\
0 -52 Td
/F1 13 Tf
(Use certificate ID and verification code to confirm completion.) Tj
ET
",
        escape_pdf_text(&cert.employee_name),
        escape_pdf_text(&cert.issued_at_utc),
        escape_pdf_text(&cert.cert_id),
        cert.score,
        cert.total,
        cert.score_percent,
        escape_pdf_text(&cert.verification_code),
    );

    let (badge_width, badge_height, badge_stream) = encode_badge_stream(badge_png)?;
    let mut pdf = Vec::new();
    pdf.extend_from_slice(
        b"%PDF-1.4
",
    );
    let mut offsets = vec![0_usize];

    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
",
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
",
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 792 612] /Resources << /Font << /F1 4 0 R >> /XObject << /Im1 6 0 R >> >> /Contents 5 0 R >>
endobj
");
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"4 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>
endobj
",
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        format!(
            "5 0 obj
<< /Length {} >>
stream
{}endstream
endobj
",
            content.len(),
            content
        )
        .as_bytes(),
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        format!(
            "6 0 obj
<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>
stream
",
            badge_width,
            badge_height,
            badge_stream.len()
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(&badge_stream);
    pdf.extend_from_slice(
        b"
endstream
endobj
",
    );

    let xref_start = pdf.len();
    pdf.extend_from_slice(
        b"xref
0 7
0000000000 65535 f 
",
    );
    for off in offsets.iter().skip(1) {
        pdf.extend_from_slice(
            format!(
                "{:010} 00000 n 
",
                off
            )
            .as_bytes(),
        );
    }
    pdf.extend_from_slice(
        b"trailer
<< /Size 7 /Root 1 0 R >>
",
    );
    pdf.extend_from_slice(
        format!(
            "startxref
{}
%%EOF
",
            xref_start
        )
        .as_bytes(),
    );
    Ok(pdf)
}

fn encode_badge_stream(png_bytes: &[u8]) -> std::io::Result<(u32, u32, Vec<u8>)> {
    let image = ImageReader::new(Cursor::new(png_bytes))
        .with_guessed_format()
        .map_err(|err| std::io::Error::other(format!("badge format error: {err}")))?
        .decode()
        .map_err(|err| std::io::Error::other(format!("badge decode error: {err}")))?
        .to_rgb8();
    let (width, height) = image.dimensions();
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    use std::io::Write as _;
    encoder
        .write_all(&image.into_raw())
        .map_err(|err| std::io::Error::other(format!("badge stream write error: {err}")))?;
    let compressed = encoder
        .finish()
        .map_err(|err| std::io::Error::other(format!("badge compression error: {err}")))?;
    Ok((width, height, compressed))
}

fn seed_videos() -> Vec<Video> {
    vec![
        Video {
            title: "Module 1: Threat Awareness Through Cultural Study",
            url: "https://www.youtube.com/embed/xt5ghXdq6Z0",
            description: "Mandatory viewing: 'Lemme Smang It' as interpreted through enterprise risk narratives.",
        },
        Video {
            title: "Module 2: Fundamentals of Maintainability and Supply Chain Risk Analysis",
            url: "https://www.youtube.com/embed/W7Hoz2ZHYZM",
            description: "Mandatory viewing: Prevent your code base from becoming the liquid slam monster by practicing good repository hygiene.",
        },
        Video {
            title: "Module 3: Social-Engineering Pattern Recognition",
            url: "https://www.youtube.com/embed/7cqOEr_yfak",
            description: "Required viewing: 'Look how dat boy mind me!' to practice identifying attention-grabbing social cues.",
        },
        Video {
            title: "Module 4: Learning When to Say No.",
            url: "https://www.youtube.com/embed/2aj-8lmB5q8",
            description: "New assignment module: Knowing when to say no is important, but sometimes you have to deal with consequences.",
        },
    ]
}
fn seed_questions() -> Vec<Question> {
    vec![
        Question {
            id: "module-one-lemme-smang-it-meaning",
            prompt: "In the context of software threat vectors, what does “lemme smang it” really mean?",
            choices: [
                "Please allow this unsigned executable to run with administrator privileges.",
                "Let me bypass input validation and inject something spicy into your backend.",
                "I found an exposed debug port and would like to introduce myself.",
                "This third-party dependency looks trustworthy because the README has badges.",
            ],
            correct: 1,
        },
        Question {
            id: "module-one-smash-bang-fusion",
            prompt: "In Git, how can “smash bang fusion” be correctly implemented safely when merging branches with conflicts?",
            choices: [
                "Run git merge, see conflict markers, delete the weird-looking lines, and commit whatever still compiles.",
                "Force-push main over everyone else’s work because true fusion requires dominance.",
                "Carefully review each conflict, understand both sides of the change, resolve the file intentionally, run the tests, then commit the merge.",
                "Accept all incoming changes because the other branch probably had more confidence.",
            ],
            correct: 2,
        },
        Question {
            id: "module-one-be-cautious",
            prompt: "What does “you should be cautious, but don’t be scary — Imma have you looking like a Wild Thornberry” mean when dealing with thread safety and lifetimes?",
            choices: [
                "Share references across threads freely; the borrow checker is just being dramatic.",
                "Use static mut for shared state because nothing says “cautious” like global chaos.",
                "Ensure shared data satisfies the right lifetime, Send, and Sync requirements, and use safe synchronization primitives like Arc<Mutex<T>> or Arc<RwLock<T>> when ownership crosses thread boundaries.",
                "Add lifetime annotations everywhere until the code looks like it was attacked by punctuation.",
            ],
            correct: 2,
        },
        Question {
            id: "module-two-liquid-slam-avoidance",
            prompt: "In software development, how do you avoid creating a “liquid slam monster” in your repository?",
            choices: [
                "Keep adding features directly into main until the codebase begins begging for deletion.",
                "Copy-paste the same logic into five places because consistency is a future problem.",
                "Maintain clear module boundaries, write tests, document intent, refactor deliberately, review changes carefully, and keep technical debt visible instead of letting it mutate in the basement.",
                "Rename every variable to x, thing, or final_final_real_v2 so nobody gets emotionally attached.",
            ],
            correct: 2,
        },
        Question {
            id: "module-two-liquid-slam-causes",
            prompt: "What common pitfall most often leads to unknown “liquid slam monsters” hiding in a repository?",
            choices: [
                "Time crunches that turn “we’ll clean it up later” into permanent architecture.",
                "Skipping tests because the demo worked once on somebody’s laptop.",
                "Copy-pasting code without understanding it, then building new features on top of it.",
                "All of the above.",
            ],
            correct: 3,
        },
        Question {
            id: "module-three-look-how-good-he-mind-me-meaning",
            prompt: "In software testing, what does “Look how good he mind me” really mean when you see code work one time during a manual demo?",
            choices: [
                "The code obeyed once, so it is obviously production-ready and should be merged immediately.",
                "The happy path worked, but that does not prove the code handles edge cases, invalid inputs, failures, or future regressions.",
                "Unit tests are unnecessary if the developer says, “It worked on my machine,” with enough confidence.",
                "The code has formed an emotional bond with the tester and will continue behaving out of loyalty.",
            ],
            correct: 1,
        },
        Question {
            id: "module-three-you-gon-get-wet",
            prompt: "In software development, what does “Don’t go in that damn water, I’m over here, don’t go, you gon get wet” really mean?",
            choices: [
                "Ignore warnings from senior engineers because the forbidden water probably has better architecture.",
                "Avoid known risky areas of the codebase unless you understand the impact, have tests in place, and know how to recover if things break.",
                "Jump straight into production changes because getting wet is how you learn.",
                "Disable CI checks so the repository stops yelling about puddles.",
            ],
            correct: 1,
        },
        Question {
            id: "module-three-feet-wet",
            prompt: "In software development, what does “He got his damn feet wet, now shit dog” mean after someone ignores the warning signs?",
            choices: [
                "A small “harmless” change touched risky code and now the team is discovering hidden side effects.",
                "The developer successfully hydrated the repository, which improves runtime moisture content.",
                "Production incidents are fine as long as the commit message says “minor cleanup.”",
                "The correct fix is to panic-revert everything without reading the logs.",
            ],
            correct: 0,
        },
        Question {
            id: "module-three-crank-the-vibe",
            prompt: "In software project management, what does “I ain’t not buying your shit” usually mean when a team is trying to deliver a project?",
            choices: [
                "Leadership expects the project to succeed but refuses to fund the tools, staffing, licenses, hardware, training, test equipment, or schedule needed to do it correctly.",
                "The team should simply increase velocity by believing harder.",
                "The correct engineering response is to remove testing, documentation, and code review so the budget feels respected.",
                "If the project fails, everyone should act surprised and schedule a lessons-learned meeting called “Unexpected Outcomes.”",
            ],
            correct: 0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evaluate_quiz_returns_pass_when_threshold_is_met() {
        let questions = seed_questions();
        let mut answers = HashMap::new();
        for (idx, question) in questions.iter().enumerate() {
            answers.insert(format!("q{idx}"), question.correct);
        }
        let evaluation = evaluate_quiz(&questions, &answers);
        assert!(evaluation.passed);
    }
    #[test]
    fn evaluate_quiz_handles_missing_answers_as_incorrect() {
        let questions = seed_questions();
        let answers = HashMap::new();
        let evaluation = evaluate_quiz(&questions, &answers);
        assert_eq!(evaluation.score, 0);
    }
    #[test]
    fn parse_answers_ignores_non_numeric_values() {
        let mut raw = HashMap::new();
        raw.insert("q0".to_string(), "2".to_string());
        raw.insert("employee_name".to_string(), "Alice".to_string());
        raw.insert("q1".to_string(), "not-a-number".to_string());
        let parsed = parse_answers(&raw);
        assert_eq!(parsed.get("q0"), Some(&2));
    }
    #[test]
    fn verification_code_is_stable_for_same_input() {
        let code = verification_code("abc", "Jane", 4, 5);
        assert_eq!(code, verification_code("abc", "Jane", 4, 5));
    }
}
