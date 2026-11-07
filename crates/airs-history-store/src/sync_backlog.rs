//! Exact retained-work revisits. These never establish historical traversal coverage.
use super::*;
use airs_runtime_core::signal::{collect, evidence::Event};
use std::sync::atomic::Ordering;

const RECONCILE_SQL: &str = r#"SELECT w.work_id,w.descriptor_json,w.provenance_json
FROM sync_work w
LEFT
JOIN json_each(?1) r ON json_extract(r.value,'$.applicationId')=json_extract(w.descriptor_json,'$.identity.app_id')
WHERE w.kind IN ('results','reports')
AND w.state IN ('pending','deferred','ineligible','falling_behind')
AND w.work_id>?2
AND (json_array_length(?3)=0 OR json_extract(w.descriptor_json,'$.identity.app_id') IN (SELECT value
FROM json_each(?3)))
AND (r.value IS NULL OR (json_extract(w.provenance_json,'$.scanner_reference.endpoint') IS NOT NULL
AND json_extract(w.provenance_json,'$.scanner_reference.endpoint')!=json_extract(r.value,'$.endpoint')))
AND (w.state!='ineligible' OR coalesce(w.reason,'')!=CASE WHEN r.value IS NULL THEN 'binding_unavailable' ELSE 'binding_rebound' END OR coalesce(json_extract(w.provenance_json,'$.ineligible_configuration'),'null')!=coalesce(r.value,'null'))
ORDER BY w.work_id
LIMIT 200"#;

const DUE_SQL: &str = r#"WITH due AS (SELECT w.work_id,w.descriptor_json,w.kind,json_extract(w.descriptor_json,'$.identity.app_id') AS app,row_number() OVER (PARTITION BY json_extract(w.descriptor_json,'$.identity.app_id')
ORDER BY coalesce(w.last_attempt_ms,0),w.work_id) AS rank
FROM sync_work w
JOIN json_each(?1) r ON json_extract(r.value,'$.applicationId')=json_extract(w.descriptor_json,'$.identity.app_id')
JOIN events e ON e.app_id=json_extract(w.descriptor_json,'$.identity.app_id')
AND e.app_name=json_extract(w.descriptor_json,'$.identity.app_name')
AND e.session_id=json_extract(w.descriptor_json,'$.identity.session_id')
AND e.scan_id=json_extract(w.descriptor_json,'$.identity.scan_id')
AND e.sub_request=json_extract(w.descriptor_json,'$.identity.sub_request')
WHERE w.kind IN ('results','reports')
AND w.state!='complete'
AND (w.kind!='reports' OR (?4>0 AND e.verdict='malicious' AND e.event_ms IS NOT NULL))
AND (json_array_length(?2)=0 OR e.app_id IN (SELECT value
FROM json_each(?2)))
AND (json_extract(w.provenance_json,'$.scanner_reference.endpoint') IS NULL OR json_extract(w.provenance_json,'$.scanner_reference.endpoint')=json_extract(r.value,'$.endpoint'))
AND (w.last_attempt_ms IS NULL OR w.last_attempt_ms<=?3)
AND (coalesce(w.binding_revision,'')!=json_extract(r.value,'$.revision') OR (w.state='pending'
AND w.attempts<3
AND (w.eligible_after_ms IS NULL OR w.eligible_after_ms<=?3)))
AND NOT EXISTS(SELECT 1
FROM service_cooldowns c
WHERE c.service=w.kind
AND (c.until_ms>?3 OR c.observed_ms>?3))
AND NOT EXISTS(SELECT 1
FROM work_attempts a
JOIN sync_work denied ON denied.work_id=a.work_id
WHERE a.binding_revision=json_extract(r.value,'$.revision')
AND a.outcome='auth_denied'
AND json_extract(denied.descriptor_json,'$.identity.app_id')=e.app_id)) SELECT work_id,descriptor_json,kind,(SELECT length(CAST(provenance_json AS BLOB))
FROM sync_work original
WHERE original.work_id=due.work_id)
FROM due
ORDER BY rank,app,work_id
LIMIT 200"#;

#[derive(Clone)]
pub struct Item {
    descriptor: Descriptor,
    source: Source,
    event: Event,
}
impl Sync {
    fn reconcile_bindings(&self) -> Result<(), CliError> {
        let references = json!(self.references.values().collect::<Vec<_>>()).to_string();
        let apps = json!(self.apps).to_string();
        let mut cursor = String::new();
        loop {
            let last = self.batch(|conn| {
                let mut query = conn.prepare(RECONCILE_SQL).map_err(sql_error)?;
                let rows = query
                    .query_map(params![references, cursor, apps], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    })
                    .map_err(sql_error)?;
                let mut last = None;
                let mut bytes = 0;
                for row in rows {
                    let (id, descriptor, provenance) = row.map_err(sql_error)?;
                    let descriptor_bytes = descriptor.len();
                    let descriptor: Descriptor = serde_json::from_str(&descriptor)
                        .map_err(|_| failure("retained work identity is invalid"))?;
                    let reference = self.references.get(&descriptor.identity.app_id);
                    let reference_bytes = serde_json::to_vec(&reference)
                        .map_err(|_| failure("invalid public binding"))?
                        .len();
                    let size =
                        id.len() + descriptor_bytes + provenance.len() + reference_bytes * 2 + 2048;
                    if size > 256 * 1024 {
                        return Err(failure(
                            "stored work exceeds the binding-transition batch bound",
                        ));
                    }
                    if bytes + size > 256 * 1024 {
                        break;
                    }
                    work::configuration_ineligible(
                        conn,
                        &id,
                        database::now_ms()?,
                        self.references.get(&descriptor.identity.app_id),
                    )?;
                    bytes += size;
                    last = Some(id);
                }
                Ok(last)
            })?;
            if let Some(last) = last {
                cursor = last;
            } else {
                break;
            }
        }
        Ok(())
    }
    pub fn split_backlog(items: Vec<Item>) -> (Vec<Item>, Vec<Item>) {
        items.into_iter().partition(|i| i.source == Source::Results)
    }
    /// One combined strategic pool. At most one slot is reserved for previously
    /// selected unfinished work; a one-ID budget alternates old/new preference.
    /// This ranking view is never published or used as an API query interval.
    pub fn select_examples(
        &self,
        current: &mut Evidence,
        old: Vec<Item>,
    ) -> Result<Vec<Item>, CliError> {
        let mut view = current.clone();
        for item in &old {
            if !view
                .events
                .iter()
                .any(|e| e.identity == item.event.identity)
            {
                view.events.push(item.event.clone());
            }
        }
        view.window.start_ms = i64::MIN + 1;
        view.window.end_ms = i64::MAX;
        let (cached,eligible,sequence)=self.batch(|conn|{
            let mut cached=std::collections::BTreeSet::new();let mut eligible=std::collections::BTreeSet::new();
            for event in &view.events {
                for report in &event.report_ids {
                    let d=Descriptor{identity:event.identity.clone(),report_id:Some(report.clone())};
                    if !repository::report_ownership(conn,&d)?{continue;}
                    eligible.insert(report.clone());
                    if repository::cached_example(conn,&event.identity,report)?{cached.insert(report.clone());}
                }
            }
            let sequence:i64=conn.query_row("SELECT count(*) FROM collection_runs WHERE kind IN ('sync','refresh') AND json_extract(provenance_json,'$.example_limit')>0",[],|r|r.get(0)).map_err(sql_error)?;
            Ok((cached,eligible,sequence))
        })?;
        for e in &mut view.events {
            e.report_ids.retain(|id| eligible.contains(id));
        }
        let preferred = if current.budgets.max_reports != 1 || sequence % 2 == 1 {
            old.iter()
                .filter_map(|i| i.descriptor.report_id.as_deref())
                .find(|id| eligible.contains(*id) && !cached.contains(*id))
        } else {
            None
        };
        collect::select_reports_policy(&mut view, current.budgets.max_reports, preferred, &cached);
        let selected = view.sampling.selected_report_ids.clone();
        let old_selected: Vec<_> = old
            .into_iter()
            .filter(|i| {
                i.descriptor
                    .report_id
                    .as_ref()
                    .is_some_and(|id| selected.contains(id))
                    && !current
                        .events
                        .iter()
                        .any(|e| e.identity == i.descriptor.identity)
            })
            .collect();
        current.sampling = view.sampling;
        current.sampling.algorithm = "coverage-greedy-persistent-v3".into();
        current
            .sampling
            .selected_report_ids
            .retain(|id| current.events.iter().any(|e| e.report_ids.contains(id)));
        let mut safe = json!(selected);
        airs_runtime_core::signal::privacy::document(&mut safe, &self.secrets, &self.payload);
        self.batch(|conn|{conn.execute("UPDATE collection_runs SET provenance_json=json_set(provenance_json,'$.example_selection',json(?2),'$.example_policy','combined strategic pool; cached features covered; one old-work reservation; one-ID old/new alternation') WHERE run_id=?1",params![self.run,safe.to_string()]).map_err(sql_error)?;Ok(())})?;
        Ok(old_selected)
    }
    pub fn current_event_limit(&self, limit: u64) -> u64 {
        limit.saturating_sub(self.backlog_reserved.load(Ordering::Relaxed))
    }
    pub fn prepare_backlog(
        &self,
        event_limit: u64,
        report_limit: u64,
    ) -> Result<Vec<Item>, CliError> {
        self.reconcile_bindings()?;
        let now = database::now_ms()?;
        let references = json!(self.references.values().collect::<Vec<_>>()).to_string();
        let apps = json!(self.apps).to_string();
        let items=self.batch(|conn|{
            conn.execute("UPDATE collection_runs SET provenance_json=json_set(provenance_json,'$.example_limit',?2) WHERE run_id=?1",params![self.run,i64::try_from(report_limit).map_err(|_|failure("invalid report limit"))?]).map_err(sql_error)?;
            let sequence:i64=conn.query_row("SELECT count(*) FROM collection_runs WHERE kind IN ('sync','refresh')",[],|r|r.get(0)).map_err(sql_error)?;
            let capacity=event_limit/2+if sequence%2==1{event_limit%2}else{0};
            if capacity==0{return Ok(vec![]);}
            let mut query=conn.prepare(DUE_SQL).map_err(sql_error)?;
            let rows=query.query_map(params![references,apps,now,i64::try_from(report_limit).map_err(|_|failure("invalid report limit"))?],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?))).map_err(sql_error)?;
            let mut items=vec![];let mut bytes=0;let mut state_bytes=4096;
            for row in rows{
                let(id,descriptor,kind,provenance_bytes)=row.map_err(sql_error)?;
                let descriptor_bytes=descriptor.len();
                let descriptor:Descriptor=serde_json::from_str(&descriptor).map_err(|_|failure("retained work descriptor is invalid"))?;
                let source=if kind=="results"{Source::Results}else{Source::Reports};
                if descriptor.id(source)?!=id{return Err(failure("stored work identity does not match its descriptor"));}
                let reference=self.references.get(&descriptor.identity.app_id).ok_or_else(||failure("current binding disappeared from pinned context"))?;
                let reference_bytes=serde_json::to_vec(reference).map_err(|_|failure("invalid public binding"))?.len();
                let frame=id.len()+descriptor_bytes+usize::try_from(provenance_bytes).map_err(|_|failure("invalid stored work length"))?+reference_bytes*2+2048;
                if frame>256*1024{return Err(failure("stored work exceeds the normalized transition bound"));}
                if state_bytes+frame>256*1024{break;}
                state_bytes+=frame;
                if source==Source::Reports&&!repository::report_ownership(conn,&descriptor)?{work::defer_local(conn,&id,now,reference,work::LocalDeferral::ReportOwnership)?;continue;}

                let Some(event)=repository::cached_event(conn,&descriptor.identity)? else{work::defer_local(conn,&id,now,reference,work::LocalDeferral::BoundedView)?;continue};
                let cached=if source==Source::Reports {repository::cached_example(conn,&descriptor.identity,descriptor.report_id.as_deref().unwrap_or_default())?}else{event.sources.iter().any(|s|s=="scan-results")&&["benign","malicious"].contains(&event.verdict.as_str())&&["allow","block"].contains(&event.action.as_str())};
                if cached{work::complete_cached(conn,&id,now)?;continue;}
                let size=serde_json::to_vec(&event).map_err(|_|failure("invalid cached work event"))?.len()+serde_json::to_vec(&descriptor).map_err(|_|failure("invalid retained descriptor"))?.len()+1024;
                if size>256*1024{work::defer_local(conn,&id,now,reference,work::LocalDeferral::BoundedView)?;continue;}
                if items.len() as u64>=capacity{break;}
                if bytes+size>256*1024{continue;}
                bytes+=size;items.push(Item{descriptor,source,event});
            }
            Ok(items)
        })?;
        self.backlog_reserved
            .store(items.len() as u64, Ordering::Relaxed);
        Ok(items)
    }
    pub async fn revisit(
        &self,
        inv: &airs_runtime_core::signal::sink::CollectContext<'_>,
        tenant: &TenantContext,
        template: &Evidence,
        items: Vec<Item>,
    ) -> Result<(), CliError> {
        let mut groups: BTreeMap<(String, &'static str), std::collections::VecDeque<Item>> =
            BTreeMap::new();
        for item in items {
            groups
                .entry((item.descriptor.identity.app_id.clone(), item.source.name()))
                .or_default()
                .push_back(item);
        }
        let mut active: std::collections::VecDeque<_> = groups.keys().cloned().collect();
        while let Some(key) = active.pop_front() {
            let queue = groups
                .get_mut(&key)
                .expect("scheduled retained work exists");
            let items: Vec<_> = (0..5).filter_map(|_| queue.pop_front()).collect();
            if !queue.is_empty() {
                active.push_back(key.clone());
            }
            let source = items[0].source;
            let descriptors: Vec<_> = items.iter().map(|i| i.descriptor.clone()).collect();
            let contexts = self.batch(|conn| repository::retained_contexts(conn, &descriptors))?;
            let (admitted, _) = self.reserve_scoped(&descriptors, source, &key.0, true)?;
            if admitted.is_empty() {
                continue;
            }
            let scanner = tenant
                .config
                .scanner_for_application(&key.0)
                .and_then(|(_, b)| {
                    airs_runtime_core::clients::build_scanner_bounded(
                        &b.overlay(&tenant.config),
                        inv.session,
                        None,
                        Some(&self.budget),
                    )
                    .ok()
                });
            let Some((scanner, status)) = scanner else {
                self.outcomes(
                    &admitted
                        .iter()
                        .map(|(_, a)| (a.clone(), Outcome::NotSent))
                        .collect::<Vec<_>>(),
                    None,
                    source,
                    None,
                )?;
                continue;
            };
            let mut events: Vec<_> = items
                .into_iter()
                .filter(|i| {
                    admitted
                        .iter()
                        .any(|(d, _)| d.identity == i.descriptor.identity)
                })
                .map(|i| i.event)
                .collect();
            events.sort_by(|a, b| a.identity.cmp(&b.identity));
            events.dedup_by(|a, b| a.identity == b.identity);
            let operation = if source == Source::Results {
                "airs.scan.results"
            } else {
                "airs.scan.reports"
            };
            let before = status.attempts(operation);
            let observed = if source == Source::Results {
                collect::enrich_in_context(
                    &scanner,
                    &status,
                    &self.budget,
                    &mut events,
                    inv.secrets,
                    Some(&contexts),
                )
                .await
            } else {
                let ids: Vec<_> = admitted
                    .iter()
                    .filter_map(|(d, _)| d.report_id.clone())
                    .collect();
                self.backlog_example_ids
                    .fetch_add(ids.len() as u64, Ordering::Relaxed);
                collect::details_attributed(
                    &scanner,
                    &status,
                    &self.budget,
                    &mut events,
                    &ids,
                    inv.secrets,
                    inv.payload_spans,
                )
                .await
            };
            let attributed = observed.attributed;
            let observed = observed.source;
            let sent = status.attempts(operation) > before;
            let response = status.response(operation);
            let finalized = self.finish_transport_only(&admitted, sent, response, source)?;
            if sent {
                self.observed_cooldown(source, response)?;
            }
            let mut fragment = template.clone();
            fragment.events = events.clone();
            fragment.profiles.clear();
            fragment.applications.clear();
            fragment.metrics.clear();
            fragment.sampling.selected_report_ids.clear();
            fragment.sources.clear();
            fragment.sources.push(observed);
            Self::referenced_sources(&mut fragment)?;
            fragment.collection_id = self.run.clone();
            fragment.collected_ms = database::now_ms()?;
            fragment.seal_native()?;
            let mut bindings = BTreeMap::new();
            if sent && let Some(reference) = self.references.get(&key.0) {
                for e in &events {
                    if attributed.contains(&e.identity) {
                        bindings.insert(e.identity.clone(), reference.clone());
                    }
                }
            }
            let mut origins = self
                .origins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for event in &fragment.events {
                origins.entry(event.identity.clone()).or_insert_with(|| {
                    format!("native/{}/observation/{}", self.run, uuid::Uuid::new_v4())
                });
            }
            let records = repository::backlog_records(
                &fragment,
                &self.secrets,
                &self.payload,
                &bindings,
                &origins,
            )?;
            drop(origins);
            let mut next = 0;
            while next < records.len() {
                let end = records.batch_end(next);
                self.batch(|conn| {
                    records.store(conn, next, end, &self.run, fragment.collected_ms)
                })?;
                next = end;
            }
            let outcomes=admitted.into_iter().filter(|_| !finalized).map(|(d,a)|{
                let usable=attributed.contains(&d.identity)&&records.identities().contains(&d.identity)&&events.iter().any(|e|e.identity==d.identity&&if source==Source::Results{["benign","malicious"].contains(&e.verdict.as_str())&&["allow","block"].contains(&e.action.as_str())}else{e.detections.iter().any(|det|Some(det.report_id.as_str())==d.report_id.as_deref()&&det.snippets.iter().any(|s|matches!(s.state,airs_runtime_core::signal::evidence::SnippetState::Present|airs_runtime_core::signal::evidence::SnippetState::Empty|airs_runtime_core::signal::evidence::SnippetState::Withheld)))});
                let outcome=if !sent{Outcome::NotSent}else if matches!(response.0,Some(401|403)){Outcome::AuthDenied}else if response.0==Some(429){Outcome::RateLimited}else if usable{Outcome::Returned}else if response.0.is_none_or(|s|s>=500){Outcome::Failed}else{Outcome::Missing};(a,outcome)
            }).collect::<Vec<_>>();
            self.outcomes(
                &outcomes,
                if sent { response.0 } else { None },
                source,
                response.1,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "sync_backlog_tests.rs"]
mod tests;
