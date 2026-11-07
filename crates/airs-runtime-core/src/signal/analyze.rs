//! Versioned deterministic analysis; no network, credentials or inferred analyst judgments.
use super::evidence::{Evidence, Status};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Group {
    pub app_id: String,
    pub app_name: String,
    pub profile: Option<String>,
    pub detector: String,
    pub direction: String,
    pub action: String,
}
#[derive(Clone, Debug)]
pub struct Finding {
    pub id: String,
    pub group: Group,
    pub rows: Vec<(usize, usize)>,
    pub event_indices: Vec<usize>,
    pub metadata_observations: usize,
    pub events: usize,
    pub scans: usize,
    pub conflicting: bool,
    pub rationale: String,
    pub recommendation: String,
}
pub struct Analysis {
    pub findings: Vec<Finding>,
    pub metadata_observations: usize,
    pub eligible_events: usize,
    pub distinct_scans: usize,
    pub violation_rows: usize,
    pub unknown_time_events: usize,
    pub outside_window_events: usize,
    pub nonviolation_rows: usize,
    pub required_incomplete: usize,
    pub ambiguous_rows: usize,
}
fn detector_rank(detector: &str) -> u8 {
    let name = detector.to_ascii_lowercase();
    if name.contains("dlp") {
        0
    } else if name.contains("injection") || name == "pi" {
        1
    } else if name.contains("database") || name == "dbs" {
        2
    } else {
        3
    }
}
pub fn analyze(e: &Evidence) -> Analysis {
    if e.rules_version == 1 {
        analyze_legacy(e)
    } else {
        analyze_metadata(e)
    }
}
fn analyze_legacy(e: &Evidence) -> Analysis {
    let mut groups: BTreeMap<Group, Vec<(usize, usize)>> = BTreeMap::new();
    let mut owners: BTreeMap<(&str, u64), BTreeSet<&super::evidence::Identity>> = BTreeMap::new();
    for event in &e.events {
        for d in &event.detections {
            owners
                .entry((&d.report_id, d.row))
                .or_default()
                .insert(&event.identity);
        }
    }
    let ambiguous_rows = owners.values().filter(|set| set.len() > 1).count();
    let mut eligible = 0;
    let mut scans = BTreeSet::new();
    let mut unknown = 0;
    let mut outside = 0;
    let mut other = 0;
    for (i, event) in e.events.iter().enumerate() {
        match event.timestamp_ms {
            None => {
                unknown += 1;
                continue;
            }
            Some(at) if at < e.window.start_ms || at >= e.window.end_ms => {
                outside += 1;
                continue;
            }
            _ => {}
        }
        eligible += 1;
        scans.insert(&event.identity.scan_id);
        for (j, d) in event.detections.iter().enumerate() {
            if owners
                .get(&(d.report_id.as_str(), d.row))
                .is_some_and(|set| set.len() > 1)
            {
                continue;
            }
            if d.verdict != "malicious" {
                other += 1;
                continue;
            }
            let group = Group {
                app_id: event.identity.app_id.clone(),
                app_name: event.identity.app_name.clone(),
                profile: event.profile.clone(),
                detector: d.detector.clone(),
                direction: d.direction.clone(),
                action: event.action.clone(),
            };
            groups.entry(group).or_default().push((i, j));
        }
    }
    let mut findings = Vec::new();
    let mut row_count = 0;
    for (group, rows) in groups {
        row_count += rows.len();
        let event_ids: BTreeSet<_> = rows.iter().map(|(i, _)| *i).collect();
        let scan_ids: BTreeSet<_> = rows
            .iter()
            .map(|(i, _)| &e.events[*i].identity.scan_id)
            .collect();
        let conflict = rows.iter().any(|(i, j)| {
            let event = &e.events[*i];
            let d = &event.detections[*j];
            (matches!(event.action.as_str(), "allow" | "block")
                && matches!(d.action.as_str(), "allow" | "block")
                && event.action != d.action)
                || event.verdict == "benign"
        });
        let rationale = format!(
            "{} collected event(s), {} distinct scan(s), {} malicious detector row(s). These counts describe collected evidence, not an extrapolation to all traffic.",
            event_ids.len(),
            scan_ids.len(),
            rows.len()
        );
        let recommendation = recommendation(e, &group, conflict);
        let keys = serde_json::json!([
            group.app_id,
            group.app_name,
            group.profile,
            group.detector,
            group.direction,
            group.action
        ]);
        let id = format!(
            "finding-{:x}",
            Sha256::digest(serde_json::to_vec(&keys).expect("string-only keys"))
        );
        findings.push(Finding {
            id,
            group,
            event_indices: event_ids.iter().copied().collect(),
            metadata_observations: 0,
            events: event_ids.len(),
            scans: scan_ids.len(),
            rows,
            conflicting: conflict,
            rationale,
            recommendation,
        });
    }
    findings.sort_by(|a, b| {
        detector_rank(&a.group.detector)
            .cmp(&detector_rank(&b.group.detector))
            .then(b.events.cmp(&a.events))
            .then(a.group.cmp(&b.group))
    });
    Analysis {
        findings,
        metadata_observations: 0,
        eligible_events: eligible,
        ambiguous_rows,
        distinct_scans: scans.len(),
        violation_rows: row_count,
        unknown_time_events: unknown,
        outside_window_events: outside,
        nonviolation_rows: other,
        required_incomplete: e
            .sources
            .iter()
            .filter(|s| s.required && s.status != Status::Complete)
            .count(),
    }
}

fn recommendation(e: &Evidence, group: &Group, conflict: bool) -> String {
    let profile = group.profile.as_ref().and_then(|name| {
        e.profiles
            .iter()
            .filter(|p| &p.name == name)
            .max_by_key(|p| p.revision)
    });
    let configuration = profile.and_then(|p| {
        let matches: Vec<_> = p
            .protections
            .iter()
            .filter(|p| {
                (if e.rules_version >= 2 {
                    super::detectors::canonical(&p.detector)
                        == super::detectors::canonical(&group.detector)
                } else {
                    p.detector == group.detector
                }) && p.direction == group.direction
                    && p.active
                    && p.confidence == "explicit"
            })
            .collect();
        let actions: BTreeSet<_> = matches.iter().map(|p| p.action.as_str()).collect();
        if actions.len() == 1 {
            matches.first().copied()
        } else {
            None
        }
    });
    if conflict {
            "Reconcile conflicting scan and detector observations before making an enforcement recommendation."
        } else if group.action=="allow" {
            if configuration.is_some_and(|c|c.action=="allow") {
                "Current profile settings support a monitoring interpretation. Review the sampled payloads and application impact before deciding whether this protection should block. Current settings do not prove the event-time policy."
            } else {
                "Review allowed detections and verify the effective event-time policy. Allowance can be intentional monitoring; it does not establish a misconfiguration."
            }
        } else if group.action=="block" {
            "Review blocked detections for recurring patterns and possible application impact. Blocking alone does not establish a confirmed attack or a false positive."
        } else {"Resolve the missing or unfamiliar scan enforcement action before making an enforcement recommendation."}.to_owned()
}

/// V2 findings originate in result flags/classifications, never optional report rows.
fn analyze_metadata(e: &Evidence) -> Analysis {
    let mut a = analyze_legacy(e);
    a.findings.clear();
    let mut groups: BTreeMap<Group, BTreeSet<usize>> = BTreeMap::new();
    for (i, event) in e.events.iter().enumerate() {
        if !event
            .timestamp_ms
            .is_some_and(|at| at >= e.window.start_ms && at < e.window.end_ms)
        {
            continue;
        }
        let mut hints = super::detectors::observation_hints(&event.detector_hints);
        if hints.is_empty() && event.verdict == "malicious" {
            hints.insert(("unknown", "unknown"));
        }
        for (direction, detector) in hints {
            groups
                .entry(Group {
                    app_id: event.identity.app_id.clone(),
                    app_name: event.identity.app_name.clone(),
                    profile: event.profile.clone(),
                    detector: detector.into(),
                    direction: direction.into(),
                    action: event.action.clone(),
                })
                .or_default()
                .insert(i);
        }
    }
    let mut owners: BTreeMap<(&str, u64), BTreeSet<&super::evidence::Identity>> = BTreeMap::new();
    for event in &e.events {
        for d in &event.detections {
            owners
                .entry((&d.report_id, d.row))
                .or_default()
                .insert(&event.identity);
        }
    }
    for (group, indices) in groups {
        let rows: Vec<_> = indices
            .iter()
            .flat_map(|i| {
                e.events[*i]
                    .detections
                    .iter()
                    .enumerate()
                    .filter_map(|(j, d)| {
                        let attributable = owners
                            .get(&(d.report_id.as_str(), d.row))
                            .is_some_and(|set| set.len() == 1);
                        (attributable
                            && d.verdict == "malicious"
                            && (group.detector == "unknown"
                                || (super::detectors::canonical(&d.detector) == group.detector
                                    && (group.direction == "unknown"
                                        || d.direction == group.direction))))
                            .then_some((*i, j))
                    })
            })
            .collect();
        let conflicting = indices.iter().any(|i| e.events[*i].verdict == "benign")
            || rows.iter().any(|(i, j)| {
                let event = &e.events[*i];
                let d = &event.detections[*j];
                matches!(event.action.as_str(), "allow" | "block")
                    && matches!(d.action.as_str(), "allow" | "block")
                    && event.action != d.action
            });
        let scans = indices
            .iter()
            .map(|i| &e.events[*i].identity.scan_id)
            .collect::<BTreeSet<_>>()
            .len();
        let observations = if group.detector == "unknown" {
            0
        } else {
            indices.len()
        };
        a.metadata_observations += observations;
        let rationale = format!(
            "{} collected event(s), {} distinct scan(s), {} detector-positive metadata observation(s). {} detailed detector row(s) provide optional examples. Counts describe collected evidence, not all tenant traffic.{}",
            indices.len(),
            scans,
            observations,
            rows.len(),
            if group.detector == "unknown" {
                if indices.iter().all(|i| e.events[*i].verdict == "malicious") {
                    " Scan classification is malicious; detector and direction are unavailable."
                } else {
                    " Detector identity is unavailable in positive metadata; scan classification and enforcement are not inferred."
                }
            } else {
                ""
            }
        );
        let keys = serde_json::json!([
            group.app_id,
            group.app_name,
            group.profile,
            group.detector,
            group.direction,
            group.action
        ]);
        let id = format!(
            "finding-{:x}",
            Sha256::digest(serde_json::to_vec(&keys).expect("string-only keys"))
        );
        let recommendation = recommendation(e, &group, conflicting);
        a.findings.push(Finding {
            id,
            group,
            events: indices.len(),
            scans,
            rows,
            event_indices: indices.into_iter().collect(),
            metadata_observations: observations,
            conflicting,
            rationale,
            recommendation,
        });
    }
    a.findings.sort_by(|a, b| {
        detector_rank(&a.group.detector)
            .cmp(&detector_rank(&b.group.detector))
            .then(b.events.cmp(&a.events))
            .then(a.group.cmp(&b.group))
    });
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example() -> Evidence {
        Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap()
    }
    #[test]
    fn directions_actions_and_benign_context_stay_distinct() {
        let mut e = example();
        let mut event = e.events[0].clone();
        event.identity.sub_request = Some(1);
        event.action = "block".into();
        event.report_ids = vec!["report-2".into()];
        event.detections[0].report_id = "report-2".into();
        event.detections[0].direction = "response".into();
        event.detections[0].action = "block".into();
        e.events.push(event);
        let mut benign = e.events[0].detections[0].clone();
        benign.row = 1;
        benign.detector = "dlp".into();
        benign.verdict = "benign".into();
        e.events[0].detections.push(benign);
        let a = analyze(&e);
        assert_eq!(a.findings.len(), 2);
        assert_eq!(a.violation_rows, 2);
        assert_eq!(a.nonviolation_rows, 1);
        assert_eq!(a.distinct_scans, 1);
        assert_eq!(a.eligible_events, 2);
        assert!(
            a.findings
                .iter()
                .any(|f| f.recommendation.contains("monitoring interpretation"))
        );
    }
    #[test]
    fn conflicts_and_window_gaps_suppress_definitive_claims() {
        let mut e = example();
        e.events[0].detections[0].action = "block".into();
        let a = analyze(&e);
        assert!(a.findings[0].conflicting);
        assert!(a.findings[0].recommendation.starts_with("Reconcile"));
        e.events[0].timestamp_ms = None;
        let a = analyze(&e);
        assert_eq!(a.unknown_time_events, 1);
        assert!(a.findings.is_empty());
        e.events[0].timestamp_ms = Some(e.window.end_ms);
        assert_eq!(analyze(&e).outside_window_events, 1);
    }
    #[test]
    fn repeated_detector_rows_are_not_distinct_events_or_scans() {
        let mut e = example();
        let mut d = e.events[0].detections[0].clone();
        d.row = 1;
        e.events[0].detections.push(d);
        let a = analyze(&e);
        assert_eq!(a.findings[0].rows.len(), 2);
        assert_eq!(a.findings[0].events, 1);
        assert_eq!(a.findings[0].scans, 1);
    }
}

#[cfg(test)]
mod attribution_tests {
    use super::*;
    fn example() -> Evidence {
        Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap()
    }
    #[test]
    fn repeated_report_rows_across_buckets_are_unresolved_not_new_violations() {
        let mut e = example();
        let mut other = e.events[0].clone();
        other.identity.app_name = "second bucket".into();
        e.events.push(other);
        let a = analyze(&e);
        assert_eq!(a.ambiguous_rows, 1);
        assert_eq!(a.violation_rows, 0);
        assert!(a.findings.is_empty());
        assert_eq!(a.eligible_events, 2);
        assert_eq!(a.distinct_scans, 1);
    }
    #[test]
    fn conflicting_current_protections_do_not_establish_monitoring() {
        let mut e = example();
        let mut protection = e.profiles[0].protections[0].clone();
        protection.action = "block".into();
        e.profiles[0].protections.push(protection);
        let a = analyze(&e);
        assert!(
            !a.findings[0]
                .recommendation
                .contains("support a monitoring interpretation")
        );
        assert!(
            a.findings[0]
                .recommendation
                .contains("verify the effective event-time policy")
        );
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    fn example() -> Evidence {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        e.rules_version = 2;
        e.events[0].detector_hints = vec!["prompt/injection".into()];
        e.events[0].detections[0].detector = "pi".into();
        e.profiles[0].protections[0].detector = "prompt-injection".into();
        e
    }
    #[test]
    fn metadata_findings_counts_and_ids_do_not_depend_on_example_reports() {
        let mut e = example();
        let enriched = analyze(&e);
        e.events[0].detections.clear();
        let metadata = analyze(&e);
        assert_eq!(metadata.findings.len(), 1);
        assert_eq!(
            metadata.metadata_observations,
            enriched.metadata_observations
        );
        assert_eq!(metadata.findings[0].events, enriched.findings[0].events);
        assert_eq!(metadata.findings[0].id, enriched.findings[0].id);
        assert_eq!(metadata.violation_rows, 0);
        assert_eq!(enriched.violation_rows, 1);
        assert_eq!(metadata.findings[0].metadata_observations, 1);
        assert!(
            metadata.findings[0]
                .recommendation
                .contains("support a monitoring interpretation")
        );
        assert_eq!(e.events[0].detector_hints, ["prompt/injection"]);
    }
    #[test]
    fn alias_conflicts_and_unknown_actions_do_not_imply_monitoring() {
        let mut e = example();
        let mut protection = e.profiles[0].protections[0].clone();
        protection.detector = "pi".into();
        protection.action = "block".into();
        e.profiles[0].protections.push(protection);
        assert!(
            !analyze(&e).findings[0]
                .recommendation
                .contains("support a monitoring interpretation")
        );
        e.events[0].action = "unknown".into();
        assert!(
            analyze(&e).findings[0]
                .recommendation
                .starts_with("Resolve")
        );
    }
    #[test]
    fn classification_without_flags_is_visible_but_not_a_detector_count() {
        let mut e = example();
        e.events[0].detector_hints.clear();
        e.events[0].detections.clear();
        let a = analyze(&e);
        assert_eq!(a.findings.len(), 1);
        assert_eq!(a.findings[0].group.detector, "unknown");
        assert_eq!(a.metadata_observations, 0);
        assert_eq!(a.findings[0].events, 1);
    }
}
