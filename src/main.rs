use axum::{
    Router,
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

const PASS_THRESHOLD: f32 = 0.80;

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
}

#[derive(Deserialize)]
struct QuizForm {
    employee_name: String,
    q0: usize,
    q1: usize,
    q2: usize,
    q3: usize,
    q4: usize,
}

#[tokio::main]
async fn main() {
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
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let app_url = format!("http://{addr}");
    println!("training portal running at {app_url}");

    if let Err(err) = webbrowser::open(&app_url) {
        eprintln!("could not open browser automatically: {err}");
    }

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn home(State(state): State<AppState>) -> Html<String> {
    let videos_html = state
        .videos
        .iter()
        .map(|v| {
            format!(
                r#"<section class='card'>
<h3>{}</h3>
<p>{}</p>
<iframe width='560' height='315' src='{}' title='{}' allowfullscreen></iframe>
</section>"#,
                v.title, v.description, v.url, v.title
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Html(format!(
        r#"<!doctype html>
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
</html>"#,
        videos_html
    ))
}

async fn quiz_page(State(state): State<AppState>) -> Html<String> {
    let question_markup = state
        .quizzes
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

    Html(format!(
        r#"<!doctype html>
<html><head><meta charset='utf-8'><title>Quiz</title></head>
<body style='font-family: Arial, sans-serif; max-width: 850px; margin: 2rem auto;'>
<h1>Compliance Knowledge Check</h1>
<form method='post' action='/quiz'>
<label>Your full name: <input type='text' name='employee_name' required></label><br><br>
{}
<button type='submit'>Submit Exam</button>
</form>
</body></html>"#,
        question_markup
    ))
}

async fn submit_quiz(
    State(state): State<AppState>,
    Form(payload): Form<QuizForm>,
) -> impl IntoResponse {
    let answers = [payload.q0, payload.q1, payload.q2, payload.q3, payload.q4];

    let score = state
        .quizzes
        .iter()
        .zip(answers.iter())
        .filter(|(q, ans)| q.correct == **ans)
        .count();

    let total = state.quizzes.len();
    let pct = score as f32 / total as f32;
    let passed = pct >= PASS_THRESHOLD;

    let mut cert_id = None;

    if passed {
        let id = Uuid::new_v4().to_string();
        let issued_at_utc = chrono::Utc::now().to_rfc3339();
        let digest = format!(
            "{:x}",
            Sha256::digest(format!(
                "{}:{}:{}:{}",
                id, payload.employee_name, score, total
            ))
        );

        let cert = Certificate {
            cert_id: id.clone(),
            employee_name: payload.employee_name.clone(),
            issued_at_utc,
            score_percent: pct * 100.0,
            score,
            total,
            digest,
        };

        let cert_json = serde_json::to_string_pretty(&cert).unwrap();
        let path = state
            .cert_dir
            .join(format!("certificate-{}.json", cert.cert_id));
        let _ = tokio::fs::write(path, cert_json).await;
        cert_id = Some(id);
    }

    let ticket = Uuid::new_v4().to_string();
    state.results.write().await.insert(
        ticket.clone(),
        ExamAttempt {
            score,
            total,
            passed,
            cert_id,
        },
    );

    Redirect::to(&format!("/result?ticket={ticket}"))
}

async fn result_page(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
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
            "<p>Certificate issued. ID: <code>{cert_id}</code></p><p>Serialized certificate written under <code>./certificates</code>.</p>"
        )
    } else {
        "<p>No certificate issued. Please retake training and achieve at least 80%.</p>".to_string()
    };

    Html(format!(
        "<h1>Exam Result: {status}</h1><p>Score: {}/{} ({:.1}%)</p>{}<p><a href='/'>Back to training portal</a></p>",
        result.score, result.total, pct, cert_msg
    ))
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
            url: "https://www.youtube.com/embed/dQw4w9WgXcQ",
            description: "Secondary media module (configurable for future meme additions).",
        },
    ]
}

fn seed_questions() -> Vec<Question> {
    vec![
        Question {
            prompt: "What is the minimum passing score for this training?",
            choices: ["50%", "70%", "80%", "100%"],
            correct: 2,
        },
        Question {
            prompt: "Where are certificates written after passing?",
            choices: [
                "In memory only",
                "./certificates",
                "Downloads folder",
                "A remote blockchain",
            ],
            correct: 1,
        },
        Question {
            prompt: "What must you complete before taking the quiz?",
            choices: ["Tax documents", "Media modules", "Driver's test", "Nothing"],
            correct: 1,
        },
        Question {
            prompt: "What format is the certificate file?",
            choices: ["PNG", "JSON", "PDF", "XLSX"],
            correct: 1,
        },
        Question {
            prompt: "How can future content be expanded?",
            choices: [
                "By editing seeded video/question lists",
                "It cannot be expanded",
                "Only with paid DLC",
                "Only by reinstalling Rust",
            ],
            correct: 0,
        },
    ]
}
