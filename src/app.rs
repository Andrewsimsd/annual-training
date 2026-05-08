use axum::{
    Router,
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use rand::{rng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Path as StdPath, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

const PASS_THRESHOLD: f32 = 0.80;
const QUIZ_QUESTION_COUNT: usize = 5;
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
    <title>Mandatory Cybersecurity Annual Training</title>
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
      <h1>Cyber Security Annual Training (Totally Legit)</h1>
      <p>Please complete all required media modules and proceed to the quiz. Passing score: 80%.</p>
    </div>
    {}
    <a class='btn' href='/quiz'>Proceed to Compliance Quiz</a>
  </body>
</html>",
        videos_html
    ))
}

async fn quiz_page(State(state): State<AppState>) -> Html<String> {
    let selected_questions = choose_quiz_questions(&state.quizzes, QUIZ_QUESTION_COUNT);
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
<h1>Compliance Knowledge Check</h1>
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
fn choose_quiz_questions(question_bank: &[Question], question_count: usize) -> Vec<Question> {
    let mut shuffled = question_bank.to_vec();
    shuffled.shuffle(&mut rng());
    shuffled
        .into_iter()
        .take(question_count.min(question_bank.len()))
        .collect()
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
    let cert_json = serde_json::to_string_pretty(cert)
        .map_err(|err| std::io::Error::other(format!("serialization error: {err}")))?;
    let json_path = cert_dir.join(format!("certificate-{}.json", cert.cert_id));
    tokio::fs::write(json_path, cert_json).await?;
    let pdf_path = cert_dir.join(format!("certificate-{}.pdf", cert.cert_id));
    tokio::fs::write(pdf_path, build_certificate_pdf(cert)).await
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
        "<p>No certificate issued. Please retake training and achieve at least 80%.</p>".to_string()
    };
    Html(format!(
        "<h1>Exam Result: {status}</h1><p>Score: {}/{} ({:.1}%)</p>{}<p><a href='/'>Back to training portal</a></p>",
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
fn build_certificate_pdf(cert: &Certificate) -> Vec<u8> {
    let lines = vec![
        "Cybersecurity Annual Training Completion Certificate".to_string(),
        format!("Employee: {}", cert.employee_name),
        format!("Certificate ID: {}", cert.cert_id),
        format!("Completed At (UTC): {}", cert.issued_at_utc),
        format!(
            "Score: {}/{} ({:.1}%)",
            cert.score, cert.total, cert.score_percent
        ),
        format!("Verification Code: {}", cert.verification_code),
        "Use the certificate ID and verification code to confirm completion.".to_string(),
    ];
    let mut content = String::from("BT\n/F1 18 Tf\n50 760 Td\n");
    content.push_str("(Completion Certificate) Tj\n");
    content.push_str("0 -28 Td\n/F1 12 Tf\n");
    for line in lines {
        content.push_str(&format!("({}) Tj\n0 -20 Td\n", escape_pdf_text(&line)));
    }
    content.push_str("ET\n");
    let content_len = content.len();
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0usize];
    offsets.push(pdf.len());
    pdf.push_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.push_str("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.push_str("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.push_str("4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.push_str(&format!(
        "5 0 obj\n<< /Length {} >>\nstream\n{}endstream\nendobj\n",
        content_len, content
    ));
    let xref_start = pdf.len();
    pdf.push_str("xref\n0 6\n");
    pdf.push_str("0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        pdf.push_str(&format!("{:010} 00000 n \n", off));
    }
    pdf.push_str("trailer\n<< /Size 6 /Root 1 0 R >>\n");
    pdf.push_str(&format!("startxref\n{}\n%%EOF\n", xref_start));
    pdf.into_bytes()
}
fn seed_videos() -> Vec<Video> {
    vec![
        Video {
            title: "Module 1: Threat Awareness Through Cultural Study",
            url: "https://www.youtube.com/embed/xt5ghXdq6Z0",
            description: "Mandatory viewing: 'Lemme Smang It' as interpreted through enterprise risk narratives.",
        },
        Video {
            title: "Module 2: Meme Resilience Drills",
            url: "https://www.youtube.com/embed/W7Hoz2ZHYZM",
            description: "Secondary media module (configurable for future meme additions).",
        },
        Video {
            title: "Module 3: Social-Engineering Pattern Recognition",
            url: "https://www.youtube.com/embed/7cqOEr_yfak",
            description: "Required viewing: 'Look how dat boy mind me!' to practice identifying attention-grabbing social cues.",
        },
    ]
}
fn seed_questions() -> Vec<Question> {
    vec![
        Question {
            id: "module-one-hook",
            prompt: "In Module 1, what repeated hook anchors the song's chorus?",
            choices: [
                "Let me smang it, girl",
                "Enable multi-factor authentication",
                "Report phishing to IT",
                "Rotate your password monthly",
            ],
            correct: 0,
        },
        Question {
            id: "module-one-breakfast-line",
            prompt: "Which breakfast food is explicitly mentioned in Module 1's lyrics?",
            choices: ["Pancakes", "Oatmeal", "Eggs", "Bagels"],
            correct: 2,
        },
        Question {
            id: "module-one-guest-verse",
            prompt: "Who is credited as the featured artist in Module 1 ('Lemme Smang It')?",
            choices: ["Drake", "Flynt Flossy", "T-Pain", "Lil Jon"],
            correct: 1,
        },
        Question {
            id: "module-two-url-check",
            prompt: "Which YouTube video ID is used for Module 2 in this training portal?",
            choices: ["dQw4w9WgXcQ", "W7Hoz2ZHYZM", "xt5ghXdq6Z0", "7cqOEr_yfak"],
            correct: 1,
        },
        Question {
            id: "module-two-theme",
            prompt: "What competency does Module 2 emphasize in its title?",
            choices: [
                "Cloud migration governance",
                "Meme resilience drills",
                "SOC 2 audit evidence",
                "Incident ticket triage",
            ],
            correct: 1,
        },
        Question {
            id: "module-three-title-callout",
            prompt: "Which phrase is called out in Module 3's description?",
            choices: [
                "Look how dat boy mind me!",
                "Zero trust, zero chill",
                "Patch Tuesday forever",
                "Always verify sender domains",
            ],
            correct: 0,
        },
        Question {
            id: "module-three-focus",
            prompt: "Module 3 frames its video as practice for identifying what kind of cues?",
            choices: [
                "Attention-grabbing social cues",
                "Memory allocation bugs",
                "TLS certificate chains",
                "Kernel panic traces",
            ],
            correct: 0,
        },
        Question {
            id: "all-modules-required",
            prompt: "Before taking the quiz, what are employees expected to complete?",
            choices: [
                "Only Module 1",
                "Only Module 2",
                "All required media modules",
                "No media modules",
            ],
            correct: 2,
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
