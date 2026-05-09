use std::collections::HashMap;

const PASS_THRESHOLD: f32 = 0.80;

#[derive(Clone)]
pub(crate) struct Question {
    pub(crate) id: &'static str,
    pub(crate) prompt: &'static str,
    pub(crate) choices: [&'static str; 4],
    pub(crate) correct: usize,
}

#[derive(Debug)]
pub(crate) struct QuizEvaluation {
    pub(crate) score: usize,
    pub(crate) total: usize,
    pub(crate) passed: bool,
}

pub(crate) fn parse_answers(raw_answers: &HashMap<String, String>) -> HashMap<String, usize> {
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

pub(crate) fn choose_quiz_questions(question_bank: &[Question]) -> Vec<Question> {
    question_bank.to_vec()
}

pub(crate) fn select_questions_by_id(question_bank: &[Question], raw_ids: &str) -> Vec<Question> {
    raw_ids
        .split(',')
        .filter_map(|id| question_bank.iter().find(|question| question.id == id))
        .cloned()
        .collect()
}

pub(crate) fn evaluate_quiz(
    questions: &[Question],
    answers: &HashMap<String, usize>,
) -> QuizEvaluation {
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

pub(crate) fn seed_questions() -> Vec<Question> {
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
    ]
}
