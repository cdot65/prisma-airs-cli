//! M8.1 static validation of the GitOps artifacts in `deploy/airs-mcp` (no cluster
//! contact): the manifests are JSON (valid YAML) checked against the contract's
//! singleton, storage, secret, probe and network rules (§9.3, §9.4, §9.5, §11), and the
//! configuration template must pass the service's own strict parser with every path the
//! deployment mounts.

use serde_json::Value;
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/airs-mcp")
}

fn load(name: &str) -> Value {
    let text = std::fs::read_to_string(dir().join(name)).unwrap();
    let body: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::from_str(&body).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pod() -> Value {
    load("deployment.json")["spec"]["template"]["spec"].clone()
}

fn mount_for(container: &Value, path: &Path) -> Option<String> {
    container["volumeMounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| path.starts_with(m["mountPath"].as_str().unwrap()))
        .map(|m| m["name"].as_str().unwrap().to_owned())
}

/// The image digest the kustomization pins, refused while it is the placeholder.
fn deployable_digest(k: &Value) -> Result<String, &'static str> {
    let digest = k["images"][0]["digest"]
        .as_str()
        .ok_or("no digest pinned")?;
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or("not a sha256 digest")?;
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("malformed digest");
    }
    if hex.bytes().all(|b| b == b'0') {
        return Err("placeholder digest: build, push and pin the image first");
    }
    Ok(digest.to_owned())
}

#[test]
fn the_deployment_is_a_hardened_singleton() {
    let d = load("deployment.json");
    assert_eq!(d["spec"]["replicas"], 1);
    assert_eq!(
        d["spec"]["strategy"]["type"], "Recreate",
        "never two owners in a rollout"
    );
    let p = pod();
    assert_eq!(p["automountServiceAccountToken"], false);
    assert_eq!(p["securityContext"]["runAsNonRoot"], true);
    assert_eq!(
        p["securityContext"]["seccompProfile"]["type"],
        "RuntimeDefault"
    );
    assert!(
        p["securityContext"].get("fsGroup").is_none(),
        "no fsGroup (contract §9.3)"
    );
    let containers = p["containers"].as_array().unwrap();
    assert_eq!(containers.len(), 1);
    let c = &containers[0];
    assert_eq!(c["securityContext"]["readOnlyRootFilesystem"], true);
    assert_eq!(c["securityContext"]["allowPrivilegeEscalation"], false);
    assert_eq!(c["securityContext"]["capabilities"]["drop"][0], "ALL");
    assert!(c["securityContext"]["capabilities"].get("add").is_none());
    // No secret ever reaches the environment.
    for e in c["env"].as_array().unwrap() {
        assert!(e["valueFrom"].get("secretKeyRef").is_none(), "{e}");
        assert!(["POD_NAME", "NODE_NAME"].contains(&e["name"].as_str().unwrap()));
    }
    assert!(c.get("envFrom").is_none());
    // The init container only creates the subtree, with CHOWN alone.
    let init = &p["initContainers"][0];
    assert_eq!(
        init["securityContext"]["capabilities"]["add"],
        serde_json::json!(["CHOWN"])
    );
    assert_eq!(init["securityContext"]["readOnlyRootFilesystem"], true);
    assert!(
        init["image"].as_str().unwrap().contains("@sha256:"),
        "init image pinned by digest"
    );
    let script = init["command"][2].as_str().unwrap();
    assert!(
        !script.contains("-R") && !script.contains("rm "),
        "never recursive, never removes"
    );
    // Every secret volume is read-only and owner-only.
    for v in p["volumes"].as_array().unwrap() {
        if let Some(s) = v.get("secret") {
            assert_eq!(s["defaultMode"], 256, "0400: {v}");
        }
    }
    for m in c["volumeMounts"].as_array().unwrap() {
        let name = m["name"].as_str().unwrap();
        if !["data", "tmp"].contains(&name) {
            assert_eq!(m["readOnly"], true, "{m}");
        }
    }
}

#[test]
fn the_configuration_template_matches_the_deployment() {
    let text = std::fs::read_to_string(dir().join("airs-mcp.toml")).unwrap();
    let cfg = airs_mcp::config::parse(&text).expect("the template passes the strict parser");
    let p = pod();
    let c = &p["containers"][0];
    let port = |name: &str| -> u64 {
        c["ports"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["name"] == name)
            .unwrap()["containerPort"]
            .as_u64()
            .unwrap()
    };
    let listen: std::net::SocketAddr = cfg.server.listen.parse().unwrap();
    assert_eq!(u64::from(listen.port()), port("https"));
    let health: std::net::SocketAddr = cfg
        .server
        .health_listen
        .as_deref()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(u64::from(health.port()), port("health"));
    // The probes and the scrape use the private health listener only.
    assert_eq!(c["readinessProbe"]["httpGet"]["path"], "/readyz");
    assert_eq!(c["livenessProbe"]["httpGet"]["path"], "/livez");
    for probe in ["readinessProbe", "livenessProbe", "startupProbe"] {
        assert_eq!(c[probe]["httpGet"]["port"], "health", "{probe}");
    }
    let ann = &load("deployment.json")["spec"]["template"]["metadata"]["annotations"];
    assert_eq!(ann["prometheus.io/path"], "/metrics");
    assert_eq!(ann["prometheus.io/port"], health.port().to_string());
    // The data subtree is on the PVC; TLS on its secret; credentials on the projection.
    assert_eq!(mount_for(c, &cfg.server.data_dir).as_deref(), Some("data"));
    let tls = cfg.server.tls.as_ref().unwrap();
    assert_eq!(mount_for(c, &tls.cert_file).as_deref(), Some("tls"));
    assert_eq!(mount_for(c, &tls.key_file).as_deref(), Some("tls"));
    let projected: Vec<String> = p["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "airs-secrets")
        .unwrap()["secret"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap().to_owned())
        .collect();
    let mut files = vec![
        cfg.airs.management_client_id_file.clone(),
        cfg.airs.management_client_secret_file.clone(),
    ];
    for s in &cfg.airs.scanners {
        files.extend(s.api_key_file.clone());
        files.extend(s.api_token_file.clone());
    }
    for f in files {
        assert_eq!(mount_for(c, &f).as_deref(), Some("airs-secrets"), "{f:?}");
        let name = f.file_name().unwrap().to_str().unwrap();
        assert!(projected.iter().any(|p| p == name), "{name} is projected");
    }
    // The config map mounts the file the container is told to read.
    assert_eq!(c["args"][2], "/etc/airs-mcp/config/airs-mcp.toml");
    let k = load("kustomization.yaml");
    assert_eq!(k["configMapGenerator"][0]["files"][0], "airs-mcp.toml");
}

#[test]
fn storage_network_and_exposure_follow_the_contract() {
    let pvc = load("pvc.json");
    assert_eq!(
        pvc["spec"]["accessModes"],
        serde_json::json!(["ReadWriteOncePod"])
    );
    assert_eq!(
        pvc["spec"]["storageClassName"], "longhorn-airs-encrypted",
        "encrypted class (§9.4)"
    );
    assert_eq!(
        pvc["spec"]["resources"]["requests"]["storage"], "100Gi",
        "initial size (§9.5)"
    );
    let svc = load("service.json");
    let ports = svc["spec"]["ports"].as_array().unwrap();
    assert_eq!(
        ports.len(),
        1,
        "the health listener is never a Service port"
    );
    assert_eq!(ports[0]["targetPort"], "https");
    let np = load("networkpolicy.json");
    let items = np["items"].as_array().unwrap();
    let by = |name: &str| {
        items
            .iter()
            .find(|i| i["metadata"]["name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(
        by("default-deny")["spec"]["podSelector"],
        serde_json::json!({})
    );
    let gw = by("airs-mcp-from-gateway");
    assert_eq!(
        gw["spec"]["ingress"][0]["ports"],
        serde_json::json!([{"protocol": "TCP", "port": 8443}])
    );
    assert_eq!(
        gw["spec"]["ingress"][0]["from"][0]["namespaceSelector"]["matchLabels"]["kubernetes.io/metadata.name"],
        "aigw-goshawks"
    );
    let metrics = by("airs-mcp-metrics-from-monitoring");
    assert_eq!(metrics["spec"]["ingress"][0]["ports"][0]["port"], 8081);
    // No ingress rule admits the health listener from the gateway or anywhere else.
    for i in items {
        for rule in i["spec"]["ingress"].as_array().into_iter().flatten() {
            for port in rule["ports"].as_array().unwrap() {
                let p = port["port"].as_u64().unwrap();
                assert!(
                    p == 8443
                        || (p == 8081
                            && i["metadata"]["name"] == "airs-mcp-metrics-from-monitoring")
                );
            }
        }
    }
    let egress = by("airs-mcp-egress");
    let internet = egress["spec"]["egress"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["to"][0].get("ipBlock").is_some())
        .unwrap();
    let except: Vec<&str> = internet["to"][0]["ipBlock"]["except"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for private in [
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "169.254.0.0/16",
    ] {
        assert!(except.contains(&private), "egress never reaches {private}");
    }
}

#[test]
fn the_kustomization_lists_every_manifest_and_gates_on_a_pinned_digest() {
    let k = load("kustomization.yaml");
    let mut listed: Vec<String> = k["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    listed.sort();
    let mut present: Vec<String> = std::fs::read_dir(dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".json"))
        .collect();
    present.sort();
    assert_eq!(listed, present);
    assert_eq!(k["namespace"], "airs-mcp");
    // The committed placeholder is refused: the set is not deployable until the user
    // pins the digest of the image they built (runbook step 2).
    assert!(deployable_digest(&k).is_err());
    let mut pinned = k.clone();
    pinned["images"][0]["digest"] = serde_json::json!(format!("sha256:{}", "ab".repeat(32)));
    assert!(deployable_digest(&pinned).is_ok());
    pinned["images"][0]["digest"] = serde_json::json!("latest");
    assert!(deployable_digest(&pinned).is_err());
}

fn script(name: &str) -> (PathBuf, String) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/m8")
        .join(name);
    let text = std::fs::read_to_string(&p).unwrap();
    (p, text)
}

/// The M8 user-run scripts parse, and the backup-policy check stays read-only: no write
/// HTTP methods, no object operations, no cluster or storage commands.
#[test]
fn the_m8_scripts_parse_and_the_backup_check_is_read_only() {
    for name in [
        "check-r2-delete-rights.sh",
        "user-checklist.sh",
        "build-hashes.sh",
    ] {
        let (p, _) = script(name);
        let ok = std::process::Command::new("bash")
            .arg("-n")
            .arg(&p)
            .status()
            .unwrap();
        assert!(ok.success(), "{name} parses");
    }
    let (_, r2) = script("check-r2-delete-rights.sh");
    let code: String = r2
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "-X ",
        "--request",
        "DELETE",
        "PUT ",
        "POST",
        "rclone delete",
        "rclone purge",
        "rclone sync",
        "rclone copy",
        "rclone move",
        "rclone ls",
        "aws s3",
        "wrangler r2",
        "kubectl",
        "rm ",
    ] {
        assert!(
            !code.contains(forbidden),
            "check-r2-delete-rights.sh must not use {forbidden:?}"
        );
    }
    let urls: Vec<&str> = code.matches("https://").collect();
    assert_eq!(urls.len(), 2, "only the two token-inspection reads");
    assert!(
        code.contains("read -r -s"),
        "the token is read at a hidden prompt"
    );
    let (_, checklist) = script("user-checklist.sh");
    let code: String = checklist
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for line in code.lines() {
        let t = line.trim_start();
        for cmd in ["kubectl", "talosctl", "docker", "helm", "flux", "curl"] {
            assert!(
                !t.starts_with(cmd),
                "the checklist only shows commands; it runs none: {line}"
            );
        }
    }
    let (_, build) = script("build-hashes.sh");
    for forbidden in [
        "git push",
        "git tag",
        "codesign",
        "docker ",
        "podman ",
        "curl ",
        "cargo publish",
    ] {
        let code: String = build
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains(forbidden),
            "build-hashes.sh must not use {forbidden:?}"
        );
    }
}
