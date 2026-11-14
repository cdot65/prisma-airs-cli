//! G0 characterizations of the production-deployment CLI failures.
//!
//! Inputs are synthetic translations of CLI-ISSUES.md, not captured service
//! contracts. Successful writes below prove local preparation and fixture traffic
//! only. The fixture does not establish the service's accepted key-type vocabulary
//! or deployment replacement/merge semantics. See the gateway reliability ledger.

use super::*;
use scm::airs::gateway::models::GatewayDeploymentUpdateRequest;
use scm::validation::Mode;

const EXPIRES: &str = "2026-11-13T06:00:00Z";

fn deployment_args(settings: &Value) -> Vec<String> {
    vec![
        "deployments".into(),
        "update".into(),
        u(fx::DEPLOYMENT),
        "--auth-settings".into(),
        settings.to_string(),
    ]
}

fn service_args(f: &Fx, expiry: &str) -> (Vec<String>, String) {
    let destination = f.path("service-key.json");
    let args = vec![
        "api-keys".into(),
        "service".into(),
        "create".into(),
        "--workspace".into(),
        u(1),
        "--name".into(),
        "synthetic-smoke-test".into(),
        "--scopes".into(),
        "completions.write".into(),
        "--expires-at".into(),
        expiry.into(),
        "--secret-output".into(),
        destination.clone(),
    ];
    (args, destination)
}

fn execute(f: &Fx, args: &[String]) -> Run {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    f.gw(&refs)
}

fn assert_schema_refused(f: &Fx, out: &Run, method: &str, route: &str) {
    out.assert_code(2);
    assert!(out.stdout.is_empty());
    assert!(out.stderr.contains(&format!(
        "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for {method} {route} did not match schema"
    )));
    assert_eq!(f.server.request_count(), 0, "validation must precede OAuth");
    assert!(temporaries(&f.sandbox.tmp()).is_empty());
}

#[test]
fn recorded_deployment_attempts_fail_before_loading_a_tenant() {
    // All three attempts retain both the numeric boolean and the read-only field.
    // Attempt 2 adds unrelated resource metadata; attempt 3 lists all workspaces.
    for attempt in 1..=3 {
        let f = bare();
        let mut settings = json!({
            "gateway_base_url": "https://gw.example.test",
            "disable_portkey_gateway": 1,
            "allow_all_workspaces": 1,
        });
        if attempt == 3 {
            settings["workspaces_allowed"] = json!(["ws-one", "ws-two", "ws-three"]);
        }
        let mut args = deployment_args(&settings);
        if attempt == 2 {
            args.extend(
                [
                    "--name",
                    "edge",
                    "--type",
                    "production",
                    "--status",
                    "active",
                ]
                .map(str::to_owned),
            );
        }
        let out = execute(&f, &args);
        assert_schema_refused(
            &f,
            &out,
            "PUT",
            &format!("/deployments/{}", u(fx::DEPLOYMENT)),
        );
    }
}

#[test]
fn deployment_blockers_are_independent_and_grouped_url_is_not_a_fix() {
    for settings in [
        json!({"allow_all_workspaces": 1}),
        json!({"allow_all_workspaces": true, "disable_portkey_gateway": 1}),
        json!({"allow_all_workspaces": true, "grouped_gateway_base_url": null}),
    ] {
        let f = bare();
        let out = execute(&f, &deployment_args(&settings));
        assert_schema_refused(
            &f,
            &out,
            "PUT",
            &format!("/deployments/{}", u(fx::DEPLOYMENT)),
        );
    }
}

#[test]
fn read_auth_settings_need_both_type_and_field_changes_for_the_write_schema() {
    let mut settings = fx::deployment_detail()["auth_settings"].clone();
    let accepts = |settings: &Value| {
        GatewayDeploymentUpdateRequest::parse(json!({"auth_settings": settings}), Mode::Request)
            .is_ok()
    };
    assert!(
        !accepts(&settings),
        "the stored response is not write-shaped"
    );
    settings["allow_all_workspaces"] = json!(true);
    assert!(
        !accepts(&settings),
        "the read-only field still prevents writing"
    );
    settings
        .as_object_mut()
        .unwrap()
        .remove("disable_portkey_gateway");
    assert!(
        accepts(&settings),
        "the minimized write shape must be valid"
    );
}

#[test]
fn a_full_deployment_get_document_is_not_a_write_request_file() {
    let f = bare();
    let file = f.file(
        "deployment.json",
        fx::deployment_detail().to_string().as_bytes(),
    );
    let out = f.gw(&["deployments", "update", &u(fx::DEPLOYMENT), "--file", &file]);
    assert_schema_refused(
        &f,
        &out,
        "PUT",
        &format!("/deployments/{}", u(fx::DEPLOYMENT)),
    );
}

#[test]
fn write_shaped_deployment_settings_reach_the_fixture_once() {
    let f = fx();
    let settings = json!({
        "gateway_base_url": "https://gw.example.test",
        "allow_all_workspaces": true,
        "workspaces_allowed": ["ws-one", "ws-two", "ws-three"],
    });
    let mut args = deployment_args(&settings);
    args.push("--force".into());
    execute(&f, &args).assert_success();
    assert_eq!(
        f.body(Gw::DeploymentUpdate),
        json!({"auth_settings": settings})
    );
    assert_eq!(
        f.api(),
        [format!("PUT {ADMIN}/deployments/{}", u(fx::DEPLOYMENT))]
    );
    assert_eq!(f.tokens(), 1);
}

#[test]
fn missing_key_type_is_rejected_without_a_tenant_or_secret_file() {
    let f = bare();
    let (args, destination) = service_args(&f, EXPIRES);
    let out = execute(&f, &args);
    assert_schema_refused(&f, &out, "POST", "/api-keys/service");
    assert!(!Path::new(&destination).exists());
    assert!(std::fs::read_dir(f.sandbox.tmp()).unwrap().next().is_none());
}

#[test]
fn key_type_can_come_from_any_input_source_with_either_timestamp_form() {
    for expiry in [EXPIRES, "2026-11-13T06:00:00.000Z"] {
        for source in ["flag", "file", "assignment"] {
            let f = fx();
            let (mut args, destination) = service_args(&f, expiry);
            match source {
                "flag" => args.extend(["--type".into(), "workspace-service".into()]),
                "file" => {
                    let file = f.file("request.json", br#"{"type":"workspace-service"}"#);
                    args.extend(["--file".into(), file]);
                }
                "assignment" => {
                    args.extend(["--set-string".into(), "type=workspace-service".into()]);
                }
                _ => unreachable!(),
            }
            execute(&f, &args).assert_success();
            assert_eq!(
                f.body(Gw::KeyCreate),
                json!({
                    "name": "synthetic-smoke-test",
                    "workspace_id": u(1),
                    "organisation_id": TSG,
                    "scopes": ["completions.write"],
                    "expires_at": expiry,
                    "type": "workspace-service",
                })
            );
            assert_eq!(f.api(), [format!("POST {DATA}/api-keys/service")]);
            assert_eq!(f.tokens(), 1);
            assert_eq!(mode(Path::new(&destination)), 0o600);
            let saved: Value =
                serde_json::from_slice(&std::fs::read(&destination).unwrap()).unwrap();
            assert_eq!(saved, fx::key_created("service"));
            assert!(temporaries(&f.sandbox.tmp()).is_empty());
        }
    }
}

#[test]
fn service_key_type_obeys_file_flag_assignment_precedence() {
    let f = fx();
    let (mut args, _) = service_args(&f, EXPIRES);
    let file = f.file("request.json", br#"{"type":"file-type"}"#);
    args.extend([
        "--file".into(),
        file,
        "--type".into(),
        "flag-type".into(),
        "--set-string".into(),
        "type=workspace-service".into(),
    ]);
    execute(&f, &args).assert_success();
    assert_eq!(f.body(Gw::KeyCreate)["type"], json!("workspace-service"));
    assert_eq!(f.api(), [format!("POST {DATA}/api-keys/service")]);
}

#[test]
fn semantic_schema_rejections_record_validation_events_without_traffic() {
    for deployment in [true, false] {
        let f = bare();
        let (service, destination) = service_args(&f, EXPIRES);
        let (mut args, method, route) = if deployment {
            (
                deployment_args(&json!({"allow_all_workspaces": 1})),
                "PUT",
                format!("/deployments/{}", u(fx::DEPLOYMENT)),
            )
        } else {
            (service, "POST", "/api-keys/service".into())
        };
        args.push("--debug".into());
        let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = f.gw_with(&refs, &RunOptions::new().env("XDG_STATE_HOME", &state));
        assert_schema_refused(&f, &out, method, &route);
        assert!(out.stderr.contains("Debug: API log"));
        let (path, records) = debug_log::only_log(&f.sandbox);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["event"], json!("validation_failure"));
        assert_eq!(records[0]["stage"], json!("request_validation"));
        assert_eq!(records[0]["checkpoint"], json!("preflight"));
        assert_eq!(
            records[0]["operation"],
            json!(if deployment {
                "gateway.deployments.update"
            } else {
                "gateway.api_keys.service.create"
            })
        );
        assert_eq!(records[0]["requestSent"], json!(false));
        assert!(records[0].get("request").is_none());
        assert!(records[0].get("body").is_none());
        assert!(std::fs::metadata(path).unwrap().len() > 0);
        assert!(!Path::new(&destination).exists());
    }
}

#[test]
fn deployment_dry_run_and_execution_share_the_effective_request() {
    let f = fx();
    let settings = json!({"allow_all_workspaces": true, "workspaces_allowed": ["ws-one"]});
    let mut args = deployment_args(&settings);
    args.extend(["--dry-run".into(), "--output".into(), "json".into()]);
    let out = execute(&f, &args);
    out.assert_success();
    let preview = out.json();
    assert_eq!(preview["mode"], json!("dry-run"));
    assert_eq!(preview["request_sent"], json!(false));
    assert_eq!(preview["service_acceptance_checked"], json!(false));
    assert_eq!(
        preview["request"]["body"],
        json!({"auth_settings": settings})
    );
    assert_eq!(
        preview["execution_requirements"]["confirmation_required"],
        json!(true)
    );
    assert_eq!(f.server.request_count(), 0);
    let mut args = deployment_args(&settings);
    args.push("--force".into());
    execute(&f, &args).assert_success();
    assert_eq!(f.body(Gw::DeploymentUpdate), preview["request"]["body"]);
    assert_eq!(f.writes().len(), 1);
}

#[test]
fn key_dry_run_needs_no_secret_destination_and_redacts_its_preview() {
    let f = fx();
    let (mut args, destination) = service_args(&f, EXPIRES);
    args.truncate(args.len() - 2); // Omit --secret-output and its path.
    args.extend([
        "--type".into(),
        "workspace-service".into(),
        "--dry-run".into(),
        "--output".into(),
        "json".into(),
    ]);
    let out = execute(&f, &args);
    out.assert_success();
    let preview = out.json();
    assert_eq!(preview["redacted"], json!(true));
    assert_eq!(preview["request"]["body"]["organisation_id"], json!(TSG));
    assert_eq!(
        preview["request"]["body"]["type"],
        json!("workspace-service")
    );
    assert_ne!(
        preview["request"]["body"]["name"],
        json!("synthetic-smoke-test")
    );
    assert_eq!(
        preview["execution_requirements"]["secret_output_required"],
        json!(true)
    );
    assert_eq!(
        preview["execution_requirements"]["secret_output_supplied"],
        json!(false)
    );
    assert_eq!(f.server.request_count(), 0);
    assert!(!Path::new(&destination).exists());
    assert!(temporaries(&f.sandbox.tmp()).is_empty());
}

#[test]
fn key_dry_run_checks_but_never_reserves_a_supplied_destination() {
    let f = fx();
    let (mut args, destination) = service_args(&f, EXPIRES);
    args.extend([
        "--type".into(),
        "workspace-service".into(),
        "--dry-run".into(),
        "--output".into(),
        "json".into(),
    ]);
    execute(&f, &args).assert_success();
    assert!(!Path::new(&destination).exists());
    assert!(temporaries(&f.sandbox.tmp()).is_empty());
    std::fs::write(&destination, b"existing-synthetic-file").unwrap();
    let out = execute(&f, &args);
    out.assert_code(1);
    assert_eq!(
        out.stderr,
        error_block(
            &format!("{destination} already exists; choose another --secret-output"),
            None,
            &[]
        )
    );
    assert!(out.stdout.is_empty());
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"existing-synthetic-file"
    );
    assert_eq!(f.server.request_count(), 0);
}

#[test]
fn dry_run_does_not_bypass_schema_or_tenant_validation() {
    let f = fx();
    let mut args = deployment_args(&json!({"allow_all_workspaces": 1}));
    args.push("--dry-run".into());
    assert_schema_refused(
        &f,
        &execute(&f, &args),
        "PUT",
        &format!("/deployments/{}", u(fx::DEPLOYMENT)),
    );
    let (mut args, _) = service_args(&f, EXPIRES);
    args.extend([
        "--type".into(),
        "workspace-service".into(),
        "--dry-run".into(),
        "--organisation-id".into(),
        "200".into(),
    ]);
    let out = execute(&f, &args);
    out.assert_code(1);
    assert_eq!(
        out.stderr,
        error_block(
            "--organisation-id 200 is not the selected tenant's TSG; airs reads and changes only the selected tenant's organisation",
            None,
            &[]
        )
    );
    assert!(out.stdout.is_empty());
    assert_eq!(f.server.request_count(), 0);
}

#[test]
fn deployment_dry_run_conflicts_with_force_before_any_execution() {
    let f = fx();
    let mut args = deployment_args(&json!({"allow_all_workspaces": true}));
    args.extend(["--dry-run".into(), "--force".into()]);
    execute(&f, &args).assert_code(2);
    assert_eq!(f.server.request_count(), 0);
}

#[test]
fn dry_run_uses_local_tenant_context_without_management_credentials() {
    let server = FixtureServer::start();
    server.seed_gateway();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant_with(&server, "prod", TSG, |config| {
        config.remove("mgmtClientId");
        config.remove("mgmtClientSecret");
    });
    sandbox.register(&[&tenant], Some("prod"));
    let f = Fx { server, sandbox };
    let mut args = deployment_args(&json!({"allow_all_workspaces": true}));
    args.extend(["--dry-run".into(), "--output".into(), "json".into()]);
    execute(&f, &args).assert_success();
    assert_eq!(f.server.request_count(), 0);
}

#[test]
fn key_preview_accepts_type_from_flags_files_and_assignments() {
    let mut expected = None;
    for source in ["flag", "file", "assignment"] {
        let f = fx();
        let (mut args, _) = service_args(&f, EXPIRES);
        args.truncate(args.len() - 2);
        match source {
            "flag" => args.extend(["--type".into(), "workspace-service".into()]),
            "file" => {
                let file = f.file("preview-base.json", br#"{"type":"workspace-service"}"#);
                args.extend(["--file".into(), file]);
            }
            "assignment" => args.extend(["--set-string".into(), "type=workspace-service".into()]),
            _ => unreachable!(),
        }
        args.extend(["--dry-run".into(), "--output".into(), "json".into()]);
        let out = execute(&f, &args);
        out.assert_success();
        let body = out.json()["request"]["body"].clone();
        if let Some(expected) = &expected {
            assert_eq!(&body, expected);
        } else {
            expected = Some(body);
        }
        assert_eq!(f.server.request_count(), 0);
        assert!(temporaries(&f.sandbox.tmp()).is_empty());
    }
}

#[test]
fn deployment_preview_accepts_flags_files_and_assignments() {
    let settings = json!({"allow_all_workspaces": true, "workspaces_allowed": ["ws-one"]});
    for source in ["flag", "file", "assignment"] {
        let f = fx();
        let mut args = vec!["deployments".into(), "update".into(), u(fx::DEPLOYMENT)];
        match source {
            "flag" => args.extend(["--auth-settings".into(), settings.to_string()]),
            "file" => {
                let file = f.file(
                    "preview-base.json",
                    json!({"auth_settings": settings}).to_string().as_bytes(),
                );
                args.extend(["--file".into(), file]);
            }
            "assignment" => args.extend([
                "--set".into(),
                "auth_settings.allow_all_workspaces=true".into(),
                "--set".into(),
                "auth_settings.workspaces_allowed=[\"ws-one\"]".into(),
            ]),
            _ => unreachable!(),
        }
        args.extend(["--dry-run".into(), "--output".into(), "json".into()]);
        let out = execute(&f, &args);
        out.assert_success();
        assert_eq!(
            out.json()["request"]["body"],
            json!({"auth_settings": settings})
        );
        assert_eq!(f.server.request_count(), 0);
    }
}

#[test]
fn key_preview_withholds_arbitrary_name_and_type_values() {
    let f = fx();
    let (mut args, destination) = service_args(&f, EXPIRES);
    let name = args.iter().position(|arg| arg == "--name").unwrap() + 1;
    args[name] = fx::marker(80);
    args.extend([
        "--type".into(),
        fx::marker(81),
        "--dry-run".into(),
        "--output".into(),
        "json".into(),
    ]);
    let out = execute(&f, &args);
    out.assert_success();
    assert_eq!(out.json()["request"]["body"]["name"], json!("[withheld]"));
    assert_eq!(out.json()["request"]["body"]["type"], json!("[withheld]"));
    assert_eq!(f.server.request_count(), 0);
    assert!(!Path::new(&destination).exists());
}

#[test]
fn dry_run_debug_records_only_rejections_with_the_correct_stage() {
    for invalid in [false, true] {
        let f = fx();
        let mut args = deployment_args(
            &json!({"allow_all_workspaces": if invalid { json!(1) } else { json!(true) }}),
        );
        args.extend([
            "--dry-run".into(),
            "--debug".into(),
            "--output".into(),
            "json".into(),
        ]);
        let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = f.gw_with(&refs, &RunOptions::new().env("XDG_STATE_HOME", &state));
        let (_, records) = debug_log::only_log(&f.sandbox);
        if invalid {
            out.assert_code(2);
            assert_eq!(records.len(), 1);
            assert_eq!(records[0]["stage"], json!("request_validation"));
            assert_eq!(records[0]["checkpoint"], json!("preflight"));
            assert_eq!(records[0]["operation"], json!("gateway.deployments.update"));
            assert_eq!(records[0]["requestSent"], json!(false));
        } else {
            out.assert_success();
            assert_eq!(out.json()["request_sent"], json!(false));
            assert!(records.is_empty());
        }
        assert_eq!(f.server.request_count(), 0);
        assert!(temporaries(&f.sandbox.tmp()).is_empty());
    }
}

#[test]
fn execution_and_preview_log_flag_and_body_tenant_refusals_once() {
    for dry_run in [false, true] {
        for in_body in [false, true] {
            let f = fx();
            let (mut args, destination) = service_args(&f, EXPIRES);
            args.extend([
                "--type".into(),
                "workspace-service".into(),
                "--debug".into(),
            ]);
            if dry_run {
                args.push("--dry-run".into());
            }
            if in_body {
                args.extend(["--set".into(), "organisation_id=\"200\"".into()]);
            } else {
                args.extend(["--organisation-id".into(), "200".into()]);
            }
            let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = f.gw_with(&refs, &RunOptions::new().env("XDG_STATE_HOME", &state));
            out.assert_code(1);
            assert!(
                out.stderr
                    .contains("--organisation-id 200 is not the selected tenant's TSG")
            );
            assert!(out.stdout.is_empty());
            let (_, records) = debug_log::only_log(&f.sandbox);
            assert_eq!(records.len(), 1);
            assert_eq!(records[0]["stage"], json!("tenant_binding"));
            assert_eq!(records[0]["checkpoint"], json!("tenant_binding"));
            assert_eq!(
                records[0]["operation"],
                json!("gateway.api_keys.service.create")
            );
            assert_eq!(records[0]["requestSent"], json!(false));
            assert!(records[0].get("body").is_none());
            assert!(records[0].get("request").is_none());
            assert_eq!(f.server.request_count(), 0);
            assert!(!Path::new(&destination).exists());
            assert!(temporaries(&f.sandbox.tmp()).is_empty());
        }
    }
}

#[test]
fn rotating_preview_reports_execution_requirements_without_reserving_a_file() {
    let f = fx();
    let mut args = deployment_args(&json!({"allow_all_workspaces": true}));
    args.extend([
        "--rotate-auth".into(),
        "true".into(),
        "--dry-run".into(),
        "--output".into(),
        "json".into(),
    ]);
    let out = execute(&f, &args);
    out.assert_success();
    let preview = out.json();
    assert_eq!(preview["request"]["body"]["rotate_auth"], json!(true));
    assert_eq!(
        preview["execution_requirements"]["secret_output_required"],
        json!(true)
    );
    assert_eq!(
        preview["execution_requirements"]["secret_output_supplied"],
        json!(false)
    );
    assert_eq!(f.server.request_count(), 0);
    assert!(temporaries(&f.sandbox.tmp()).is_empty());
}

#[test]
fn text_and_quiet_json_previews_withhold_urls_and_arbitrary_labels() {
    for output in ["pretty", "json"] {
        let f = fx();
        let mut args = deployment_args(&json!({"allow_all_workspaces": true,
            "gateway_base_url": "https://user:SYNTHETIC-PRIVATE-PASSWORD@gw.example.test/private"}));
        args.extend([
            "--name".into(),
            "SYNTHETIC-PRIVATE-NAME".into(),
            "--dry-run".into(),
            "--output".into(),
            output.into(),
        ]);
        if output == "json" {
            args.push("--quiet".into());
        }
        let out = execute(&f, &args);
        out.assert_success();
        assert!(out.stdout.contains("dry-run"));
        assert!(!format!("{}{}", out.stdout, out.stderr).contains("SYNTHETIC-PRIVATE"));
        if output == "json" {
            assert_eq!(out.json()["redacted"], json!(true));
            assert!(out.stderr.is_empty());
        }
        assert_eq!(f.server.request_count(), 0);
    }
}

#[test]
fn provider_resolution_validation_is_logged_after_reads_without_sending_the_mutation() {
    let f = fx();
    f.data(|data| {
        let catalog = data.inventory.get_mut(&Gw::CatalogProviders).unwrap();
        let row = catalog["data"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["slug"] == json!("openai"))
            .unwrap();
        // Response UUID checks are tolerant; the resolved ID must still pass
        // strict request validation before the integration mutation can be sent.
        row["id"] = json!("synthetic-invalid-provider-id");
    });
    let key_file = f.file("provider.key", b"FAKE-GW-SECRET-38\n");
    let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
    let out = f.gw_with(
        &[
            "integrations",
            "create",
            "--ai-provider",
            "openai",
            "--name",
            "synthetic",
            "--slug",
            "synthetic",
            "--key-file",
            &key_file,
            "--debug",
        ],
        &RunOptions::new().env("XDG_STATE_HOME", &state),
    );
    out.assert_code(2);
    assert!(out.stdout.is_empty());
    assert!(
        out.stderr
            .contains("Request body for POST /integrations did not match schema")
    );
    assert_eq!(
        f.api(),
        [format!("GET {ADMIN}/utils/static-resources/ai-providers")]
    );
    assert!(f.writes().is_empty());
    let (_, records) = debug_log::only_log(&f.sandbox);
    let failures: Vec<_> = records
        .iter()
        .filter(|r| r["event"] == json!("validation_failure"))
        .collect();
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0]["operation"],
        json!("gateway.integrations.create")
    );
    assert_eq!(failures[0]["stage"], json!("request_validation"));
    assert_eq!(failures[0]["checkpoint"], json!("provider_resolution"));
    assert_eq!(failures[0]["requestSent"], json!(false));
    assert!(failures[0].get("request").is_none());
    assert!(failures[0].get("body").is_none());
}

#[test]
fn execution_and_preview_share_the_selected_tenant_format_refusal() {
    for dry_run in [false, true] {
        let server = FixtureServer::start();
        server.seed_gateway();
        let sandbox = Sandbox::new();
        sandbox.single_tenant(&server, "prod", "synthetic-invalid-tsg");
        let f = Fx { server, sandbox };
        let mut args = deployment_args(&json!({"allow_all_workspaces": true}));
        args.push("--debug".into());
        if dry_run {
            args.push("--dry-run".into());
        } else {
            args.push("--force".into());
        }
        let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = f.gw_with(&refs, &RunOptions::new().env("XDG_STATE_HOME", &state));
        out.assert_code(2);
        assert!(
            out.stderr
                .contains("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid tsgId: synthetic-invalid-tsg")
        );
        assert!(out.stdout.is_empty());
        assert_eq!(f.server.request_count(), 0);
        let (_, records) = debug_log::only_log(&f.sandbox);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["stage"], json!("tenant_binding"));
        assert_eq!(records[0]["checkpoint"], json!("tenant_binding"));
        assert_eq!(records[0]["requestSent"], json!(false));
    }
}

#[test]
fn preview_and_execution_preserve_the_registered_tsg_pin_when_config_loses_it() {
    for dry_run in [false, true] {
        let server = FixtureServer::start();
        server.seed_gateway();
        let sandbox = Sandbox::new();
        let tenant = sandbox.add_fixture_tenant_with(&server, "prod", TSG, |config| {
            config.remove("mgmtTsgId");
            config.remove("mgmtClientId");
            config.remove("mgmtClientSecret");
        });
        sandbox.register(&[&tenant], Some("prod"));
        let f = Fx { server, sandbox };
        let mut args = deployment_args(&json!({"allow_all_workspaces": true}));
        args.extend([
            if dry_run { "--dry-run" } else { "--force" }.into(),
            "--debug".into(),
        ]);
        let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = f.gw_with(&refs, &RunOptions::new().env("XDG_STATE_HOME", &state));
        out.assert_code(1);
        assert!(out.stderr.contains(
            "Tenant config TSG differs from its registered identity; register it under a new name"
        ));
        assert!(out.stdout.is_empty());
        assert_eq!(f.server.request_count(), 0);
        let (_, records) = debug_log::only_log(&f.sandbox);
        assert!(
            records.is_empty(),
            "store identity refusal precedes preparation"
        );
    }
}
