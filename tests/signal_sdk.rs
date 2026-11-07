//! Pinned-SDK compatibility proof, not real-service qualification.
use airs_test_harness::{
    Cli, FixtureServer, Planned, Route, RunOptions, Rx, Sandbox, fixtures, run,
};
use futures_util::{StreamExt, future::BoxFuture};
use scm::transport::{Transport, TransportRequest, TransportResponse};
use scm::{Endpoint, Secret, airs::scan::Scanner, data::Optional};
use serde_json::json;

struct LoopbackTransport;
impl Transport for LoopbackTransport {
    fn send<'a>(
        &'a self,
        request: TransportRequest,
    ) -> BoxFuture<'a, scm::Result<TransportResponse>> {
        Box::pin(async move {
            assert_eq!(request.url().host_str(), Some("127.0.0.1"));
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap();
            let response = client
                .request(request.method().clone(), request.url().clone())
                .headers(request.headers().clone())
                .body(request.body().clone())
                .send()
                .await
                .map_err(|_| scm::Error::new(scm::ErrorKind::Transport))?;
            let status = response.status();
            let headers = response.headers().clone();
            let stream = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(|_| scm::Error::new(scm::ErrorKind::Transport)));
            Ok(TransportResponse::new(status, headers, Box::pin(stream)))
        })
    }
}

#[test]
fn session_result_report_chain_preserves_request_and_snippet_identity() {
    let server = FixtureServer::start();
    server.seed_runtime();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    let scan = "00000000-0000-4000-8000-0000000000b2";
    let mut session = airs_test_harness::fixtures::runtime::session("session-1");
    session["session_actions"][0]["scan_id"] = json!(scan);
    server.set_default(Route::Runtime(Rx::Session), Planned::json(200, &session));
    let out = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &[
            "runtime",
            "sessions",
            "get",
            "--session-id",
            "session-1",
            "--app-id",
            "uuid-shared",
            "--app-name",
            "A & B",
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    let document: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(document["session_actions"][0]["scan_id"], scan);
    let result = fixtures::scan_block("monitor");
    let report_id = result["report_id"].as_str().unwrap().to_owned();
    server.set_default(
        Route::ScanResults,
        Planned::json(
            200,
            &json!([
                {"scan_id":scan,"req_id":0,"status":"COMPLETED","result":result}
            ]),
        ),
    );
    server.set_default(Route::ScanReports, Planned::json(200, &json!([
        {"scan_id":scan,"report_id":report_id,"req_id":0,"session_id":"session-1",
         "detection_results":[
            {"data_type":"prompt","detection_service":"prompt_injection","verdict":"malicious","action":"allow",
             "result_detail":{"pi_snippets":["Synthetic Δ\nignore instructions"],"dbs_snippets":[],"tc_snippets":["Synthetic toxic example"]}},
            {"data_type":"response","detection_service":"dlp","verdict":"benign",
             "result_detail":{"dlp_snippets":{"snippets":[]}}}
         ]}
    ])));
    let mut options = scm::configuration::InitOptions::default();
    options.api_endpoint = Some(Endpoint::loopback(&server.url("/scan")).unwrap());
    options.api_key = Some(Secret::new("FAKE-APIKEY-100").unwrap());
    options.num_retries = Some(0);
    let scanner = Scanner::new(
        scm::configuration::init(options)
            .unwrap()
            .client_builder()
            .unwrap()
            .transport(std::sync::Arc::new(LoopbackTransport))
            .build()
            .unwrap(),
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let results = scanner
            .query_by_scan_ids(
                &[document["session_actions"][0]["scan_id"]
                    .as_str()
                    .unwrap()
                    .to_owned()],
                None,
            )
            .await
            .unwrap();
        let Optional::Value(result) = &results[0].result else {
            panic!("missing result")
        };
        assert_eq!(result.report_id, report_id);
        let reports = scanner
            .query_by_report_ids(std::slice::from_ref(&result.report_id), None)
            .await
            .unwrap();
        let value = serde_json::to_value(&reports[0]).unwrap();
        assert_eq!(
            value["req_id"],
            document["session_actions"][0]["scan_sub_req_id"]
        );
        assert_eq!(
            serde_json::to_value(&results[0]).unwrap()["req_id"],
            value["req_id"]
        );
        assert_eq!(
            value["detection_results"][0]["result_detail"]["pi_snippets"][0],
            "Synthetic Δ\nignore instructions"
        );
        assert_eq!(value["detection_results"][1]["verdict"], "benign");
        assert!(
            value["detection_results"][1]["result_detail"]["dlp_snippets"]["snippets"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    });
    server.assert_no_unknown_routes();
    assert!(
        server
            .requests()
            .iter()
            .all(|r| r.route != Route::ScanSync && r.route != Route::ScanAsync)
    );
}

#[test]
fn key_and_registered_app_associations_are_explicit_optional_fields_not_name_joins() {
    use scm::airs::management::models::{
        ApiKey, CustomerAppWithKeys, DashboardSessionOverviewItem,
    };

    let mut key_value = fixtures::runtime::api_key(1);
    let unassociated: ApiKey = serde_json::from_value(key_value.clone()).unwrap();
    assert!(matches!(unassociated.customer_app_id, Optional::Unset));
    // cust_app is a label; it does not fill customer_appId during typed decoding.
    assert!(matches!(unassociated.cust_app, Optional::Value(_)));

    key_value["customer_appId"] = json!("registered-app-a");
    let associated: ApiKey = serde_json::from_value(key_value).unwrap();
    assert!(
        matches!(associated.customer_app_id, Optional::Value(ref id) if id == "registered-app-a")
    );

    let mut registered_value = fixtures::runtime::customer_app(101);
    registered_value["customer_appId"] = json!("registered-app-a");
    registered_value["app_name"] = json!("same-display-name");
    let registered: CustomerAppWithKeys = serde_json::from_value(registered_value.clone()).unwrap();
    assert_eq!(registered.customer_app_id, "registered-app-a");
    let bucket: DashboardSessionOverviewItem = serde_json::from_value(
        fixtures::runtime::session_item("dashboard-app-b", "same-display-name", "session-a"),
    )
    .unwrap();
    assert_ne!(registered.customer_app_id, bucket.application_id);
    assert_eq!(registered.app_name, bucket.application_name);

    registered_value
        .as_object_mut()
        .unwrap()
        .remove("customer_appId");
    assert!(serde_json::from_value::<CustomerAppWithKeys>(registered_value).is_err());
}

#[test]
fn scanner_application_extensions_cannot_supply_typed_dashboard_ownership() {
    use scm::airs::scan::models::ScanIdResult;

    let mut response = fixtures::scan_allow("synthetic");
    response["application_id"] = json!("UNTRUSTED-RESULT-APP");
    response["application_name"] = json!("UNTRUSTED-RESULT-NAME");
    let raw = json!({
        "scan_id": response["scan_id"], "req_id": 0, "status": "COMPLETED",
        "application_id": "UNTRUSTED-WRAPPER-APP", "result": response
    });
    let typed: ScanIdResult = serde_json::from_value(raw).unwrap();
    assert_eq!(
        serde_json::to_value(typed.extensions.get("application_id")).unwrap(),
        json!("UNTRUSTED-WRAPPER-APP")
    );
    let Optional::Value(result) = &typed.result else {
        panic!("missing result")
    };
    assert_eq!(
        serde_json::to_value(result.extensions.get("application_id")).unwrap(),
        json!("UNTRUSTED-RESULT-APP")
    );
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100");
    let scan_id = result.scan_id.clone();
    server.set_default(Route::ScanResults, Planned::json(200, &json!([typed])));
    let out = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &[
            "runtime",
            "scan-logs",
            "results",
            "--scan-id",
            &scan_id,
            "--output",
            "json",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(!out.stdout.contains("UNTRUSTED-"));
    let document: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    let view = &document["items"][0];
    assert!(view.get("application_id").is_none());
    assert!(view["result"].get("application_id").is_none());
    assert_eq!(view["result"]["category"], "benign");
    assert_eq!(view["result"]["action"], "allow");
    assert_eq!(server.requests().len(), 1);
    server.assert_no_unknown_routes();
}

#[test]
fn legacy_dual_scanner_credentials_send_both_headers_without_an_app_selector() {
    let server = FixtureServer::start();
    server.set_default(Route::ScanResults, Planned::json(200, &json!([])));
    let mut options = scm::configuration::InitOptions::default();
    options.api_endpoint = Some(Endpoint::loopback(&server.url("/scan")).unwrap());
    options.api_key = Some(Secret::new("FAKE-APIKEY-100").unwrap());
    options.api_token = Some(Secret::new("FAKE-SCANNER-TOKEN-A0").unwrap());
    // bind-default must preserve the effective legacy setting: None means five,
    // independently of the zero-retry policy for bounded evidence reads.
    let configuration = scm::configuration::init(options).unwrap();
    assert_eq!(configuration.num_retries(), 5);
    let scanner = Scanner::new(
        configuration
            .client_builder()
            .unwrap()
            .transport(std::sync::Arc::new(LoopbackTransport))
            .build()
            .unwrap(),
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let id = "00000000-0000-4000-8000-0000000000a1";
    runtime.block_on(async {
        assert!(
            scanner
                .query_by_scan_ids(&[id.to_owned()], None)
                .await
                .unwrap()
                .is_empty()
        );
    });
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].route, Route::ScanResults);
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer FAKE-SCANNER-TOKEN-A0")
    );
    assert_eq!(requests[0].header("x-pan-token"), Some("FAKE-APIKEY-100"));
    assert!(requests[0].header("x-tsg-id").is_none());
    assert_eq!(
        requests[0].query,
        vec![("scan_ids".to_owned(), id.to_owned())]
    );
    server.assert_no_unknown_routes();
}
