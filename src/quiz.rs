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

#[allow(
    clippy::too_many_lines,
    reason = "Keeping complete seed content together preserves training bank readability and expected behavior."
)]
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
