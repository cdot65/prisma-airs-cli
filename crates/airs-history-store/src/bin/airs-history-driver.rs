//! `airs-history-driver`: the history store's executable test driver (plan M1.3).
//!
//! M1 removed the unpublished local `airs runtime history` and `airs runtime sync`
//! commands from the CLI; their domain code moved here. Until M2–M4 expose the same
//! operations as authenticated MCP tools and durable jobs, this binary keeps the
//! B3-qualified executable tests (`tests/`) running against the extracted code with the
//! former grammar: `runtime sync [...]` and `runtime history <leaf> [...]`. It is a test
//! fixture, not a product. No release or documented procedure builds or installs it,
//! building or installing the `airs` CLI never compiles it, and its crate is
//! `publish = false`; a local `cargo install --path crates/airs-history-store` could
//! still build it, which is unsupported. It reads the test tenant store
//! (`AIRS_CLI_CONFIG_DIR`) only to build the same tenant value the CLI did.

use airs_history_store::{self as history, identity::Cloud, inspect, repository, sync};
use airs_runtime_core::{
    error::{CliError, EXIT_OPERATIONAL, InterruptContext, SecretSet},
    scan_logs::{self, CollectionOptions, Limits},
    signal::{privacy::PayloadSpans, sink::CollectContext},
    store::{normalize, parse_toml, valid_tenant_name},
    tenant::{Selected, TenantContext, config::TenantConfig, scanners::valid_application},
};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

/// Flags that take a value; every other recognised flag is a switch.
const VALUED: &[&str] = &[
    "--since",
    "--until",
    "--app-id",
    "--max-reports",
    "--max-pages",
    "--max-events",
    "--history-dir",
    "--timeout",
    "--concurrency",
    "--max-requests",
    "--output",
    "--tenant",
    "--source",
    "--file",
    "--expect-tsg",
    "--expect-environment",
    "--before",
];
const SWITCHES: &[&str] = &[
    "--backfill",
    "--with-examples",
    "--strict",
    "--force",
    "--quiet",
    "--debug",
    "--examples-only",
];

struct Args {
    command: Vec<String>,
    values: BTreeMap<String, Vec<String>>,
    switches: Vec<String>,
}

impl Args {
    fn parse(argv: &[String]) -> Result<Self, CliError> {
        let mut args = Self {
            command: vec![],
            values: BTreeMap::new(),
            switches: vec![],
        };
        let mut items = argv.iter();
        while let Some(item) = items.next() {
            if VALUED.contains(&item.as_str()) {
                let value = items
                    .next()
                    .ok_or_else(|| CliError::usage(format!("{item} requires a value")))?;
                let values = args.values.entry(item.clone()).or_default();
                if item != "--app-id" && !values.is_empty() {
                    return Err(CliError::usage(format!("{item} cannot be repeated")));
                }
                values.push(value.clone());
            } else if SWITCHES.contains(&item.as_str()) {
                if args.switches.contains(item) {
                    return Err(CliError::usage(format!("{item} cannot be repeated")));
                }
                args.switches.push(item.clone());
            } else if item.starts_with('-') {
                return Err(CliError::usage(format!("unknown option '{item}'")));
            } else {
                args.command.push(item.clone());
            }
        }
        Ok(args)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.values
            .get(name)
            .and_then(|v| v.first())
            .map(String::as_str)
    }
    fn all(&self, name: &str) -> Vec<String> {
        self.values.get(name).cloned().unwrap_or_default()
    }
    fn on(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name)
    }
    fn json(&self) -> Result<bool, CliError> {
        match self.value("--output") {
            None | Some("pretty" | "table") => Ok(false),
            Some("json") => Ok(true),
            Some(_) => Err(CliError::usage("--output must be pretty, table or json")),
        }
    }
}

fn range(since: Option<&str>, until: Option<&str>) -> Result<Option<(i64, i64)>, CliError> {
    match (since, until) {
        (None, None) => Ok(None),
        (Some(since), Some(until)) => {
            let parse = |text: &str| {
                text.ends_with('Z')
                    .then(|| airs_runtime_core::time::parse_iso(text))
                    .flatten()
            };
            match (parse(since), parse(until)) {
                (Some(a), Some(b)) if a < b => Ok(Some((a, b))),
                _ => Err(CliError::usage(
                    "Supply valid UTC --since/--until instants ending in Z, with since before until",
                )),
            }
        }
        _ => Err(CliError::usage(
            "--since and --until must be supplied together",
        )),
    }
}

fn apps(args: &Args) -> Result<Vec<String>, CliError> {
    let mut apps = args.all("--app-id");
    if apps.len() > 1000 || apps.iter().any(|id| !valid_application(id)) {
        return Err(CliError::usage(
            "Supply at most 1000 nonblank --app-id values, at most 4096 bytes without controls",
        ));
    }
    apps.sort();
    apps.dedup();
    Ok(apps)
}

/// The test tenant store's selected tenant (`D/config.toml`, `D/tenants/<name>.toml`).
fn load_tenant(flag: Option<&str>) -> Result<TenantContext, CliError> {
    let unreadable = || CliError::operational("Cannot read a valid tenant config");
    let dir = std::env::var_os("AIRS_CLI_CONFIG_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(unreadable)?;
    let dir = normalize(&dir);
    let index = parse_toml(&std::fs::read(dir.join("config.toml")).map_err(|_| unreadable())?)
        .ok_or_else(unreadable)?;
    let tenants = index
        .get("tenants")
        .and_then(toml::Value::as_table)
        .ok_or_else(unreadable)?;
    let name = match flag {
        Some(name) => name.to_owned(),
        None => index
            .get("active")
            .and_then(toml::Value::as_str)
            .ok_or_else(unreadable)?
            .to_owned(),
    };
    if !valid_tenant_name(&name) {
        return Err(unreadable());
    }
    let tsg = tenants
        .get(&name)
        .and_then(|entry| entry.get("tsg_id"))
        .and_then(toml::Value::as_str)
        .ok_or_else(unreadable)?
        .to_owned();
    let bytes = std::fs::read(dir.join("tenants").join(format!("{name}.toml")))
        .map_err(|_| unreadable())?;
    let config = TenantConfig::parse(&bytes).map_err(|_| unreadable())?;
    Ok(TenantContext {
        selected: Selected {
            name,
            tsg_id: tsg,
            by_flag: flag.is_some(),
        },
        config,
    })
}

fn archive_root(args: &Args) -> Result<PathBuf, CliError> {
    let directory = args.value("--history-dir").map(PathBuf::from);
    if let Some(path) = &directory {
        history::validate_override(path)?;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    history::root(
        directory.as_deref(),
        std::env::var_os("XDG_DATA_HOME").as_deref(),
        home.as_deref(),
    )
}

struct Run {
    secrets: SecretSet,
    payload: PayloadSpans,
    session: scm::Session,
    interrupt: Option<InterruptContext>,
}

impl Run {
    fn context(&self) -> CollectContext<'_> {
        CollectContext {
            session: &self.session,
            secrets: &self.secrets,
            payload_spans: &self.payload,
            version: airs_runtime_core::VERSION,
        }
    }
}

enum Done {
    Data(String),
    Incomplete(String, &'static str),
}

/// `runtime sync`: the former plan validation, then [`sync::run`].
async fn run_sync(run: &mut Run, args: &Args) -> Result<Done, CliError> {
    if args.on("--debug") {
        return Err(CliError::usage("Disable --debug for runtime sync"));
    }
    let json = args.json()?;
    let range = range(args.value("--since"), args.value("--until"))?;
    let backfill = args.on("--backfill");
    if backfill && range.is_none() {
        return Err(CliError::usage("--backfill requires --since and --until"));
    }
    let with_examples = args.on("--with-examples");
    if args.value("--max-reports").is_some() && !with_examples {
        return Err(CliError::usage("--max-reports requires --with-examples"));
    }
    let directory = args.value("--history-dir").map(PathBuf::from);
    if let Some(path) = &directory {
        history::validate_override(path)?;
    }
    let app_ids = apps(args)?;
    let now = airs_runtime_core::time::now_ms();
    if range.is_some_and(|(since, until)| since >= now || until > now) {
        return Err(CliError::usage(
            "Sync absolute bounds must describe a past UTC interval",
        ));
    }
    if range.is_some_and(|(since, _)| {
        u64::try_from(now.saturating_sub(since))
            .unwrap_or(u64::MAX)
            .div_ceil(3_600_000)
            > 87600
    }) {
        return Err(CliError::usage(
            "--since exceeds the supported 87600-hour rolling query bound",
        ));
    }
    let options = CollectionOptions {
        limits: Limits::parse(
            args.value("--timeout"),
            args.value("--concurrency"),
            args.value("--max-requests"),
        )?,
        interval: 24,
        unit: "hours".into(),
        app_ids,
        max_pages: scan_logs::number(args.value("--max-pages"), 100, 1, 100, "--max-pages")?,
        max_events: scan_logs::number(args.value("--max-events"), 10000, 1, 10000, "--max-events")?,
        max_reports: if with_examples {
            scan_logs::number(args.value("--max-reports"), 5, 0, 10, "--max-reports")?
        } else {
            0
        },
    };
    run.secrets.guard_local_output();
    let tenant = load_tenant(args.value("--tenant"))?;
    run.secrets.register_config(&tenant.config);
    let root = archive_root(args)?;
    run.interrupt = Some(InterruptContext::Context("runtime history may contain committed sync observations and conservative started attempts; repeat sync to revisit eligible work".into()));
    let request = sync::SyncRequest {
        root,
        range,
        options,
        backfill,
        strict: args.on("--strict"),
        run_id: None,
        auth_replay: false,
        reports_gate: None,
        admission_gate: None,
        credential_revision: None,
        send_fence: None,
    };
    let report = match sync::run(&run.context(), &tenant, request).await {
        Ok(report) => report,
        Err(failure) => {
            if !failure.final_status_saved {
                eprintln!(
                    "Runtime history: final run status could not be saved; committed observations remain available."
                );
            }
            return Err(failure.error);
        }
    };
    let text = if json {
        serde_json::to_string_pretty(&report.document)
            .map_err(|_| history::failure("cannot encode sync summary"))?
    } else {
        format!(
            "Runtime history sync {}: {} observations; {} required sources incomplete",
            report.run_id, report.observed_events, report.incomplete_required_sources
        )
    };
    Ok(if report.incomplete {
        Done::Incomplete(
            text,
            "Runtime history sync retained incomplete observations; review source coverage.\n",
        )
    } else {
        Done::Data(text)
    })
}

fn timeout(args: &Args, default: u64) -> Result<Duration, CliError> {
    Ok(Duration::from_secs(scan_logs::number(
        args.value("--timeout"),
        default,
        1,
        3600,
        "--timeout",
    )?))
}

fn protect(value: &mut serde_json::Value, secrets: &SecretSet) -> Result<(), CliError> {
    use serde_json::Value;
    match value {
        Value::String(text) => {
            *text = airs_runtime_core::text::terminal_safe(secrets.scrub(text)).into_owned()
        }
        Value::Array(items) => {
            for item in items {
                protect(item, secrets)?;
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                if secrets.scrub(key) != *key {
                    return Err(history::failure(
                        "history metadata cannot be safely displayed",
                    ));
                }
                protect(item, secrets)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// `runtime history <leaf>`: the former plan validation, then the archive operation.
fn run_history(run: &mut Run, args: &Args) -> Result<Done, CliError> {
    let leaf = args.command.get(2).map(String::as_str);
    args.json()?;
    enum Action {
        Retry(sync::Source),
        Status,
        Stats(Duration),
        Coverage(Option<(i64, i64)>, Vec<String>),
        Import(PathBuf, String, String),
        Maintain(bool, Duration),
        Export(PathBuf),
        Logical(PathBuf, String, String),
        Prune(i64, Vec<String>, bool),
    }
    let action = match leaf {
        Some("retry") => {
            if !args.on("--force") {
                return Err(CliError::usage(
                    "--force is required for explicit retry reset",
                ));
            }
            Action::Retry(match args.value("--source") {
                Some("results") => sync::Source::Results,
                Some("reports") => sync::Source::Reports,
                _ => return Err(CliError::usage("--source must be results or reports")),
            })
        }
        Some("status") => Action::Status,
        Some("stats") => Action::Stats(timeout(args, 120)?),
        Some("coverage") => Action::Coverage(
            range(args.value("--since"), args.value("--until"))?,
            apps(args)?,
        ),
        Some("export") => {
            let file = args.value("--file").unwrap_or_default();
            if file.is_empty() {
                return Err(CliError::usage("--file cannot be empty"));
            }
            Action::Export(file.into())
        }
        Some("prune") => {
            if !args.on("--examples-only") {
                return Err(CliError::usage("this driver prunes --examples-only"));
            }
            let before = args
                .value("--before")
                .and_then(airs_runtime_core::dto::utc_ms)
                .ok_or_else(|| CliError::usage("--before must be an RFC 3339 UTC instant"))?;
            Action::Prune(before, apps(args)?, args.on("--force"))
        }
        Some("import") => {
            let file = args.value("--file").unwrap_or_default();
            let tsg = args.value("--expect-tsg").unwrap_or_default();
            let environment = args.value("--expect-environment").unwrap_or_default();
            if file.is_empty() || tsg.is_empty() || environment.is_empty() {
                return Err(CliError::usage(
                    "--file, --expect-tsg and --expect-environment are required",
                ));
            }
            Action::Logical(file.into(), tsg.into(), environment.into())
        }
        Some("import-bundle") => {
            let file = args.value("--file").unwrap_or_default();
            if file.is_empty() {
                return Err(CliError::usage("--file cannot be empty"));
            }
            let tsg = args.value("--expect-tsg").unwrap_or_default();
            if tsg.is_empty() || tsg.len() > 32 || !tsg.bytes().all(|b| b.is_ascii_digit()) {
                return Err(CliError::usage(
                    "--expect-tsg must contain 1–32 ASCII digits",
                ));
            }
            let environment = args.value("--expect-environment").unwrap_or_default();
            if !environment.strip_prefix("env1:").is_some_and(|v| {
                v.len() == 64
                    && v.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }) {
                return Err(CliError::usage(
                    "--expect-environment must be env1: followed by 64 lowercase hexadecimal digits",
                ));
            }
            Action::Import(file.into(), tsg.into(), environment.into())
        }
        Some(leaf @ ("compact" | "recover")) => {
            if !args.on("--force") {
                return Err(CliError::usage(
                    "--force is required for explicit archive maintenance",
                ));
            }
            let compact = leaf == "compact";
            Action::Maintain(compact, timeout(args, if compact { 600 } else { 120 })?)
        }
        _ => return Err(CliError::usage("unknown runtime history command")),
    };
    if let Some(path) = args.value("--history-dir") {
        history::validate_override(Path::new(path))?;
    }
    run.secrets.guard_local_output();
    let tenant = load_tenant(args.value("--tenant"))?;
    run.secrets.register_config(&tenant.config);
    let cloud = Cloud::from_tenant(&tenant)?;
    cloud.check_public(&run.secrets)?;
    let root = archive_root(args)?;
    let session = &run.session;
    let mut document = match action {
        Action::Retry(source) => sync::retry(&root, &cloud, source, session)?,
        Action::Status => inspect::status(&root, &cloud, None, session)?,
        Action::Stats(budget) => inspect::status(&root, &cloud, Some(budget), session)?,
        Action::Coverage(range, apps) => {
            let (since, until) = match range {
                Some(pair) => pair,
                None => {
                    let now = history::now_ms()?;
                    (now - 86_400_000, now)
                }
            };
            inspect::coverage(&root, &cloud, since, until, &apps, session)?
        }
        Action::Import(file, tsg, environment) => {
            if tsg != cloud.tsg || environment != cloud.environment_id {
                return Err(history::failure(
                    "explicit legacy cloud binding does not match the selected context; archive was not changed",
                ));
            }
            let original = repository::read_legacy(&file)?;
            let context = "runtime history may contain committed import batches; no remote requests were made";
            run.interrupt = Some(InterruptContext::Context(context.into()));
            serde_json::to_value(
                repository::import_evidence(
                    &root,
                    &cloud,
                    &original,
                    &run.secrets,
                    &run.payload,
                    session,
                )
                .map_err(|error| match error {
                    CliError::Interrupted(_) => {
                        CliError::Interrupted(InterruptContext::Context(context.into()))
                    }
                    other => other,
                })?,
            )
            .map_err(|_| history::failure("cannot encode import result"))?
        }
        Action::Export(file) => history::transfer::export_file(
            &root,
            &cloud,
            &file,
            Duration::from_secs(600),
            session,
            &run.secrets,
        )?,
        Action::Logical(file, tsg, environment) => {
            if tsg != cloud.tsg || environment != cloud.environment_id {
                return Err(history::failure(
                    "explicit cloud binding does not match the selected context; archive was not changed",
                ));
            }
            history::transfer::import_file(
                &root,
                &cloud,
                &file,
                Duration::from_secs(600),
                session,
                &run.secrets,
            )?
        }
        Action::Prune(before, apps, force) => repository::prune_examples(
            &root,
            &cloud,
            before,
            &apps,
            force,
            Duration::from_secs(600),
            session,
        )?,
        Action::Maintain(compact, timeout) => {
            let context = if compact {
                "runtime history may contain a published compacted archive; no remote requests were made"
            } else {
                "local recovery may have progressed; preserve the archive and any private recovery artifacts before retrying"
            };
            run.interrupt = Some(InterruptContext::Context(context.into()));
            let result = if compact {
                history::compact(&root, &cloud, timeout, session)
            } else {
                history::recover(&root, &cloud, timeout, session)
            };
            result.map_err(|error| match error {
                CliError::Interrupted(_) => {
                    CliError::Interrupted(InterruptContext::Context(context.into()))
                }
                other => other,
            })?
        }
    };
    protect(&mut document, &run.secrets)?;
    let rendered = serde_json::to_string_pretty(&document)
        .map_err(|_| history::failure("cannot encode archive inspection"))?;
    if run.secrets.carried_by(&rendered) {
        return Err(history::failure(
            "history metadata cannot be safely displayed",
        ));
    }
    Ok(Done::Data(rendered))
}

fn watch_signals(session: scm::Session, code: Arc<AtomicI32>) {
    let marker = std::env::var_os("AIRS_CLI_TEST_SIGNAL_MARKER")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let (ready, listening) = std::sync::mpsc::channel::<()>();
    let _ = std::thread::Builder::new()
        .name("signals".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                let _ = ready.send(());
                return;
            };
            runtime.block_on(async move {
                use tokio::signal::unix::{SignalKind, signal};
                let (Ok(mut interrupt), Ok(mut terminate)) = (
                    signal(SignalKind::interrupt()),
                    signal(SignalKind::terminate()),
                ) else {
                    let _ = ready.send(());
                    return;
                };
                let _ = ready.send(());
                loop {
                    let received = tokio::select! {
                        _ = interrupt.recv() => 130,
                        _ = terminate.recv() => 143,
                    };
                    if let Some(path) = &marker {
                        let _ = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(path);
                    }
                    if code.swap(received, Ordering::SeqCst) != 0 {
                        std::process::exit(received);
                    }
                    session.cancel();
                }
            });
        });
    let _ = listening.recv_timeout(Duration::from_secs(5));
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut run = Run {
        secrets: SecretSet::default(),
        payload: PayloadSpans::default(),
        session: scm::Session::new(),
        interrupt: None,
    };
    let signal = Arc::new(AtomicI32::new(0));
    watch_signals(run.session.clone(), Arc::clone(&signal));
    let result = (|| {
        let args = Args::parse(&argv)?;
        let words: Vec<&str> = args.command.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["runtime", "sync"] => {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| CliError::operational("The async runtime could not start"))?;
                let result = runtime.block_on(run_sync(&mut run, &args));
                runtime.shutdown_background();
                result
            }
            ["runtime", "history", _] => run_history(&mut run, &args),
            _ => Err(CliError::usage("unknown command")),
        }
    })();
    let code = signal.load(Ordering::SeqCst);
    let result = match (result, code) {
        (Err(CliError::Interrupted(context)), _) => Err(CliError::Interrupted(context)),
        (_, 130 | 143) => Err(CliError::Interrupted(
            run.interrupt
                .take()
                .unwrap_or(InterruptContext::NoDataWritten),
        )),
        (result, _) => result,
    };
    let signal_code = (code != 0).then_some(code);
    let exit = match result {
        Ok(Done::Data(text)) => {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{text}");
            0
        }
        Ok(Done::Incomplete(text, warning)) => {
            let _ = writeln!(std::io::stdout().lock(), "{text}");
            eprint!("{warning}");
            EXIT_OPERATIONAL
        }
        Err(error) => {
            let error = if run.secrets.guards_local_output() {
                error.with_secrets(&run.secrets)
            } else {
                error
            };
            eprint!("{}", error.render());
            error.exit_code(signal_code)
        }
    };
    std::process::exit(exit);
}
