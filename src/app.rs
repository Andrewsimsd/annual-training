use crate::certificate::{CertificateOperations, CertificateService};
use crate::quiz::{Question, QuizOperations, QuizService, seed_questions};
use axum::{
    Router,
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

const ENABLE_DEBUG_SKIP: bool = true;

#[derive(Clone)]
struct AppState {
    videos: Arc<Vec<Video>>,
    quizzes: Arc<Vec<Question>>,
    quiz_service: Arc<QuizService>,
    results: Arc<RwLock<HashMap<String, ExamAttempt>>>,
    cert_dir: Arc<PathBuf>,
    certificate_service: Arc<CertificateService>,
}

#[derive(Clone, Serialize)]
struct Video {
    title: &'static str,
    url: &'static str,
    description: &'static str,
}

#[derive(Clone)]
struct ExamAttempt {
    score: usize,
    total: usize,
    passed: bool,
    cert_id: Option<String>,
}

#[derive(Deserialize)]
struct QuizForm {
    employee_name: String,
    selected_question_ids: String,
    debug_skip: Option<String>,
    #[serde(flatten)]
    answers: HashMap<String, String>,
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
        quiz_service: Arc::new(QuizService),
        results: Arc::new(RwLock::new(HashMap::new())),
        cert_dir: Arc::new(cert_dir),
        certificate_service: Arc::new(CertificateService),
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
    let selected_questions = state.quiz_service.choose_questions(&state.quizzes);
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
    let selected_questions = state
        .quiz_service
        .select_questions_by_id(&state.quizzes, &payload.selected_question_ids);
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
        state.quiz_service.parse_answers(&payload.answers)
    };
    let evaluation = state
        .quiz_service
        .evaluate(&selected_questions, &parsed_answers);
    let employee_name = if skip_requested {
        "debug".to_owned()
    } else {
        payload.employee_name
    };
    let mut cert_id = None;
    if evaluation.passed {
        let id = Uuid::new_v4().to_string();
        let cert = state.certificate_service.build_certificate(
            &id,
            &employee_name,
            evaluation.score,
            evaluation.total,
        );
        if let Err(err) = state
            .certificate_service
            .write_certificate_files(&state.cert_dir, &cert)
            .await
        {
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

fn seed_videos() -> Vec<Video> {
    vec![
        Video {
            title: "Module 1: Lemme Smang it",
            url: "https://www.youtube.com/embed/xt5ghXdq6Z0",
            description: "The dangers of Smash-Bang-Fusion.",
        },
        Video {
            title: "Module 2: Liquid Slam",
            url: "https://www.youtube.com/embed/W7Hoz2ZHYZM",
            description: "Prevent your code base from becoming the liquid slam monster by practicing good repository hygiene.",
        },
        Video {
            title: "Module 3: Look how dat boy mind me!",
            url: "https://www.youtube.com/embed/7cqOEr_yfak",
            description: "Common traps in software testing.",
        },
        Video {
            title: "Module 4: Asset Acquisition",
            url: "https://www.youtube.com/embed/2aj-8lmB5q8",
            description: "The consequences of not providing a team with the assets they need to succeed.",
        },
    ]
}
