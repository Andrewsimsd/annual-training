use crate::certificate::{build_certificate, write_certificate_files};
use crate::quiz::{
    Question, choose_quiz_questions, evaluate_quiz, parse_answers, seed_questions,
    select_questions_by_id,
};
use axum::{
    Router,
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
    let _ = webbrowser::open(&app_url);
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
    Html(format!(
        "<h1>Software Development Annual Training</h1>{}<a href='/quiz'>Proceed to Development Practices Quiz</a>",
        state
            .videos
            .iter()
            .map(|v| format!(
                "<section><h3>{}</h3><p>{}</p><iframe src='{}'></iframe></section>",
                v.title, v.description, v.url
            ))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

async fn quiz_page(State(state): State<AppState>) -> Html<String> {
    let selected_questions = choose_quiz_questions(&state.quizzes);
    let selected_ids = selected_questions
        .iter()
        .map(|question| question.id)
        .collect::<Vec<_>>()
        .join(",");
    let question_markup = selected_questions.iter().enumerate().map(|(idx, q)| {
        let options = q.choices.iter().enumerate().map(|(choice_idx, choice)| format!("<label><input required type='radio' name='q{idx}' value='{choice_idx}'> {choice}</label><br>")).collect::<String>();
        format!("<fieldset><legend><strong>Q{}:</strong> {}</legend>{}</fieldset><br>", idx + 1, q.prompt, options)
    }).collect::<String>();
    let skip_button = if ENABLE_DEBUG_SKIP {
        "<button type='submit' name='debug_skip' value='1' formnovalidate style='margin-left: .75rem;'>Skip (Debug)</button>"
    } else {
        ""
    };
    Html(format!(
        "<h1>Software Development Knowledge Check</h1><form method='post' action='/quiz'><label>Your full name: <input type='text' name='employee_name' required></label><br><br><input type='hidden' name='selected_question_ids' value='{}'>{}<button type='submit'>Submit Exam</button>{}</form>",
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
        if write_certificate_files(&state.cert_dir, &cert)
            .await
            .is_ok()
        {
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
    let Some(result) = state.results.read().await.get(ticket).cloned() else {
        return Html("<h1>Unknown result ticket.</h1>".to_string());
    };
    let pct = (result.score as f32 / result.total as f32) * 100.0;
    let status = if result.passed { "PASS" } else { "FAIL" };
    let cert_msg = if let Some(cert_id) = result.cert_id {
        format!(
            "<a href='/certificate/{cert_id}' download>Download completion certificate (.pdf)</a>"
        )
    } else {
        "<p>No certificate issued.</p>".to_string()
    };
    Html(format!(
        "<h1>Exam Result: {status}</h1><p>Score: {}/{} ({pct:.1}%)</p>{}",
        result.score, result.total, cert_msg
    ))
}

async fn download_certificate(
    State(state): State<AppState>,
    Path(cert_id): Path<String>,
) -> Response {
    let path = state.cert_dir.join(format!("certificate-{cert_id}.pdf"));
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let mut res = Response::new(axum::body::Body::from(bytes));
            *res.status_mut() = StatusCode::OK;
            res.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/pdf"),
            );
            if let Ok(value) = HeaderValue::from_str(&format!(
                "attachment; filename=\"certificate-{cert_id}.pdf\""
            )) {
                res.headers_mut().insert(header::CONTENT_DISPOSITION, value);
            }
            res
        }
        Err(_) => (StatusCode::NOT_FOUND, "Certificate not found").into_response(),
    }
}

fn seed_videos() -> Vec<Video> {
    vec![Video {
        title: "Module 1: Lemme Smang it",
        url: "https://www.youtube.com/embed/xt5ghXdq6Z0",
        description: "The dangers of Smash-Bang-Fusion.",
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seed_questions_not_empty() {
        assert!(!seed_questions().is_empty());
    }
}
