//! The quiz: one question at a time, answers that calibrate the suggestions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::draft::DraftDto;
use super::preview::{AnchorCardDto, EstimateSummaryDto};

/// A tier choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierOptionDto {
    /// Tier index, zero for neolithic.
    pub tier: u8,
    /// Display label.
    pub label: String,
    /// Reference weapons at the tier.
    pub count: u32,
}

/// A named choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedOptionDto {
    /// The name.
    pub name: String,
    /// Reference weapons with the name.
    pub count: u32,
}

/// One bin of an interval question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinDto {
    /// Lower bound. Absent for the open bottom bin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lo: Option<f64>,
    /// Upper bound. Absent for the open top bin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hi: Option<f64>,
    /// Display label.
    pub label: String,
}

/// A question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum QuestionDto {
    /// Which tech tier is the weapon.
    Tier {
        /// The choices.
        options: Vec<TierOptionDto>,
    },
    /// Which role is the weapon.
    Role {
        /// The choices.
        options: Vec<NamedOptionDto>,
    },
    /// Is the weapon weaker, the same or stronger than the anchor.
    Compare {
        /// The anchor.
        anchor: AnchorCardDto,
        /// Comparisons left in the budget.
        remaining: u32,
        /// Comparisons asked so far.
        asked: u32,
        /// Comparison budget.
        budget: u32,
        /// Expected information gain of this question, in bits.
        information: f64,
    },
    /// Is the weapon closer to the lower or the upper anchor.
    CloserTo {
        /// The lower anchor.
        lower: AnchorCardDto,
        /// The upper anchor.
        upper: AnchorCardDto,
    },
    /// Which group is the weapon in.
    Group {
        /// The choices.
        options: Vec<NamedOptionDto>,
    },
    /// In which bin does a stat fall.
    Interval {
        /// Stat name.
        stat: String,
        /// The bins, in ascending order.
        bins: Vec<BinDto>,
        /// Spread of the current estimate of the stat.
        spread: f64,
    },
    /// Is the stat lower, similar or higher than the anchor's value.
    VsAnchor {
        /// Stat name.
        stat: String,
        /// The anchor.
        anchor: AnchorCardDto,
        /// The anchor's value of the stat.
        value: f64,
    },
}

/// The open prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDto {
    /// Stable question id; the answer names it.
    pub id: String,
    /// The question.
    pub question: QuestionDto,
    /// One based number of the question.
    pub number: u32,
    /// Expected total number of questions.
    pub about_total: u32,
}

/// Bucket of a compared value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BucketDto {
    /// Lower than the anchor.
    Lower,
    /// Similar to the anchor.
    Similar,
    /// Higher than the anchor.
    Higher,
}

/// An answer to the open question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum QuizAnswerDto {
    /// Chose a tier.
    Tier {
        /// Tier index.
        tier: u8,
    },
    /// Chose a role.
    Role {
        /// Role name.
        role: String,
    },
    /// Chose a group.
    Group {
        /// Group name.
        group: String,
    },
    /// Weaker than the anchor.
    Weaker,
    /// The same as the anchor.
    Same,
    /// Stronger than the anchor.
    Stronger,
    /// Closer to the lower anchor.
    CloserToLower,
    /// Closer to the upper anchor.
    CloserToUpper,
    /// Chose a bin.
    Bin {
        /// Index of the bin.
        index: u32,
    },
    /// Chose a bucket.
    Bucket {
        /// The bucket.
        bucket: BucketDto,
    },
    /// Typed a value.
    Typed {
        /// The value.
        value: f64,
    },
    /// Not sure.
    NotSure,
    /// Skip the question.
    Skip,
    /// Stop asking and use what is known.
    UseWhatIHave,
}

/// Response of `designer_quiz_next` and part of the answer response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizStepDto {
    /// The open prompt. Absent when the quiz is finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<PromptDto>,
    /// True when no further question will be asked.
    pub finished: bool,
    /// Questions answered so far.
    pub answered: u32,
    /// The estimate after the answers so far. Absent when no reference set is loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<EstimateSummaryDto>,
    /// Constraints implied by the answers, by stat name; text for display. Omitted when empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub implied: BTreeMap<String, String>,
}

/// Request of `designer_quiz_next`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignerQuizNextRequest {
    /// The draft with its stored answers.
    pub draft: DraftDto,
}

/// Request of `designer_quiz_answer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignerQuizAnswerRequest {
    /// The draft with its stored answers.
    pub draft: DraftDto,
    /// Id of the question being answered.
    pub question_id: String,
    /// The answer.
    pub answer: QuizAnswerDto,
}

/// Request of `designer_quiz_back`: take the last answer of the draft's quiz back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignerQuizBackRequest {
    /// The draft with its stored answers.
    pub draft: DraftDto,
}

/// Response of `designer_quiz_answer` and `designer_quiz_back`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignerQuizAnswerResponse {
    /// The draft with the answer stored and implied values applied (typed values untouched).
    pub draft: DraftDto,
    /// The next step.
    pub step: QuizStepDto,
}
