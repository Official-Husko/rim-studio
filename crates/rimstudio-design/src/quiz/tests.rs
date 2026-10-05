//! Tests of the quiz state machine on fictional pools.

use super::*;
use crate::baseline::BaselineConfig;
use crate::classes::synthetic::{SyntheticSpec, synthetic_items};
use crate::classes::{Pool, PoolItem, PoolOptions};

fn pool(n: usize, noise: f64, seed: u64) -> Pool {
    let items = synthetic_items(&SyntheticSpec {
        n,
        noise,
        seed,
        ..SyntheticSpec::default()
    });
    Pool::build(ItemKind::Ranged, &items, &PoolOptions::default())
}

/// A model fitted without item `t` and that item, to play the quiz truthfully.
fn split(n: usize, t: usize) -> (Model, PoolItem) {
    let full = pool(n, 0.08, 1);
    let target = full.items[t].clone();
    (
        Model::fit(full.without(&[t]), BaselineConfig::default()),
        target,
    )
}

fn bin_of(bins: &[Bin], v: f64) -> usize {
    bins.iter()
        .position(|b| b.lo.is_none_or(|l| v >= l) && b.hi.is_none_or(|h| v < h))
        .unwrap_or(0)
}

fn truthful(model: &Model, item: &PoolItem, q: &Question) -> Answer {
    let edge = model.config.bucket_edge;
    match q {
        Question::Tier { .. } => Answer::Tier { tier: item.tier },
        Question::Role { .. } => Answer::Role {
            role: item.role.clone(),
        },
        Question::Group { .. } => Answer::Group {
            group: item.group.clone().unwrap(),
        },
        Question::Compare { anchor, .. } => {
            let d = item.ln_strength - anchor.strength.ln();
            if d.abs() < model.config.same_band {
                Answer::Same
            } else if d > 0.0 {
                Answer::Stronger
            } else {
                Answer::Weaker
            }
        }
        Question::CloserTo { lower, upper } => {
            if item.ln_strength - lower.strength.ln() < upper.strength.ln() - item.ln_strength {
                Answer::CloserToLower
            } else {
                Answer::CloserToUpper
            }
        }
        Question::Interval { stat, bins, .. } => Answer::Bin {
            index: bin_of(bins, item.stat(stat).unwrap()),
        },
        Question::VsAnchor { stat, value, .. } => {
            let r = (item.stat(stat).unwrap() / value).ln();
            Answer::Bucket {
                bucket: if r < -edge {
                    Bucket::Lower
                } else if r > edge {
                    Bucket::Higher
                } else {
                    Bucket::Similar
                },
            }
        }
    }
}

fn play(model: &Model, item: &PoolItem) -> Quiz {
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    for _ in 0..30 {
        let Some(p) = quiz.next(model).unwrap() else {
            break;
        };
        let a = truthful(model, item, &p.question);
        quiz.answer(model, a).unwrap();
    }
    assert!(quiz.is_finished(model).unwrap());
    quiz
}

#[test]
fn first_questions_are_the_setup_taps() {
    let (m, _) = split(40, 5);
    let quiz = Quiz::for_kind(ItemKind::Ranged);
    let p = quiz.next(&m).unwrap().unwrap();
    assert_eq!(p.number, 1);
    assert_eq!(p.id, "tier");
    match p.question {
        Question::Tier { options } => {
            assert_eq!(options.iter().map(|o| o.count).sum::<usize>(), 39)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_truthful_player_finishes_within_the_caps() {
    for t in [3, 11, 20, 33] {
        let (m, item) = split(40, t);
        let quiz = play(&m, &item);
        let cmp = quiz
            .answers
            .iter()
            .filter(|a| a.question.starts_with("cmp:") || a.question.starts_with("closer:"))
            .count();
        assert!(cmp <= 3, "{cmp} comparisons");
        assert!(
            quiz.answers.len() <= 2 + 3 + 4,
            "{} answers",
            quiz.answers.len()
        );
    }
}

#[test]
fn the_dialogue_places_strength_near_the_truth() {
    let mut errors = Vec::new();
    for t in (2..38).step_by(3) {
        let (m, item) = split(40, t);
        let quiz = play(&m, &item);
        let input = quiz.input(&m).unwrap();
        let Some(StrengthInput::Placed { ln_power }) = input.strength else {
            panic!("no strength placed")
        };
        errors.push((ln_power - item.ln_strength).abs());
    }
    errors.sort_by(f64::total_cmp);
    let median = errors[errors.len() / 2];
    assert!(median < 0.25, "median placement error {median}");
}

#[test]
fn quiz_estimates_beat_the_class_median_on_strength_driven_stats() {
    let (mut quiz_err, mut median_err) = (Vec::new(), Vec::new());
    for t in (1..40).step_by(2) {
        let (m, item) = split(40, t);
        let quiz = play(&m, &item);
        let e = quiz.estimate(&m, None).unwrap();
        let truth = item.stat("mass").unwrap();
        quiz_err.push((e.value("mass").unwrap() - truth).abs() / truth);
        let class = m.class_stats(&ClassKey::of_item(&item));
        median_err.push((class.stats["mass"].median - truth).abs() / truth);
    }
    let med = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let (q, c) = (med(&mut quiz_err), med(&mut median_err));
    assert!(q < c, "quiz {q} class median {c}");
}

#[test]
fn same_ends_the_dialogue_early() {
    let (m, _) = split(40, 9);
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    quiz.answer(&m, Answer::Tier { tier: 1 }).unwrap();
    quiz.answer(
        &m,
        Answer::Role {
            role: "RS_role_a".into(),
        },
    )
    .unwrap();
    assert!(matches!(
        quiz.next(&m).unwrap().unwrap().question,
        Question::Compare { .. }
    ));
    quiz.answer(&m, Answer::Same).unwrap();
    let state = quiz.state(&m).unwrap();
    assert!(state.dialogue_done && state.placed.is_some());
    assert!(!matches!(
        quiz.next(&m).unwrap().unwrap().question,
        Question::Compare { .. }
    ));
}

#[test]
fn back_restores_the_previous_state() {
    let (m, item) = split(40, 14);
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    let mut snapshots = Vec::new();
    for _ in 0..5 {
        let p = quiz.next(&m).unwrap().unwrap();
        snapshots.push((quiz.clone(), p.id.clone()));
        let a = truthful(&m, &item, &p.question);
        quiz.answer(&m, a).unwrap();
    }
    while let Some((before, id)) = snapshots.pop() {
        let undone = quiz.back().unwrap();
        assert_eq!(undone.question, id);
        assert_eq!(quiz, before);
        assert_eq!(quiz.next(&m).unwrap().unwrap().id, id);
    }
    assert!(quiz.back().is_none());
}

#[test]
fn a_quiz_halfway_survives_a_json_round_trip() {
    let (m, item) = split(40, 21);
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    for _ in 0..4 {
        let p = quiz.next(&m).unwrap().unwrap();
        let a = truthful(&m, &item, &p.question);
        quiz.answer(&m, a).unwrap();
    }
    quiz.answer(&m, Answer::Typed { value: 3.5 }).ok();
    let json = serde_json::to_string(&quiz).unwrap();
    let back: Quiz = serde_json::from_str(&json).unwrap();
    assert_eq!(quiz, back);
    assert_eq!(quiz.next(&m).unwrap(), back.next(&m).unwrap());
    assert_eq!(
        serde_json::to_string(&quiz.estimate(&m, None).unwrap()).unwrap(),
        serde_json::to_string(&back.estimate(&m, None).unwrap()).unwrap()
    );
    // the answers are plain data a draft can store
    assert!(json.contains("\"answers\""));
}

#[test]
fn wrong_answers_are_rejected_and_leave_the_quiz_unchanged() {
    let (m, _) = split(40, 5);
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    let err = quiz.answer(&m, Answer::Stronger).unwrap_err();
    assert_eq!(err.code(), "quiz.wrong-answer");
    let err = quiz.answer(&m, Answer::Tier { tier: 77 }).unwrap_err();
    assert_eq!(err.code(), "quiz.unknown-choice");
    let err = quiz.answer(&m, Answer::Typed { value: 1.0 }).unwrap_err();
    assert_eq!(err.code(), "quiz.wrong-answer");
    assert!(quiz.answers.is_empty());
}

#[test]
fn skip_not_sure_and_use_what_i_have_never_block() {
    let (m, _) = split(40, 5);
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    quiz.answer(&m, Answer::Skip).unwrap();
    quiz.answer(&m, Answer::NotSure).unwrap();
    let state = quiz.state(&m).unwrap();
    assert!(state.key.tier.is_none() && state.key.role.is_none());
    // the estimate exists with nothing answered
    let e = quiz.estimate(&m, None).unwrap();
    assert!(e.stats.values().all(|s| s.value.is_some()));
    quiz.use_what_i_have(&m).unwrap();
    assert!(quiz.is_finished(&m).unwrap());
    assert_eq!(
        quiz.answer(&m, Answer::Skip).unwrap_err().code(),
        "quiz.finished"
    );
    quiz.use_what_i_have(&m).unwrap();
}

#[test]
fn typed_values_become_typed_constraints_and_are_kept() {
    let (m, item) = split(40, 17);
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    loop {
        let p = quiz.next(&m).unwrap().unwrap();
        if matches!(
            p.question,
            Question::Interval { .. } | Question::VsAnchor { .. }
        ) {
            quiz.answer(&m, Answer::Typed { value: 4.25 }).unwrap();
            break;
        }
        let a = truthful(&m, &item, &p.question);
        quiz.answer(&m, a).unwrap();
    }
    let e = quiz.estimate(&m, None).unwrap();
    let typed: Vec<_> = e
        .stats
        .iter()
        .filter(|(_, s)| s.source == Some(crate::baseline::Source::Typed))
        .collect();
    assert_eq!(typed.len(), 1);
    assert_eq!(typed[0].1.value, Some(4.25));
}

#[test]
fn a_changed_pool_makes_stored_answers_stale() {
    let (m, item) = split(40, 8);
    let quiz = play(&m, &item);
    let other = Model::fit(pool(25, 0.2, 9), BaselineConfig::default());
    let err = quiz.state(&other).unwrap_err();
    assert_eq!(err.code(), "quiz.stale");
    let mut cut = quiz.clone();
    let dropped = cut.truncate_stale(&other);
    assert!(dropped > 0 && cut.answers.len() < quiz.answers.len());
    assert!(cut.state(&other).is_ok());
}

#[test]
fn next_question_is_deterministic() {
    let (m, item) = split(40, 12);
    let a = play(&m, &item);
    let b = play(&m, &item);
    assert_eq!(a, b);
}

#[test]
fn pivot_has_the_highest_information_and_prefers_the_middle_on_ties() {
    let (m, _) = split(40, 5);
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    quiz.answer(&m, Answer::Tier { tier: 1 }).unwrap();
    quiz.answer(
        &m,
        Answer::Role {
            role: "RS_role_b".into(),
        },
    )
    .unwrap();
    let state = quiz.state(&m).unwrap();
    let Some(Question::Compare {
        anchor,
        information,
        remaining,
        ..
    }) = quiz.next_question(&m, &state)
    else {
        panic!("expected a comparison");
    };
    assert_eq!(remaining, m.pool.len());
    // the information of any other candidate does not exceed the chosen one
    let list: Vec<usize> = (0..m.pool.len()).collect();
    let ln = Quiz::list_ln(&m, &list);
    let bracket = Bracket {
        lo: -1,
        hi: ln.len() as i64,
    };
    let (k, h) = quiz.pick_pivot(&m, &state, &ln, bracket).unwrap();
    assert_eq!(m.pool.items[k].id, anchor.id);
    assert!((h - information).abs() < 1e-12);
    assert!(
        information > 0.8 && information <= 3.0f64.ln() + 1e-9,
        "{information}"
    );
    // the pivot sits near the prior median: it is not at either end
    assert!(k > 3 && k + 4 < m.pool.len(), "pivot {k}");
}

#[test]
fn flat_stats_are_not_asked() {
    let items: Vec<_> = synthetic_items(&SyntheticSpec {
        n: 30,
        ..SyntheticSpec::default()
    })
    .into_iter()
    .map(|mut it| {
        it.stats.insert("warmup".into(), 1.0);
        it.stats.insert("range".into(), 25.0);
        it
    })
    .collect();
    let m = Model::fit(
        Pool::build(ItemKind::Ranged, &items, &PoolOptions::default()),
        BaselineConfig::default(),
    );
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    let mut asked = Vec::new();
    for _ in 0..30 {
        let Some(p) = quiz.next(&m).unwrap() else {
            break;
        };
        asked.push(p.id.clone());
        let a = match &p.question {
            Question::Tier { options } => Answer::Tier {
                tier: options[0].tier,
            },
            Question::Role { options } => Answer::Role {
                role: options[0].name.clone(),
            },
            Question::Compare { .. } => Answer::Same,
            Question::Group { options } => Answer::Group {
                group: options[0].name.clone(),
            },
            _ => Answer::Skip,
        };
        quiz.answer(&m, a).unwrap();
    }
    assert!(
        !asked
            .iter()
            .any(|id| id.starts_with("bin:warmup") || id.starts_with("bin:range")),
        "{asked:?}"
    );
    assert!(
        asked.iter().any(|id| id.starts_with("vs:mass")),
        "{asked:?}"
    );
}

#[test]
fn progress_total_is_never_below_the_current_number() {
    let (m, item) = split(40, 25);
    let mut quiz = Quiz::for_kind(ItemKind::Ranged);
    while let Some(p) = quiz.next(&m).unwrap() {
        assert!(
            p.about_total >= p.number,
            "{} < {}",
            p.about_total,
            p.number
        );
        assert!(p.about_total <= 12);
        let a = truthful(&m, &item, &p.question);
        quiz.answer(&m, a).unwrap();
    }
}

#[test]
fn apparel_scope_uses_the_role_list_and_a_larger_budget() {
    let items = synthetic_items(&SyntheticSpec {
        n: 60,
        kind: ItemKind::Apparel,
        ..SyntheticSpec::default()
    });
    let m = Model::fit(
        Pool::build(ItemKind::Apparel, &items, &PoolOptions::default()),
        BaselineConfig::default(),
    );
    let mut quiz = Quiz::for_kind(ItemKind::Apparel);
    assert_eq!(quiz.config.comparison_budget, 5);
    quiz.answer(&m, Answer::Tier { tier: 2 }).unwrap();
    quiz.answer(
        &m,
        Answer::Role {
            role: "RS_role_c".into(),
        },
    )
    .unwrap();
    let state = quiz.state(&m).unwrap();
    let list = quiz.comparison_list(&m, &state.key);
    assert_eq!(list.len(), 20);
    assert!(list.iter().all(|p| m.pool.items[*p].role == "RS_role_c"));
}

#[test]
fn bins_have_round_edges_and_labels() {
    let (m, _) = split(40, 5);
    let class = m.class_stats(&ClassKey::default());
    let bins = bins_for(&class.stats["mass"]);
    assert_eq!(bins.len(), 4);
    assert!(bins[0].lo.is_none() && bins[3].hi.is_none());
    assert!(bins[0].label.starts_with("under ") && bins[3].label.starts_with("over "));
    assert!(bins[1].label.contains(" to "));
    assert!(bins.windows(2).all(|w| w[0].hi == w[1].lo));
    assert_eq!(round_sig(12.3456, 2), 12.0);
    assert_eq!(round_sig(0.012_34, 2), 0.012);
}

#[test]
fn normal_cdf_matches_known_values() {
    assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
    assert!((normal_cdf(1.0) - 0.841_344_7).abs() < 1e-5);
    assert!((normal_cdf(-1.96) - 0.025).abs() < 1e-4);
    assert!(normal_cdf(8.0) > 0.999_999 && normal_cdf(-8.0) < 1e-6);
}

#[test]
fn free_apply_matches_the_method() {
    let (m, _) = split(40, 5);
    let mut a = Quiz::for_kind(ItemKind::Ranged);
    let mut b = a.clone();
    a.answer(&m, Answer::Skip).unwrap();
    apply(&mut b, &m, Answer::Skip).unwrap();
    assert_eq!(a, b);
}

mod random_play {
    use super::*;
    use proptest::prelude::*;

    /// Every answer that is valid for a question, including the always available ones.
    fn valid_answers(q: &Question) -> Vec<Answer> {
        let mut out = vec![Answer::NotSure, Answer::Skip, Answer::UseWhatIHave];
        match q {
            Question::Tier { options } => {
                out.extend(options.iter().map(|o| Answer::Tier { tier: o.tier }))
            }
            Question::Role { options } => out.extend(options.iter().map(|o| Answer::Role {
                role: o.name.clone(),
            })),
            Question::Group { options } => out.extend(options.iter().map(|o| Answer::Group {
                group: o.name.clone(),
            })),
            Question::Compare { .. } => {
                out.extend([Answer::Weaker, Answer::Same, Answer::Stronger])
            }
            Question::CloserTo { .. } => out.extend([Answer::CloserToLower, Answer::CloserToUpper]),
            Question::Interval { bins, .. } => {
                out.extend((0..bins.len()).map(|index| Answer::Bin { index }));
                out.push(Answer::Typed { value: 7.5 });
            }
            Question::VsAnchor { .. } => out.extend([
                Answer::Bucket {
                    bucket: Bucket::Lower,
                },
                Answer::Bucket {
                    bucket: Bucket::Similar,
                },
                Answer::Bucket {
                    bucket: Bucket::Higher,
                },
                Answer::Typed { value: 2.5 },
            ]),
        }
        out
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        #[test]
        fn any_valid_answer_sequence_keeps_the_quiz_consistent(
            picks in proptest::collection::vec((0usize..64, any::<bool>()), 1..14),
            seed in 1u64..6,
        ) {
            let items = synthetic_items(&SyntheticSpec { n: 36, seed, ..SyntheticSpec::default() });
            let m = Model::fit(Pool::build(ItemKind::Ranged, &items, &PoolOptions::default()), BaselineConfig::default());
            let mut quiz = Quiz::for_kind(ItemKind::Ranged);
            for (pick, go_back) in picks {
                if go_back && !quiz.answers.is_empty() {
                    let before = quiz.answers.len();
                    quiz.back();
                    prop_assert_eq!(quiz.answers.len(), before - 1);
                }
                let Some(p) = quiz.next(&m).unwrap() else { break };
                prop_assert!(p.about_total >= p.number);
                let options = valid_answers(&p.question);
                let a = options[pick % options.len()].clone();
                quiz.answer(&m, a).unwrap();
                // the state replays and the estimate is always available and finite
                let e = quiz.estimate(&m, None).unwrap();
                for (name, s) in &e.stats {
                    prop_assert!(s.value.is_some_and(f64::is_finite), "{name}");
                }
                let json = serde_json::to_string(&quiz).unwrap();
                let back: Quiz = serde_json::from_str(&json).unwrap();
                prop_assert_eq!(back.answers.len(), quiz.answers.len());
                prop_assert!(back.state(&m).is_ok());
            }
        }
    }
}
