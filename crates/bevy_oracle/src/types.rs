use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The API takes at most this many embedded images.
pub const MAX_IMAGES: usize = 4;
/// The API takes 1 to this many questions.
pub const MAX_QUESTIONS: usize = 64;

/// One typed question. Serialized as the API's `{"type": ..., "instructions": ..., "criteria": ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Yes or no; the answer is P(yes).
    Noul { instructions: String },
    /// Named options (name -> what it means); the answer is each option's probability.
    Choice { instructions: String, criteria: BTreeMap<String, String> },
    /// Levels on a scale, lowest first; the answer is a probability-weighted score.
    Score { instructions: String, criteria: Vec<String> },
}

impl Question {
    pub fn noul(instructions: impl Into<String>) -> Self {
        Question::Noul { instructions: instructions.into() }
    }
    pub fn choice<K: Into<String>, V: Into<String>>(
        instructions: impl Into<String>,
        options: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        Question::Choice {
            instructions: instructions.into(),
            criteria: options.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        }
    }
    pub fn score<S: Into<String>>(
        instructions: impl Into<String>,
        levels: impl IntoIterator<Item = S>,
    ) -> Self {
        Question::Score {
            instructions: instructions.into(),
            criteria: levels.into_iter().map(Into::into).collect(),
        }
    }

    fn validate(&self, id: &str) -> Result<(), Error> {
        match self {
            Question::Choice { criteria, .. } if criteria.len() < 2 => {
                Err(Error::Invalid(format!("question `{id}`: a choice needs at least two options")))
            }
            Question::Score { criteria, .. } if criteria.len() < 2 => {
                Err(Error::Invalid(format!("question `{id}`: a score needs at least two levels")))
            }
            _ => Ok(()),
        }
    }
}

/// A request body. `images` are data URLs or URLs, as the endpoint takes them. `model` is omitted when
/// `None`: a llama.cpp server has one model, Cloudflare needs it named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    pub questions: BTreeMap<String, Question>,
}

impl Request {
    pub fn new(state: impl Into<String>) -> Self {
        Request { model: None, state: state.into(), images: Vec::new(), questions: BTreeMap::new() }
    }
    pub fn with_image(mut self, image: impl Into<String>) -> Self {
        self.images.push(image.into());
        self
    }
    pub fn with_question(mut self, id: impl Into<String>, q: Question) -> Self {
        self.questions.insert(id.into(), q);
        self
    }

    /// The API's limits, checked before anything is sent.
    pub fn validate(&self) -> Result<(), Error> {
        if self.questions.is_empty() || self.questions.len() > MAX_QUESTIONS {
            return Err(Error::Invalid(format!(
                "{} questions; the API takes 1 to {MAX_QUESTIONS}",
                self.questions.len()
            )));
        }
        if self.images.len() > MAX_IMAGES {
            return Err(Error::Invalid(format!(
                "{} images; the API takes at most {MAX_IMAGES}",
                self.images.len()
            )));
        }
        self.questions.iter().try_for_each(|(id, q)| q.validate(id))
    }
}

/// One question's answer, with a probability for every allowed option.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Noul {
        /// P(yes).
        noul: f64,
    },
    Choice {
        /// The most probable option.
        choice: String,
        probabilities: BTreeMap<String, f64>,
        #[serde(default)]
        confidence: f64,
    },
    Score {
        /// The probability-weighted level index.
        score: f64,
        /// Level index -> level name.
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
        #[serde(default)]
        confidence: f64,
    },
}

impl Answer {
    /// P(yes) of a `noul` answer.
    pub fn p_yes(&self) -> Option<f64> {
        match self {
            Answer::Noul { noul } => Some(*noul),
            _ => None,
        }
    }
    /// The probability of one `choice` option.
    pub fn p_option(&self, name: &str) -> Option<f64> {
        match self {
            Answer::Choice { probabilities, .. } => probabilities.get(name).copied(),
            _ => None,
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Answer::Noul { .. } => "noul",
            Answer::Choice { .. } => "choice",
            Answer::Score { .. } => "score",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

/// A response body: answers keyed by question id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub answers: BTreeMap<String, Answer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

impl Response {
    /// Every question has an answer of the type asked, and a choice's options are the ones offered.
    pub(crate) fn check_answers(&self, request: &Request, endpoint: &str) -> Result<(), Error> {
        let bad = |detail: String| Error::BadResponse { endpoint: endpoint.to_string(), detail };
        for (id, q) in &request.questions {
            let a = self.answers.get(id).ok_or_else(|| bad(format!("no answer for question `{id}`")))?;
            let want = match q {
                Question::Noul { .. } => "noul",
                Question::Choice { .. } => "choice",
                Question::Score { .. } => "score",
            };
            if a.kind() != want {
                return Err(bad(format!("question `{id}` is a {want} but was answered as a {}", a.kind())));
            }
            if let (Question::Choice { criteria, .. }, Answer::Choice { probabilities, .. }) = (q, a)
                && let Some(extra) = probabilities.keys().find(|k| !criteria.contains_key(*k))
            {
                return Err(bad(format!("question `{id}` answered with unknown option `{extra}`")));
            }
        }
        Ok(())
    }
}
