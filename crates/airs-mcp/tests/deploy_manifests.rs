//! M8.1 static validation of the GitOps artifacts in `deploy/airs-mcp` (no cluster
//! contact): the manifests are JSON (valid YAML) checked against the contract's
//! singleton, storage, secret, probe and network rules (§9.3, §9.4, §9.5, §11), and the
//! configuration template must pass the service's own strict parser with every path the
//! deployment mounts.
//!
//! Every rule is a check function over a manifest directory, so the same checks:
//! - pass on the committed set (`deploy/airs-mcp`, with its placeholder image digest);
//! - reject each security regression applied to a temporary copy (the mutation tests
//!   below, M8 review 1 finding 2);
//! - validate the owner's filled set before deployment:
//!   `AIRS_MCP_DEPLOY_DIR=<dir> AIRS_MCP_REQUIRE_DEPLOYABLE=1 cargo test -p airs-mcp --test
//!   deploy_manifests`, which additionally requires a valid pinned image digest.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The service's container user and group (the image's non-root user).
const SERVICE_UID: u64 = 65532;

fn committed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/airs-mcp")
}

/// The set under validation: the committed one, or the owner's (`AIRS_MCP_DEPLOY_DIR`).
fn dir() -> PathBuf {
    std::env::var_os("AIRS_MCP_DEPLOY_DIR").map_or_else(committed, PathBuf::from)
}

fn load_in(dir: &Path, name: &str) -> Value {
    let text = std::fs::read_to_string(dir.join(name)).unwrap();
    let body: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::from_str(&body).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pod_in(dir: &Path) -> Value {
    load_in(dir, "deployment.json")["spec"]["template"]["spec"].clone()
}

fn service_container(pod: &Value) -> Value {
    pod["containers"][0].clone()
}

fn init_named(pod: &Value, name: &str) -> Option<Value> {
    pod["initContainers"]
        .as_array()?
        .iter()
        .find(|c| c["name"] == name)
        .cloned()
}

/// The volume and sub-path a container reads `path` through (the longest mount prefix).
fn mount_for(container: &Value, path: &Path) -> Option<(String, Option<String>)> {
    container["volumeMounts"]
        .as_array()?
        .iter()
        .filter(|m| path.starts_with(m["mountPath"].as_str().unwrap_or("\u{0}")))
        .max_by_key(|m| m["mountPath"].as_str().unwrap_or_default().len())
        .map(|m| {
            (
                m["name"].as_str().unwrap_or_default().to_owned(),
                m["subPath"].as_str().map(str::to_owned),
            )
        })
}

fn volume<'a>(pod: &'a Value, name: &str) -> Option<&'a Value> {
    pod["volumes"]
        .as_array()?
        .iter()
        .find(|v| v["name"] == name)
}

fn ensure(ok: bool, why: impl Into<String>) -> Result<(), String> {
    if ok { Ok(()) } else { Err(why.into()) }
}

/// Every container and init-container image reference in the manifests the
/// kustomization lists (walked recursively, so CronJob and Job templates count).
fn container_images(dir: &Path) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for key in ["containers", "initContainers"] {
                    if let Some(list) = m.get(key).and_then(Value::as_array) {
                        for c in list {
                            if let Some(i) = c["image"].as_str() {
                                out.push(i.to_owned());
                            }
                        }
                    }
                }
                for child in m.values() {
                    walk(child, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    let k = load_in(dir, "kustomization.yaml");
    let mut out = Vec::new();
    for r in k["resources"].as_array().into_iter().flatten() {
        if let Some(name) = r.as_str() {
            walk(&load_in(dir, name), &mut out);
        }
    }
    out
}

fn sha256_digest_ok(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The image mappings against every image the manifests run (M8 review 3 finding 4):
/// each image is a bare name with exactly one `images` mapping, or a fully qualified
/// reference already pinned by `@sha256:`. The mappings are exactly the bare names in
/// use, none missing and none extra. Each mapping has a registry `newName` and a
/// well-formed digest. With `deployable`, every digest, mapped or literal, must be a real
/// pin, not the all-zero placeholder (M8 review 4 minor). Returns the airs-mcp image's
/// digest.
fn check_images(dir: &Path, deployable: bool) -> Result<String, String> {
    let k = load_in(dir, "kustomization.yaml");
    let images = k["images"].as_array().ok_or("no images")?;
    let mut mapped: Vec<&str> = Vec::new();
    let mut service = None;
    for image in images {
        let name = image["name"]
            .as_str()
            .ok_or("an image mapping has no name")?;
        if mapped.contains(&name) {
            return Err(format!("image {name} is mapped twice"));
        }
        mapped.push(name);
        let new_name = image["newName"].as_str().unwrap_or_default();
        if !new_name.contains('/') || new_name.contains('@') || new_name.contains(':') {
            return Err(format!("image {name} has no registry newName"));
        }
        let digest = image["digest"]
            .as_str()
            .ok_or(format!("image {name} has no digest"))?;
        if !sha256_digest_ok(digest) {
            return Err(format!("image {name} has a malformed digest"));
        }
        if deployable && is_placeholder_digest(digest) {
            return Err("placeholder digest: build, push and pin the image first".to_owned());
        }
        if name == "airs-mcp" {
            service = Some(digest.to_owned());
        }
    }
    let mut bare: Vec<String> = Vec::new();
    for reference in container_images(dir) {
        if let Some((_, digest)) = reference.split_once('@') {
            if !reference.contains('/') || !sha256_digest_ok(digest) {
                return Err(format!(
                    "image {reference} is not pinned by a sha256 digest"
                ));
            }
            if deployable && is_placeholder_digest(digest) {
                return Err(format!(
                    "placeholder digest on {reference}: pin the image first"
                ));
            }
        } else if reference.contains('/') || reference.contains(':') {
            return Err(format!("image {reference} is neither mapped nor pinned"));
        } else if !mapped.contains(&reference.as_str()) {
            return Err(format!("image {reference} has no mapping"));
        } else if !bare.contains(&reference) {
            bare.push(reference);
        }
    }
    for name in &mapped {
        if !bare.iter().any(|b| b == name) {
            return Err(format!("image mapping {name} is not used by any container"));
        }
    }
    service.ok_or_else(|| "no airs-mcp image".to_owned())
}

/// The committed placeholder: `sha256:` and 64 zeros.
fn is_placeholder_digest(digest: &str) -> bool {
    digest
        .trim_start_matches("sha256:")
        .bytes()
        .all(|b| b == b'0')
}

/// The digest of the airs-mcp image once every image is pinned (the deployable rule).
fn deployable_digest_in(dir: &Path) -> Result<String, String> {
    check_images(dir, true)
}

/// The committed images are either placeholders or valid pins (never malformed), and
/// exactly cover what the containers run.
fn images_are_placeholders_or_pinned(dir: &Path) -> Result<(), String> {
    check_images(dir, false).map(|_| ())
}

/// The singleton, the pod and every container's hardening (contract §9.3).
fn check_hardening(dir: &Path) -> Result<(), String> {
    let d = load_in(dir, "deployment.json");
    ensure(d["spec"]["replicas"] == 1, "exactly one replica")?;
    ensure(
        d["spec"]["strategy"]["type"] == "Recreate",
        "Recreate: never two owners in a rollout",
    )?;
    let p = pod_in(dir);
    ensure(
        p["automountServiceAccountToken"] == false,
        "no service account token",
    )?;
    for host in ["hostNetwork", "hostPID", "hostIPC"] {
        ensure(p[host] != true, format!("no {host}"))?;
    }
    let sc = &p["securityContext"];
    ensure(sc["runAsNonRoot"] == true, "pod runAsNonRoot")?;
    ensure(sc["runAsUser"] == SERVICE_UID, "pod runAsUser 65532")?;
    ensure(
        sc["seccompProfile"]["type"] == "RuntimeDefault",
        "RuntimeDefault seccomp",
    )?;
    ensure(sc.get("fsGroup").is_none(), "no fsGroup (contract §9.3)")?;
    for v in p["volumes"].as_array().ok_or("volumes")? {
        ensure(
            v.get("hostPath").is_none(),
            format!("no hostPath volume: {v}"),
        )?;
    }
    let containers = p["containers"].as_array().ok_or("containers")?;
    ensure(containers.len() == 1, "one service container")?;
    let c = &containers[0];
    let csc = &c["securityContext"];
    ensure(
        csc["privileged"] != true,
        "the service container is not privileged",
    )?;
    ensure(
        csc["allowPrivilegeEscalation"] == false,
        "no privilege escalation",
    )?;
    ensure(
        csc["readOnlyRootFilesystem"] == true,
        "read-only root filesystem",
    )?;
    ensure(csc["runAsNonRoot"] == true, "container runAsNonRoot")?;
    ensure(csc["runAsUser"] == SERVICE_UID, "container runAsUser 65532")?;
    ensure(
        csc["capabilities"]["drop"] == json!(["ALL"]),
        "drop ALL capabilities",
    )?;
    ensure(
        csc["capabilities"].get("add").is_none(),
        "no added capabilities",
    )?;
    // No secret ever reaches the environment.
    for e in c["env"].as_array().into_iter().flatten() {
        ensure(
            e["valueFrom"].get("secretKeyRef").is_none(),
            format!("no secret in the environment: {e}"),
        )?;
        ensure(
            ["POD_NAME", "NODE_NAME"].contains(&e["name"].as_str().unwrap_or_default()),
            format!("only POD_NAME and NODE_NAME: {e}"),
        )?;
    }
    ensure(c.get("envFrom").is_none(), "no envFrom")?;
    for m in c["volumeMounts"].as_array().ok_or("service mounts")? {
        let name = m["name"].as_str().unwrap_or_default();
        if !["data", "tmp"].contains(&name) {
            ensure(m["readOnly"] == true, format!("read-only mount: {m}"))?;
        }
    }
    // Init containers: pinned images, CHOWN alone, never privileged, never recursive.
    for init in p["initContainers"].as_array().ok_or("init containers")? {
        let isc = &init["securityContext"];
        ensure(
            init["image"]
                .as_str()
                .unwrap_or_default()
                .contains("@sha256:"),
            format!("init image pinned by digest: {}", init["name"]),
        )?;
        ensure(
            isc["privileged"] != true,
            "init containers are not privileged",
        )?;
        ensure(
            isc["allowPrivilegeEscalation"] == false,
            "init: no escalation",
        )?;
        ensure(
            isc["readOnlyRootFilesystem"] == true,
            "init: read-only root",
        )?;
        ensure(
            isc["capabilities"]["drop"] == json!(["ALL"]),
            "init: drop ALL",
        )?;
        ensure(
            isc["capabilities"]["add"] == json!(["CHOWN"]),
            "init: CHOWN is the only added capability",
        )?;
        let script = init["command"][2].as_str().unwrap_or_default();
        ensure(
            !script.contains("-R") && !script.contains("rm "),
            "init: never recursive, never removes",
        )?;
    }
    Ok(())
}

/// Secrets reach the service only through an in-memory volume of files owned by UID
/// 65532 at 0400 (the projected Secret files are root-owned and there is no fsGroup):
/// the `secrets` init container is the only reader of the raw Secret volumes, and it
/// copies them with umask 0377 and chowns exactly what it wrote (M8 review 1 finding 1).
fn check_secret_delivery(dir: &Path) -> Result<(), String> {
    let p = pod_in(dir);
    let raw: Vec<String> = p["volumes"]
        .as_array()
        .ok_or("volumes")?
        .iter()
        .filter(|v| v.get("secret").is_some())
        .map(|v| v["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    ensure(!raw.is_empty(), "the Secret volumes are present")?;
    for name in &raw {
        let v = volume(&p, name).unwrap();
        ensure(
            v["secret"]["defaultMode"] == 256,
            format!("raw Secret volume {name} is 0400"),
        )?;
    }
    let delivered = volume(&p, "secrets").ok_or("the in-memory secrets volume")?;
    ensure(
        delivered["emptyDir"]["medium"] == "Memory",
        "delivered secrets live in memory only",
    )?;
    let c = service_container(&p);
    for m in c["volumeMounts"].as_array().ok_or("service mounts")? {
        let name = m["name"].as_str().unwrap_or_default();
        ensure(
            !raw.iter().any(|r| r == name),
            format!("the service never mounts a raw Secret volume: {name}"),
        )?;
        if name == "secrets" {
            ensure(m["readOnly"] == true, "the delivered secrets are read-only")?;
        }
    }
    let init = init_named(&p, "secrets").ok_or("the secrets init container")?;
    for other in p["initContainers"].as_array().ok_or("init containers")? {
        if other["name"] == "secrets" {
            continue;
        }
        for m in other["volumeMounts"].as_array().into_iter().flatten() {
            let name = m["name"].as_str().unwrap_or_default();
            ensure(
                !raw.iter().any(|r| r == name) && name != "secrets",
                format!("only the secrets init container reads Secret material: {other}"),
            )?;
        }
    }
    for m in init["volumeMounts"].as_array().ok_or("init mounts")? {
        let name = m["name"].as_str().unwrap_or_default();
        if raw.iter().any(|r| r == name) {
            ensure(m["readOnly"] == true, "raw Secret volumes are read-only")?;
        }
    }
    let script = init["command"][2].as_str().unwrap_or_default();
    ensure(script.contains("umask 0377"), "copies are created 0400")?;
    ensure(
        script.contains("chown 65532:65532"),
        "copies are owned by the service user",
    )?;
    secret_copy_order(script)
}

/// The secrets init container runs as root with only CAP_CHOWN (no DAC override), so
/// once a directory is 0500 and owned by 65532 it can no longer search it (M8 review 2
/// finding 1). Every file must therefore be chowned while its directory still belongs
/// to root, and each directory's mode and owner change only after the last command
/// that names one of its children.
fn secret_copy_order(script: &str) -> Result<(), String> {
    let commands: Vec<&str> = script.split(';').map(str::trim).collect();
    for d in ["/secrets/tls", "/secrets/airs"] {
        let child = format!("{d}/");
        let names_dir = |c: &str| c.split_whitespace().any(|t| t == d) && !c.starts_with("stat ");
        let last_child = commands
            .iter()
            .rposition(|c| c.contains(&child) && !c.starts_with("stat "))
            .ok_or(format!("{d}: no command writes its files"))?;
        let first_dir_change = commands
            .iter()
            .position(|c| (c.starts_with("chmod") || c.starts_with("chown")) && names_dir(c))
            .ok_or(format!("{d}: its mode and owner are never set"))?;
        ensure(
            last_child < first_dir_change,
            format!(
                "{d}: a command on its files ({:?}) follows the directory's mode or owner change ({:?})",
                commands[last_child], commands[first_dir_change]
            ),
        )?;
        ensure(
            commands
                .iter()
                .any(|c| c.starts_with("chmod 0500") && names_dir(c)),
            format!("{d} ends 0500"),
        )?;
        ensure(
            commands
                .iter()
                .any(|c| c.starts_with("chown 65532:65532") && names_dir(c)),
            format!("{d} ends owned by 65532"),
        )?;
    }
    Ok(())
}

/// The configuration template, the container and the Service agree: listeners, probes,
/// the scrape, every mounted path, and the Secret keys each delivered file comes from.
fn check_configuration(dir: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(dir.join("airs-mcp.toml")).unwrap();
    let cfg = airs_mcp::config::parse(&text).map_err(|e| format!("strict parser: {e}"))?;
    check_proof_grants(&cfg)?;
    let p = pod_in(dir);
    let c = service_container(&p);
    let port = |name: &str| -> Option<u64> {
        c["ports"].as_array()?.iter().find(|x| x["name"] == name)?["containerPort"].as_u64()
    };
    let listen: std::net::SocketAddr = cfg.server.listen.parse().map_err(|_| "listen")?;
    ensure(
        Some(u64::from(listen.port())) == port("https"),
        "the TLS listener is the https container port",
    )?;
    let health: std::net::SocketAddr = cfg
        .server
        .health_listen
        .as_deref()
        .ok_or("health_listen")?
        .parse()
        .map_err(|_| "health_listen")?;
    ensure(
        Some(u64::from(health.port())) == port("health"),
        "the health listener is the health container port",
    )?;
    ensure(
        c["readinessProbe"]["httpGet"]["path"] == "/readyz",
        "readiness",
    )?;
    ensure(
        c["livenessProbe"]["httpGet"]["path"] == "/livez",
        "liveness",
    )?;
    for probe in ["readinessProbe", "livenessProbe", "startupProbe"] {
        ensure(
            c[probe]["httpGet"]["port"] == "health",
            format!("{probe} uses the private health listener"),
        )?;
    }
    let ann = &load_in(dir, "deployment.json")["spec"]["template"]["metadata"]["annotations"];
    ensure(ann["prometheus.io/path"] == "/metrics", "scrape path")?;
    ensure(
        ann["prometheus.io/port"].as_str() == Some(health.port().to_string().as_str()),
        "the scrape uses the health listener",
    )?;
    ensure(
        mount_for(&c, &cfg.server.data_dir).map(|m| m.0).as_deref() == Some("data"),
        "the data subtree is on the PVC",
    )?;
    // Each credential file is delivered from the matching Secret key.
    let init = init_named(&p, "secrets").ok_or("the secrets init container")?;
    let script = init["command"][2].as_str().unwrap_or_default().to_owned();
    let delivered = |file: &Path, sub: &str, source_volume: &str| -> Result<(), String> {
        let (vol, subpath) = mount_for(&c, file).ok_or(format!("{file:?} is mounted"))?;
        ensure(
            vol == "secrets",
            format!("{file:?} comes from the delivered secrets"),
        )?;
        ensure(
            subpath.as_deref() == Some(sub),
            format!("{file:?} is the {sub} sub-path"),
        )?;
        let name = file.file_name().unwrap().to_str().unwrap();
        ensure(
            script.contains(&format!("/secrets/{sub}/{name}")),
            format!("{name} is copied into {sub}"),
        )?;
        let v = volume(&p, source_volume).ok_or(format!("{source_volume} volume"))?;
        let projected = v["secret"]["items"].as_array().map_or_else(
            // A Secret without items projects every key (the TLS Secret).
            || true,
            |items| items.iter().any(|i| i["path"] == name),
        );
        ensure(
            projected,
            format!("{name} is projected from {source_volume}"),
        )?;
        Ok(())
    };
    let tls = cfg.server.tls.as_ref().ok_or("tls")?;
    delivered(&tls.cert_file, "tls", "tls-source")?;
    delivered(&tls.key_file, "tls", "tls-source")?;
    let mut files = vec![
        cfg.airs.management_client_id_file.clone(),
        cfg.airs.management_client_secret_file.clone(),
    ];
    for s in &cfg.airs.scanners {
        files.extend(s.api_key_file.clone());
        files.extend(s.api_token_file.clone());
    }
    for f in files {
        delivered(&f, "airs", "airs-secrets-source")?;
    }
    ensure(
        c["args"][2] == "/etc/airs-mcp/config/airs-mcp.toml",
        "the container reads the mounted configuration",
    )?;
    let k = load_in(dir, "kustomization.yaml");
    ensure(
        k["configMapGenerator"][0]["files"][0] == "airs-mcp.toml",
        "the config map carries the template",
    )?;
    Ok(())
}

/// The certificate covers exactly the hostnames the service answers for, and is the
/// Secret the TLS copy reads (M8 review 1 finding 2).
fn check_certificate(dir: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(dir.join("airs-mcp.toml")).unwrap();
    let cfg = airs_mcp::config::parse(&text).map_err(|e| format!("strict parser: {e}"))?;
    let cert = load_in(dir, "certificate.json");
    let mut names: Vec<String> = cert["spec"]["dnsNames"]
        .as_array()
        .ok_or("dnsNames")?
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_owned())
        .collect();
    names.sort();
    let mut hosts = cfg.server.allowed_hosts.clone();
    hosts.sort();
    ensure(
        names == hosts,
        format!("certificate dnsNames {names:?} equal allowed_hosts {hosts:?}"),
    )?;
    let resource_host = url::Url::parse(&cfg.server.resource)
        .map_err(|_| "resource URL")?
        .host_str()
        .unwrap_or_default()
        .to_owned();
    ensure(
        names.contains(&resource_host),
        format!("the certificate covers the resource host {resource_host}"),
    )?;
    let p = pod_in(dir);
    let tls = volume(&p, "tls-source").ok_or("tls-source volume")?;
    ensure(
        tls["secret"]["secretName"] == cert["spec"]["secretName"],
        "the TLS copy reads the certificate's Secret",
    )?;
    Ok(())
}

/// Storage and the Service (contract §9.4, §9.5): the health listener is never exposed.
fn check_storage_and_service(dir: &Path) -> Result<(), String> {
    let pvc = load_in(dir, "pvc.json");
    ensure(
        pvc["spec"]["accessModes"] == json!(["ReadWriteOncePod"]),
        "RWOP",
    )?;
    ensure(
        pvc["spec"]["storageClassName"] == "longhorn-airs-encrypted",
        "encrypted class (§9.4)",
    )?;
    ensure(
        pvc["spec"]["resources"]["requests"]["storage"] == "100Gi",
        "initial size (§9.5)",
    )?;
    let svc = load_in(dir, "service.json");
    let ports = svc["spec"]["ports"].as_array().ok_or("service ports")?;
    ensure(
        ports.len() == 1 && ports[0]["targetPort"] == "https",
        "the Service exposes only the TLS listener",
    )?;
    ensure(
        svc["spec"].get("type").is_none() || svc["spec"]["type"] == "ClusterIP",
        "a ClusterIP Service only",
    )?;
    let labels = &load_in(dir, "deployment.json")["spec"]["template"]["metadata"]["labels"];
    for (k, v) in svc["spec"]["selector"].as_object().ok_or("selector")? {
        ensure(&labels[k] == v, "the Service selects the pod")?;
    }
    Ok(())
}

const PRIVATE: [&str; 5] = [
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "100.64.0.0/10",
    "169.254.0.0/16",
];

/// The complete effective network rules (M8 review 1 finding 2): default deny; TLS
/// ingress only from the gateway pods of the gateway namespace; the health listener
/// only from the monitoring namespace; egress only to DNS, the in-cluster ingress on
/// 443, and public addresses on 443. A rule without `from`/`to` (all peers) or without
/// `ports` (all ports), an empty or partial selector, an extra policy or an IPv6
/// catch-all is refused.
fn check_network(dir: &Path) -> Result<(), String> {
    let np = load_in(dir, "networkpolicy.json");
    let items = np["items"].as_array().ok_or("items")?;
    let mut names: Vec<&str> = items
        .iter()
        .map(|i| i["metadata"]["name"].as_str().unwrap_or_default())
        .collect();
    names.sort_unstable();
    ensure(
        names
            == [
                "airs-mcp-egress",
                "airs-mcp-from-gateway",
                "airs-mcp-metrics-from-monitoring",
                "default-deny",
            ],
        format!("exactly the four policies: {names:?}"),
    )?;
    let by = |name: &str| {
        items
            .iter()
            .find(|i| i["metadata"]["name"] == name)
            .cloned()
            .unwrap()
    };
    let deny = by("default-deny");
    ensure(
        deny["spec"]["podSelector"] == json!({})
            && deny["spec"]["policyTypes"] == json!(["Ingress", "Egress"])
            && deny["spec"].get("ingress").is_none()
            && deny["spec"].get("egress").is_none(),
        "default deny of all ingress and egress",
    )?;
    let ns = |name: &str| json!({"matchLabels": {"kubernetes.io/metadata.name": name}});
    let tcp = |port: u64| json!([{"protocol": "TCP", "port": port}]);
    let mine = json!({"matchLabels": {"app.kubernetes.io/name": "airs-mcp"}});
    // Ingress: each policy has exactly one rule with exactly one peer and one port.
    // Every policy's spec is exactly its selector, its one direction and that
    // direction's rules (M8 review 2 finding 2): an ingress policy can never also carry
    // egress, and the reverse.
    let exact = |name: &str, kind: &str, types: Value| -> Result<(), String> {
        let p = by(name);
        let mut keys: Vec<&str> = p["spec"]
            .as_object()
            .ok_or(format!("{name} spec"))?
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let mut want = vec!["podSelector", "policyTypes", kind];
        want.sort_unstable();
        ensure(
            keys == want,
            format!("{name}: spec is exactly {want:?}, not {keys:?}"),
        )?;
        ensure(
            p["spec"]["policyTypes"] == types,
            format!("{name}: policyTypes exactly {types}"),
        )
    };
    exact("airs-mcp-from-gateway", "ingress", json!(["Ingress"]))?;
    exact(
        "airs-mcp-metrics-from-monitoring",
        "ingress",
        json!(["Ingress"]),
    )?;
    exact("airs-mcp-egress", "egress", json!(["Egress"]))?;
    let only_rule = |name: &str, kind: &str| -> Result<Value, String> {
        let p = by(name);
        ensure(
            p["spec"]["podSelector"] == mine,
            format!("{name} selects the service"),
        )?;
        let rules = p["spec"][kind].as_array().ok_or(format!("{name} {kind}"))?;
        ensure(rules.len() == 1, format!("{name}: one {kind} rule"))?;
        Ok(rules[0].clone())
    };
    let gw = only_rule("airs-mcp-from-gateway", "ingress")?;
    ensure(gw["ports"] == tcp(8443), "TLS ingress on 8443 only")?;
    ensure(
        gw["from"]
            == json!([{
                "namespaceSelector": ns("aigw-goshawks"),
                "podSelector": {"matchLabels": {
                    "app.kubernetes.io/instance": "aigw-goshawks",
                    "app.kubernetes.io/name": "airs-gw",
                    "goshawks.io/component": "gateway"
                }}
            }]),
        "TLS ingress only from the gateway pods of the gateway namespace",
    )?;
    let metrics = only_rule("airs-mcp-metrics-from-monitoring", "ingress")?;
    ensure(
        metrics["ports"] == tcp(8081),
        "the health listener on 8081 only",
    )?;
    ensure(
        metrics["from"] == json!([{"namespaceSelector": ns("monitoring")}]),
        "the health listener only from the monitoring namespace",
    )?;
    // Egress: exactly DNS, the in-cluster ingress on 443, and public 443.
    let egress = by("airs-mcp-egress");
    ensure(
        egress["spec"]["podSelector"] == mine,
        "egress selects the service",
    )?;
    ensure(
        egress["spec"]["policyTypes"] == json!(["Egress"]),
        "egress policy type",
    )?;
    let rules = egress["spec"]["egress"].as_array().ok_or("egress rules")?;
    ensure(rules.len() == 3, "exactly three egress rules")?;
    for r in rules {
        let to = r["to"].as_array().ok_or("an egress rule names its peers")?;
        ensure(!to.is_empty(), "no egress rule to every peer")?;
        let ports = r["ports"]
            .as_array()
            .ok_or("an egress rule names its ports")?;
        ensure(!ports.is_empty(), "no egress rule on every port")?;
    }
    ensure(
        rules[0]
            == json!({
                "to": [{"namespaceSelector": ns("kube-system"),
                        "podSelector": {"matchLabels": {"k8s-app": "kube-dns"}}}],
                "ports": [{"protocol": "UDP", "port": 53}, {"protocol": "TCP", "port": 53}]
            }),
        "DNS only to kube-dns",
    )?;
    ensure(
        rules[1] == json!({"to": [{"namespaceSelector": ns("traefik")}], "ports": tcp(443)}),
        "in-cluster ingress on 443 only",
    )?;
    let public = &rules[2];
    ensure(public["ports"] == tcp(443), "public egress on 443 only")?;
    let peers = public["to"].as_array().unwrap();
    ensure(peers.len() == 1, "one public peer")?;
    let block = &peers[0]["ipBlock"];
    ensure(block["cidr"] == "0.0.0.0/0", "IPv4 public block")?;
    let except: Vec<&str> = block["except"]
        .as_array()
        .ok_or("except")?
        .iter()
        .map(|v| v.as_str().unwrap_or_default())
        .collect();
    for private in PRIVATE {
        ensure(
            except.contains(&private),
            format!("public egress never reaches {private}"),
        )?;
    }
    Ok(())
}

/// The daily coordinated backup (contract §9.4; M8 review 2 finding 10). An
/// orchestrator CronJob stops the service, runs the coordinated set from a suspended
/// template Job, waits for it, and always scales the service back up. The PVC is RWOP,
/// so the backup pod can only be scheduled once the service pod is gone. RBAC is
/// minimal; the backup pod carries the service's hardening and no Secret, and only the
/// orchestrator reaches the API server.
fn check_backup(dir: &Path) -> Result<(), String> {
    let list = load_in(dir, "backup-cronjob.json");
    let items = list["items"].as_array().ok_or("backup items")?;
    let mut kinds: Vec<(String, String)> = items
        .iter()
        .map(|i| {
            (
                i["kind"].as_str().unwrap_or_default().to_owned(),
                i["metadata"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect();
    kinds.sort();
    let want: Vec<(String, String)> = [
        ("CronJob", "airs-mcp-backup"),
        ("CronJob", "airs-mcp-backup-set"),
        ("NetworkPolicy", "airs-mcp-backup-to-apiserver"),
        ("Role", "airs-mcp-backup"),
        ("RoleBinding", "airs-mcp-backup"),
        ("ServiceAccount", "airs-mcp-backup"),
    ]
    .iter()
    .map(|(k, n)| ((*k).to_owned(), (*n).to_owned()))
    .collect();
    ensure(
        kinds == want,
        format!("exactly the backup resources: {kinds:?}"),
    )?;
    let get = |kind: &str, name: &str| {
        items
            .iter()
            .find(|i| i["kind"] == kind && i["metadata"]["name"] == name)
            .cloned()
            .unwrap()
    };
    // RBAC: exactly these rules.
    let role = get("Role", "airs-mcp-backup");
    ensure(
        role["rules"]
            == json!([
                {"apiGroups": ["apps"], "resources": ["deployments"], "resourceNames": ["airs-mcp"], "verbs": ["get"]},
                {"apiGroups": ["apps"], "resources": ["deployments/scale"], "resourceNames": ["airs-mcp"], "verbs": ["get", "patch", "update"]},
                {"apiGroups": [""], "resources": ["pods"], "verbs": ["get", "list", "watch"]},
                {"apiGroups": ["batch"], "resources": ["cronjobs"], "resourceNames": ["airs-mcp-backup-set"], "verbs": ["get"]},
                {"apiGroups": ["batch"], "resources": ["jobs"], "verbs": ["create", "get", "list", "watch"]}
            ]),
        "the backup Role grants exactly: scale the airs-mcp Deployment, read pods, read the template CronJob, create and watch Jobs",
    )?;
    let binding = get("RoleBinding", "airs-mcp-backup");
    ensure(
        binding["roleRef"]
            == json!({"apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "airs-mcp-backup"})
            && binding["subjects"]
                == json!([{"kind": "ServiceAccount", "name": "airs-mcp-backup", "namespace": "airs-mcp"}]),
        "the binding names only the backup ServiceAccount and Role",
    )?;
    let sa = get("ServiceAccount", "airs-mcp-backup");
    ensure(
        sa["automountServiceAccountToken"] == false,
        "the ServiceAccount does not mount its token by default",
    )?;
    let hardened = |c: &Value, what: &str| -> Result<(), String> {
        let sc = &c["securityContext"];
        ensure(
            sc["allowPrivilegeEscalation"] == false
                && sc["readOnlyRootFilesystem"] == true
                && sc["privileged"] == false
                && sc["capabilities"] == json!({"drop": ["ALL"]})
                && sc["runAsNonRoot"] == true
                && sc["runAsUser"] == 65532,
            format!("{what}: non-root, read-only, unprivileged, no capabilities"),
        )
    };
    let pod_hardened = |p: &Value, what: &str| -> Result<(), String> {
        ensure(
            p["securityContext"]["runAsNonRoot"] == true
                && p["securityContext"]["runAsUser"] == 65532
                && p["securityContext"].get("fsGroup").is_none()
                && p["hostNetwork"] == false
                && p["hostPID"] == false
                && p["hostIPC"] == false
                && p["restartPolicy"] == "Never",
            format!("{what}: pod hardening"),
        )
    };
    // The orchestrator.
    let orch = get("CronJob", "airs-mcp-backup");
    let spec = &orch["spec"];
    ensure(
        spec["schedule"] == "0 3 * * *"
            && spec["timeZone"] == "America/Chicago"
            && spec["concurrencyPolicy"] == "Forbid"
            && spec.get("suspend").is_none_or(|s| s == false),
        "the orchestrator runs daily at 03:00 Chicago, never concurrently",
    )?;
    let job = &spec["jobTemplate"]["spec"];
    ensure(
        job["backoffLimit"] == 0,
        "the orchestrator never retries blindly",
    )?;
    let ap = &job["template"]["spec"];
    pod_hardened(ap, "orchestrator")?;
    ensure(
        ap["serviceAccountName"] == "airs-mcp-backup" && ap["automountServiceAccountToken"] == true,
        "only the orchestrator carries the backup ServiceAccount token",
    )?;
    ensure(
        ap["volumes"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|v| v.get("persistentVolumeClaim").is_none() && v.get("secret").is_none()),
        "the orchestrator mounts no data volume and no Secret",
    )?;
    let containers = ap["containers"]
        .as_array()
        .ok_or("orchestrator containers")?;
    ensure(containers.len() == 1, "one orchestrator container")?;
    hardened(&containers[0], "orchestrator")?;
    ensure(
        containers[0]["image"] == "kubectl",
        "the pinned kubectl image",
    )?;
    let script = containers[0]["command"][2]
        .as_str()
        .ok_or("orchestrator script")?;
    let at = |needle: &str| {
        script
            .find(needle)
            .ok_or(format!("orchestrator step {needle:?}"))
    };
    // The exit trap (defined first) waits until every set-Job pod is terminal (the RWOP
    // volume is released; a completed Job keeps its pod, so deletion is never awaited),
    // scales back up and waits for availability, then reports. The body stops the
    // service, runs the set, and polls the Job's own terminal conditions.
    let phases = at("phases() {")?;
    let released = at("released() {")?;
    let restore = at("restore() {")?;
    let release_wait = at("until released; do")?;
    let up = at("scale deployment/airs-mcp --replicas=1")?;
    let ready = at("availableReplicas")?;
    let restored = at("service_restored")?;
    let daily = at("backup_set_daily_complete")?;
    let trap = at("trap restore EXIT")?;
    let down = at("--replicas=0")?;
    let gone = at("wait --for=delete pod -l app.kubernetes.io/name=airs-mcp")?;
    let create = at("create job \"$RUN\" --from=cronjob/airs-mcp-backup-set")?;
    let poll = at("get job \"$RUN\"")?;
    let verified = at("backup_set_verified")?;
    ensure(
        script.starts_with("set -eu;")
            && phases < released
            && released < restore
            && restore < release_wait
            && release_wait < up
            && up < ready
            && ready < restored
            && restored < daily
            && daily < trap
            && trap < down
            && down < gone
            && gone < create
            && create < poll
            && poll < verified
            && script.contains("[ \"$OK\" = 1 ] && [ \"$svc\" = 0 ] && [ \"$rc\" = 0 ]")
            && script.contains("trap 'exit 143' TERM INT")
            && script.contains("trap - TERM INT")
            && script.contains("grep -qvE '^(Succeeded|Failed)$'")
            && script.contains("*Complete=True*) break ;; *Failed=True*) exit 1 ;;")
            && !script.contains("wait --for=delete pod -l job-name"),
        "stop the service, run the set and poll the Job's terminal conditions; on any exit \
         wait until every set pod is terminal (never for its deletion: a completed Job \
         keeps its pod), scale back up and wait until available; report the verified \
         backup and the restored service separately, and daily completion only for both",
    )?;
    // Every kubectl call is bounded.
    ensure(
        script.contains("K=\"kubectl -n $NS --request-timeout="),
        "every kubectl call carries a request timeout",
    )?;
    ensure(
        !script.replace("K=\"kubectl", "").contains("kubectl"),
        "no kubectl call bypasses $K",
    )?;
    // Budgets (M8 review 4 finding 1): the full path's sequential bounds (stop wait, Job
    // poll, release wait, readiness, margin) sum strictly below the orchestrator's
    // deadline; the set Job's own deadline fails it before the poll gives up and before
    // the release wait does; and the termination path (the exit trap alone, plus its API
    // calls) fits the pod's grace period, so a SIGTERM or a deadline still restores the
    // service.
    let num_after = |needle: &str| -> Result<u64, String> {
        let i = script.find(needle).ok_or(format!("bound {needle:?}"))? + needle.len();
        script[i..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u64>()
            .map_err(|_| format!("bound {needle:?} is not a number"))
    };
    let bound_after = |anchor: &str| -> Result<u64, String> {
        let i = script.find(anchor).ok_or(format!("bound {anchor:?}"))?;
        let rest = &script[i..];
        let j = rest
            .find("if [ \"$i\" -ge ")
            .ok_or(format!("no -ge bound after {anchor:?}"))?;
        rest[j + "if [ \"$i\" -ge ".len()..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u64>()
            .map_err(|_| format!("bound after {anchor:?} is not a number"))
    };
    let request = num_after("--request-timeout=")?;
    let stop = num_after("app.kubernetes.io/name=airs-mcp --timeout=")?;
    let release_bound = bound_after("until released; do")?;
    let ready_bound = bound_after("availableReplicas")?;
    let poll_bound = bound_after("get job \"$RUN\"")?;
    let orch_deadline = job["activeDeadlineSeconds"]
        .as_u64()
        .ok_or("orchestrator deadline")?;
    let grace = ap["terminationGracePeriodSeconds"]
        .as_u64()
        .ok_or("orchestrator terminationGracePeriodSeconds")?;
    let tmpl_deadline = items
        .iter()
        .find(|i| i["kind"] == "CronJob" && i["metadata"]["name"] == "airs-mcp-backup-set")
        .and_then(|t| t["spec"]["jobTemplate"]["spec"]["activeDeadlineSeconds"].as_u64())
        .ok_or("set Job deadline")?;
    let margin = 300;
    let full_path = stop + poll_bound + release_bound + ready_bound + margin;
    let termination_path = release_bound + ready_bound + 4 * request + margin;
    ensure(
        (1..=60).contains(&request)
            && tmpl_deadline < poll_bound
            && tmpl_deadline + 60 <= release_bound
            && full_path < orch_deadline
            && termination_path <= grace,
        format!(
            "budgets: request {request} s; set Job {tmpl_deadline} s < poll {poll_bound} s; \
             release wait {release_bound} s >= Job + 60; full path stop {stop} + poll \
             {poll_bound} + release {release_bound} + ready {ready_bound} + margin {margin} = \
             {full_path} < orchestrator deadline {orch_deadline}; termination path release + \
             ready + 4 requests + margin = {termination_path} <= grace {grace}"
        ),
    )?;
    // The suspended template the orchestrator instantiates.
    let tmpl = get("CronJob", "airs-mcp-backup-set");
    ensure(
        tmpl["spec"]["suspend"] == true,
        "the set template never runs on its own",
    )?;
    let tjob = &tmpl["spec"]["jobTemplate"]["spec"];
    ensure(
        tjob["backoffLimit"] == 0,
        "a failed set is never retried blindly",
    )?;
    let bp = &tjob["template"]["spec"];
    pod_hardened(bp, "backup pod")?;
    ensure(
        bp["automountServiceAccountToken"] == false && bp.get("serviceAccountName").is_none(),
        "the backup pod has no API credential",
    )?;
    ensure(
        tjob["template"]["metadata"]["labels"]["app.kubernetes.io/name"] == "airs-mcp-backup",
        "the backup pod is not selected as the service (its own label)",
    )?;
    let vols = bp["volumes"].as_array().ok_or("backup volumes")?;
    ensure(
        vols.iter().all(|v| v.get("secret").is_none()),
        "the backup pod mounts no Secret: maintenance needs none",
    )?;
    let data = vols
        .iter()
        .find(|v| v["name"] == "data")
        .ok_or("the backup pod mounts the data volume")?;
    let service_claim = pod_in(dir)["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "data")
        .map(|v| v["persistentVolumeClaim"]["claimName"].clone())
        .unwrap_or_default();
    ensure(
        data["persistentVolumeClaim"]["claimName"] == service_claim,
        "the backup pod mounts the service's own PVC",
    )?;
    let set = bp["initContainers"].as_array().ok_or("backup init")?;
    let verify = bp["containers"].as_array().ok_or("backup containers")?;
    ensure(
        set.len() == 1 && verify.len() == 1,
        "one set step and one verify step",
    )?;
    hardened(&set[0], "backup-set")?;
    hardened(&verify[0], "verify")?;
    ensure(
        set[0]["image"] == "airs-mcp" && verify[0]["image"] == "airs-mcp",
        "both steps run the pinned service image",
    )?;
    ensure(
        set[0]["args"]
            == json!([
                "control-backup-set",
                "--config",
                "/etc/airs-mcp/config/airs-mcp.toml",
                "--dir",
                "/data/airs/backups",
                "--keep",
                "3"
            ]),
        "the set step writes the coordinated set to DATA/backups and keeps the newest 3 verified sets",
    )?;
    ensure(
        verify[0]["args"] == json!(["control-backup-verify", "--latest-in", "/data/airs/backups"]),
        "the verify step verifies the newest set",
    )?;
    ensure(
        verify[0]["volumeMounts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| m["name"] == "data")
            .all(|m| m["readOnly"] == true),
        "verification mounts the data read-only",
    )?;
    // Only the orchestrator reaches the API server; the backup pod has no network.
    let np = get("NetworkPolicy", "airs-mcp-backup-to-apiserver");
    ensure(
        np["spec"]
            == json!({
                "podSelector": {"matchLabels": {"app.kubernetes.io/name": "airs-mcp-backup-orchestrator"}},
                "policyTypes": ["Egress"],
                "egress": [{
                    "to": [
                        {"ipBlock": {"cidr": "192.168.255.10/32"}},
                        {"ipBlock": {"cidr": "192.168.255.11/32"}},
                        {"ipBlock": {"cidr": "192.168.255.12/32"}},
                        {"ipBlock": {"cidr": "192.168.255.13/32"}}
                    ],
                    "ports": [{"protocol": "TCP", "port": 6443}]
                }]
            }),
        "the orchestrator reaches only the API server on 6443",
    )?;
    Ok(())
}

/// Every check over a manifest directory.
fn validate(dir: &Path) -> Result<(), String> {
    check_hardening(dir)?;
    check_backup(dir)?;
    check_secret_delivery(dir)?;
    check_configuration(dir)?;
    check_certificate(dir)?;
    check_storage_and_service(dir)?;
    check_network(dir)?;
    images_are_placeholders_or_pinned(dir)
}

/// Every tool the runbook's proofs call through the airs CLI (§7b, §9, §11, §12).
const CLI_PROOF_TOOLS: &[&str] = &[
    "service_info",
    "history_status",
    "history_coverage",
    "history_query",
    "collection_start",
    "receipts_ingest",
    "jobs_list",
    "jobs_get",
    "reports_create",
    "reports_list",
    "reports_get",
    "artifacts_read",
    "artifacts_download",
];

/// The Claude Code proof (§8) calls these.
const CLAUDE_PROOF_TOOLS: &[&str] = &["service_info", "history_status"];

/// The grants make the documented proofs executable (M8 review 2 finding 9): the airs
/// CLI grant holds every proof tool and the Claude Code grant its two. Every granted
/// tool is an implemented catalogue tool; the strict parser also enforces tiers.
fn check_proof_grants(cfg: &airs_mcp::config::Config) -> Result<(), String> {
    for (label, needed) in [
        ("airs-cli", CLI_PROOF_TOOLS),
        ("claude-code", CLAUDE_PROOF_TOOLS),
    ] {
        let g = cfg
            .grants
            .iter()
            .find(|g| g.label.as_deref() == Some(label))
            .ok_or(format!("a {label} grant"))?;
        for t in needed {
            ensure(
                g.tools.iter().any(|x| x == t),
                format!("{label} grant lacks proof tool {t}"),
            )?;
        }
    }
    for g in &cfg.grants {
        for t in &g.tools {
            ensure(
                airs_mcp::catalogue::IMPLEMENTED.contains(&t.as_str()),
                format!("granted tool {t} is not an implemented catalogue tool"),
            )?;
        }
    }
    Ok(())
}

/// A complete, deployable set: every check, and a valid pinned image digest.
fn validate_deployable(dir: &Path) -> Result<String, String> {
    validate(dir)?;
    deployable_digest_in(dir)
}

/// A temporary copy of the committed set, for one mutation.
struct Copy(PathBuf);

impl Copy {
    fn new() -> Self {
        let d = std::env::temp_dir().join(format!("airs-mcp-deploy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&d).unwrap();
        for e in std::fs::read_dir(committed()).unwrap() {
            let e = e.unwrap();
            std::fs::copy(e.path(), d.join(e.file_name())).unwrap();
        }
        Self(d)
    }

    /// Rewrite one JSON manifest.
    fn edit(&self, name: &str, f: impl FnOnce(&mut Value)) -> &Self {
        let mut v = load_in(&self.0, name);
        f(&mut v);
        std::fs::write(self.0.join(name), serde_json::to_vec_pretty(&v).unwrap()).unwrap();
        self
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Pin every image in a kustomization with a valid (synthetic) digest.
fn pin_all(k: &mut Value) {
    for image in k["images"].as_array_mut().unwrap() {
        image["digest"] = json!(format!("sha256:{}", "ab".repeat(32)));
    }
}

/// One item of the backup manifest list, by kind and name.
fn backup_item<'a>(list: &'a mut Value, kind: &str, name: &str) -> &'a mut Value {
    list["items"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|i| i["kind"] == kind && i["metadata"]["name"] == name)
        .unwrap()
}

fn policy<'a>(np: &'a mut Value, name: &str) -> &'a mut Value {
    np["items"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|i| i["metadata"]["name"] == name)
        .unwrap()
}

#[test]
fn the_committed_set_passes_every_check() {
    validate(&dir()).unwrap();
}

/// The owner's filled set, before deployment (runbook step 1 and 2): every check and a
/// valid pinned digest. Runs only when `AIRS_MCP_REQUIRE_DEPLOYABLE` is set.
#[test]
fn the_set_is_deployable_when_required() {
    if std::env::var_os("AIRS_MCP_REQUIRE_DEPLOYABLE").is_none() {
        eprintln!(
            "DEPLOYABLE SKIPPED: set AIRS_MCP_REQUIRE_DEPLOYABLE=1 (and AIRS_MCP_DEPLOY_DIR)"
        );
        return;
    }
    let digest = validate_deployable(&dir()).unwrap();
    eprintln!("DEPLOYABLE: {digest}");
}

/// A valid pin is accepted (M8 review 1 finding 2: the committed test used to fail on
/// a completed pin); the placeholder, a tag and a malformed digest are refused.
#[test]
fn the_deployment_validator_requires_and_accepts_a_valid_pin() {
    let placeholder = Copy::new();
    assert!(
        validate(&placeholder.0).is_ok(),
        "the placeholder set is structurally valid"
    );
    assert!(
        validate_deployable(&placeholder.0)
            .unwrap_err()
            .contains("placeholder")
    );
    let pinned = Copy::new();
    pinned.edit("kustomization.yaml", pin_all);
    assert!(validate_deployable(&pinned.0).is_ok());
    // The service pinned but the backup orchestrator's kubectl still a placeholder.
    let partial = Copy::new();
    partial.edit("kustomization.yaml", |k| {
        k["images"][0]["digest"] = json!(format!("sha256:{}", "ab".repeat(32)));
    });
    assert!(
        validate_deployable(&partial.0)
            .unwrap_err()
            .contains("placeholder"),
        "every image must be pinned"
    );
    for bad in [
        "latest",
        "sha256:XYZ",
        "sha256:ABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABAB",
    ] {
        let c = Copy::new();
        c.edit("kustomization.yaml", |k| {
            k["images"][0]["digest"] = json!(bad)
        });
        assert!(validate_deployable(&c.0).is_err(), "{bad}");
        assert!(
            validate(&c.0).is_err(),
            "a malformed committed digest: {bad}"
        );
    }
    // M8 review 3 finding 4: the mappings must exactly cover every image the
    // containers run. Each case is otherwise fully pinned, so only the mapping can fail it.
    type Mutation = Box<dyn Fn(&Copy)>;
    let mappings: Vec<(&str, Mutation)> = vec![
        (
            "the kubectl mapping removed",
            Box::new(|c: &Copy| {
                c.edit("kustomization.yaml", |k| {
                    k["images"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|i| i["name"] != "kubectl")
                });
            }),
        ),
        (
            "the kubectl mapping renamed",
            Box::new(|c: &Copy| {
                c.edit("kustomization.yaml", |k| {
                    for i in k["images"].as_array_mut().unwrap() {
                        if i["name"] == "kubectl" {
                            i["name"] = json!("kubectl-cli");
                        }
                    }
                });
            }),
        ),
        (
            "an extra, unused mapping",
            Box::new(|c: &Copy| {
                c.edit("kustomization.yaml", |k| {
                    k["images"].as_array_mut().unwrap().push(json!({"name": "unused",
                        "newName": "registry.example/unused", "digest": format!("sha256:{}", "cd".repeat(32))}))
                });
            }),
        ),
        (
            "an init container image unmapped and unpinned",
            Box::new(|c: &Copy| {
                c.edit("deployment.json", |d| {
                    d["spec"]["template"]["spec"]["initContainers"][0]["image"] = json!("busybox")
                });
            }),
        ),
        (
            "an init container image by tag only",
            Box::new(|c: &Copy| {
                c.edit("deployment.json", |d| {
                    d["spec"]["template"]["spec"]["initContainers"][0]["image"] =
                        json!("docker.io/library/busybox:1.37.0")
                });
            }),
        ),
        (
            // M8 review 4 minor: a literal reference with the all-zero placeholder.
            "an init container image pinned to the placeholder digest",
            Box::new(|c: &Copy| {
                c.edit("deployment.json", |d| {
                    d["spec"]["template"]["spec"]["initContainers"][0]["image"] = json!(format!(
                        "docker.io/library/busybox@sha256:{}",
                        "0".repeat(64)
                    ))
                });
            }),
        ),
    ];
    for (what, mutate) in mappings {
        let c = Copy::new();
        c.edit("kustomization.yaml", pin_all);
        mutate(&c);
        c.edit("kustomization.yaml", pin_all);
        assert!(
            validate_deployable(&c.0).is_err(),
            "{what} must not be deployable"
        );
    }
}

/// Each security regression, applied alone to a copy, is refused by its check (M8
/// review 1 finding 2: these all passed the earlier tests).
#[test]
fn every_security_regression_is_refused() {
    type Mutation = (&'static str, &'static str, fn(&mut Value));
    let cases: [Mutation; 32] = [
        (
            "backup-cronjob.json",
            "the backup Role widened to every verb",
            |l| {
                backup_item(l, "Role", "airs-mcp-backup")["rules"][1]["verbs"] = json!(["*"]);
            },
        ),
        ("backup-cronjob.json", "scale of any Deployment", |l| {
            backup_item(l, "Role", "airs-mcp-backup")["rules"][1]
                .as_object_mut()
                .unwrap()
                .remove("resourceNames");
        }),
        (
            "backup-cronjob.json",
            "the orchestrator never scales back up",
            |l| {
                let c = &mut backup_item(l, "CronJob", "airs-mcp-backup")["spec"]["jobTemplate"]["spec"]
                    ["template"]["spec"]["containers"][0];
                let s = c["command"][2]
                    .as_str()
                    .unwrap()
                    .replace("trap restore EXIT; ", "");
                c["command"][2] = json!(s);
            },
        ),
        ("backup-cronjob.json", "two backups at once", |l| {
            backup_item(l, "CronJob", "airs-mcp-backup")["spec"]["concurrencyPolicy"] =
                json!("Allow");
        }),
        (
            "backup-cronjob.json",
            "the set template runs on its own",
            |l| {
                backup_item(l, "CronJob", "airs-mcp-backup-set")["spec"]["suspend"] = json!(false);
            },
        ),
        (
            "backup-cronjob.json",
            "the backup pod mounts a Secret",
            |l| {
                backup_item(l, "CronJob", "airs-mcp-backup-set")["spec"]["jobTemplate"]["spec"]["template"]["spec"]["volumes"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name": "creds", "secret": {"secretName": "airs-mcp-airs-credentials"}}));
            },
        ),
        ("backup-cronjob.json", "the backup pod runs as root", |l| {
            backup_item(l, "CronJob", "airs-mcp-backup-set")["spec"]["jobTemplate"]["spec"]["template"]
                ["spec"]["initContainers"][0]["securityContext"]["runAsUser"] = json!(0);
        }),
        (
            "backup-cronjob.json",
            "the orchestrator may reach anything",
            |l| {
                backup_item(l, "NetworkPolicy", "airs-mcp-backup-to-apiserver")["spec"]["egress"] =
                    json!([{}]);
            },
        ),
        (
            "networkpolicy.json",
            "egress on the gateway ingress policy (review 2)",
            |np| {
                let p = policy(np, "airs-mcp-from-gateway");
                p["spec"]["policyTypes"] = json!(["Ingress", "Egress"]);
                p["spec"]["egress"] = json!([{}]);
            },
        ),
        (
            "networkpolicy.json",
            "egress on the health ingress policy",
            |np| {
                policy(np, "airs-mcp-metrics-from-monitoring")["spec"]["egress"] = json!([{}]);
            },
        ),
        ("networkpolicy.json", "ingress on the egress policy", |np| {
            let p = policy(np, "airs-mcp-egress");
            p["spec"]["policyTypes"] = json!(["Egress", "Ingress"]);
            p["spec"]["ingress"] = json!([{}]);
        }),
        (
            "deployment.json",
            "the secret copy changes directory ownership before its files (review 2)",
            |d| {
                let inits = d["spec"]["template"]["spec"]["initContainers"]
                    .as_array_mut()
                    .unwrap();
                let init = inits.iter_mut().find(|c| c["name"] == "secrets").unwrap();
                init["command"][2] = json!(OLD_SECRET_COPY);
            },
        ),
        (
            "networkpolicy.json",
            "public metrics: any namespace",
            |np| {
                policy(np, "airs-mcp-metrics-from-monitoring")["spec"]["ingress"][0]["from"] =
                    json!([{"namespaceSelector": {}}]);
            },
        ),
        ("networkpolicy.json", "public metrics: no from", |np| {
            policy(np, "airs-mcp-metrics-from-monitoring")["spec"]["ingress"][0]
                .as_object_mut()
                .unwrap()
                .remove("from");
        }),
        (
            "networkpolicy.json",
            "public TLS ingress: an ipBlock",
            |np| {
                policy(np, "airs-mcp-from-gateway")["spec"]["ingress"][0]["from"] =
                    json!([{"ipBlock": {"cidr": "0.0.0.0/0"}}]);
            },
        ),
        (
            "networkpolicy.json",
            "TLS ingress from any pod of the gateway namespace",
            |np| {
                policy(np, "airs-mcp-from-gateway")["spec"]["ingress"][0]["from"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("podSelector");
            },
        ),
        ("networkpolicy.json", "an extra permissive policy", |np| {
            np["items"].as_array_mut().unwrap().push(json!({
                "apiVersion": "networking.k8s.io/v1", "kind": "NetworkPolicy",
                "metadata": {"name": "allow-all"},
                "spec": {"podSelector": {}, "ingress": [{}], "policyTypes": ["Ingress"]}
            }));
        }),
        (
            "networkpolicy.json",
            "the health listener on the gateway rule",
            |np| {
                policy(np, "airs-mcp-from-gateway")["spec"]["ingress"][0]["ports"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"protocol": "TCP", "port": 8081}));
            },
        ),
        (
            "networkpolicy.json",
            "unrestricted egress: no ports",
            |np| {
                policy(np, "airs-mcp-egress")["spec"]["egress"][2]
                    .as_object_mut()
                    .unwrap()
                    .remove("ports");
            },
        ),
        (
            "networkpolicy.json",
            "unrestricted egress: no peers",
            |np| {
                policy(np, "airs-mcp-egress")["spec"]["egress"][2]
                    .as_object_mut()
                    .unwrap()
                    .remove("to");
            },
        ),
        ("networkpolicy.json", "egress to private ranges", |np| {
            policy(np, "airs-mcp-egress")["spec"]["egress"][2]["to"][0]["ipBlock"]
                .as_object_mut()
                .unwrap()
                .remove("except");
        }),
        ("networkpolicy.json", "an IPv6 catch-all egress", |np| {
            policy(np, "airs-mcp-egress")["spec"]["egress"]
                .as_array_mut()
                .unwrap()
                .push(json!({"to": [{"ipBlock": {"cidr": "::/0"}}], "ports": [{"protocol": "TCP", "port": 443}]}));
        }),
        ("networkpolicy.json", "the default deny weakened", |np| {
            policy(np, "default-deny")["spec"]["policyTypes"] = json!(["Ingress"]);
        }),
        ("deployment.json", "a privileged service container", |d| {
            d["spec"]["template"]["spec"]["containers"][0]["securityContext"]["privileged"] =
                json!(true);
        }),
        ("deployment.json", "privilege escalation", |d| {
            d["spec"]["template"]["spec"]["containers"][0]["securityContext"]["allowPrivilegeEscalation"] =
                json!(true);
        }),
        ("deployment.json", "the service as root", |d| {
            d["spec"]["template"]["spec"]["containers"][0]["securityContext"]["runAsUser"] =
                json!(0);
        }),
        ("deployment.json", "host networking", |d| {
            d["spec"]["template"]["spec"]["hostNetwork"] = json!(true);
        }),
        ("deployment.json", "an fsGroup", |d| {
            d["spec"]["template"]["spec"]["securityContext"]["fsGroup"] = json!(65532);
        }),
        (
            "deployment.json",
            "the service reads the raw TLS Secret",
            |d| {
                d["spec"]["template"]["spec"]["containers"][0]["volumeMounts"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"name": "tls-source", "mountPath": "/raw", "readOnly": true}));
            },
        ),
        (
            "deployment.json",
            "secrets delivered to disk, not memory",
            |d| {
                let vols = d["spec"]["template"]["spec"]["volumes"]
                    .as_array_mut()
                    .unwrap();
                let v = vols.iter_mut().find(|v| v["name"] == "secrets").unwrap();
                v["emptyDir"] = json!({});
            },
        ),
        ("certificate.json", "a wrong certificate hostname", |c| {
            c["spec"]["dnsNames"] = json!(["other.example.test"]);
        }),
        (
            "service.json",
            "the health listener as a Service port",
            |s| {
                s["spec"]["ports"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name": "health", "port": 8081, "targetPort": "health", "protocol": "TCP"}));
            },
        ),
    ];
    for (file, what, mutate) in cases {
        let c = Copy::new();
        c.edit(file, mutate);
        let err = validate(&c.0).err();
        assert!(err.is_some(), "not refused: {what}");
        eprintln!("refused {what}: {}", err.unwrap());
        // A pinned copy of the same mutation is never deployable either.
        let k = c.0.join("kustomization.yaml");
        let mut v = load_in(&c.0, "kustomization.yaml");
        pin_all(&mut v);
        std::fs::write(&k, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
        assert!(
            validate_deployable(&c.0).is_err(),
            "deployable despite: {what}"
        );
    }
}

/// M8 review 2 finding 9: the grants make the documented proofs executable. Removing a
/// proof tool from the airs CLI grant, or giving a grant a tool that is not in the
/// catalogue, is refused.
#[test]
fn the_grants_cover_every_documented_proof_tool() {
    let toml_path = |c: &Copy| c.0.join("airs-mcp.toml");
    let missing = Copy::new();
    let text = std::fs::read_to_string(toml_path(&missing)).unwrap();
    let edited = text.replacen("\"reports_create\", ", "", 1);
    assert_ne!(text, edited, "the template grants reports_create");
    std::fs::write(toml_path(&missing), edited).unwrap();
    let err = validate(&missing.0).unwrap_err();
    assert!(err.contains("lacks proof tool reports_create"), "{err}");
    let claude = Copy::new();
    let text = std::fs::read_to_string(toml_path(&claude)).unwrap();
    let edited = text.replacen(
        "tools = [\"service_info\", \"history_status\", \"history_coverage\"]",
        "tools = [\"service_info\", \"history_coverage\"]",
        1,
    );
    assert_ne!(text, edited, "the Claude Code grant is the reader grant");
    std::fs::write(toml_path(&claude), edited).unwrap();
    let err = validate(&claude.0).unwrap_err();
    assert!(
        err.contains("claude-code grant lacks proof tool history_status"),
        "{err}"
    );
}

/// The review-2 secret copy: directories reach 0500 and 65532 ownership in the same
/// command as their files, so root with only CAP_CHOWN cannot reach the files.
const OLD_SECRET_COPY: &str = "set -eu; umask 0377; mkdir -m 0700 /secrets/tls /secrets/airs; for f in tls.crt tls.key; do cat /source/tls/$f > /secrets/tls/$f; done; for f in mgmt-client-id mgmt-client-secret scanner-monitor; do cat /source/airs/$f > /secrets/airs/$f; done; chmod 0500 /secrets/tls /secrets/airs; chown 65532:65532 /secrets/tls /secrets/tls/tls.crt /secrets/tls/tls.key /secrets/airs /secrets/airs/mgmt-client-id /secrets/airs/mgmt-client-secret /secrets/airs/scanner-monitor; stat -c '%u:%g %a %n' /secrets/tls /secrets/airs";

/// M8 review 2 finding 1, modelled: the copier (root with only CAP_CHOWN) loses search
/// permission on a directory the moment the directory's owner becomes 65532. The test
/// runs the committed script with each directory `chown` replaced by `chmod 0000`, so a
/// later command that still needs to reach a file in that directory fails, as in the
/// pod. The committed order passes; the review-2 order fails.
#[test]
fn the_secret_copy_never_needs_a_directory_after_giving_it_away() {
    let p = pod_in(&committed());
    let init = init_named(&p, "secrets").unwrap();
    let committed_script = init["command"][2].as_str().unwrap().to_owned();
    assert!(
        run_with_access_loss(&committed_script),
        "the committed order completes"
    );
    assert!(
        !run_with_access_loss(OLD_SECRET_COPY),
        "the review-2 order must fail once the directory is given away"
    );
}

fn run_with_access_loss(script: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let base = std::env::temp_dir().join(format!("airs-mcp-loss-{}", uuid::Uuid::new_v4()));
    let (src, dst) = (base.join("source"), base.join("secrets"));
    for (sub, files) in [
        ("tls", &["tls.crt", "tls.key"][..]),
        (
            "airs",
            &["mgmt-client-id", "mgmt-client-secret", "scanner-monitor"][..],
        ),
    ] {
        std::fs::create_dir_all(src.join(sub)).unwrap();
        for f in files {
            std::fs::write(src.join(sub).join(f), format!("synthetic {f}")).unwrap();
        }
    }
    std::fs::create_dir_all(&dst).unwrap();
    let uid = rustix::process::getuid().as_raw();
    let gid = rustix::process::getgid().as_raw();
    let dirs = ["/secrets/tls", "/secrets/airs"];
    // Split each `chown 65532:65532 …` into one command per path: a directory loses all
    // access for the copier (chmod 0000); a file is chowned to the current user.
    let rewritten: Vec<String> = script
        .split(';')
        .map(str::trim)
        .map(|c| {
            let Some(paths) = c.strip_prefix("chown 65532:65532 ") else {
                return c.to_owned();
            };
            paths
                .split_whitespace()
                .map(|path| {
                    if dirs.contains(&path) {
                        format!("chmod 0000 {path}")
                    } else {
                        format!("chown {uid}:{gid} {path}")
                    }
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .collect();
    let script = rewritten
        .join("; ")
        .replace("/source/", &format!("{}/", src.display()))
        .replace("/secrets/", &format!("{}/", dst.display()));
    let ok = std::process::Command::new("sh")
        .arg("-c")
        .arg(&script)
        .output()
        .unwrap()
        .status
        .success();
    for sub in ["tls", "airs"] {
        let d = dst.join(sub);
        if d.exists() {
            let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    ok
}

/// The `secrets` init script, run with its paths moved into a temporary directory (and
/// the service user replaced by the current user, who cannot chown to 65532), copies
/// every Secret file byte for byte into owner-only files (0400) under owner-only
/// directories (0500), readable by their owner: the service user's access, exercised.
#[test]
fn the_secret_copy_yields_owner_only_files_readable_by_their_owner() {
    use std::os::unix::fs::PermissionsExt;
    let p = pod_in(&committed());
    let init = init_named(&p, "secrets").unwrap();
    let base = std::env::temp_dir().join(format!("airs-mcp-secrets-{}", uuid::Uuid::new_v4()));
    let (src, dst) = (base.join("source"), base.join("secrets"));
    for (sub, files) in [
        ("tls", &["tls.crt", "tls.key"][..]),
        (
            "airs",
            &["mgmt-client-id", "mgmt-client-secret", "scanner-monitor"][..],
        ),
    ] {
        std::fs::create_dir_all(src.join(sub)).unwrap();
        for f in files {
            std::fs::write(src.join(sub).join(f), format!("synthetic {f}")).unwrap();
        }
    }
    std::fs::create_dir_all(&dst).unwrap();
    let uid = rustix::process::getuid().as_raw();
    let gid = rustix::process::getgid().as_raw();
    let script = init["command"][2]
        .as_str()
        .unwrap()
        .replace("/source/", &format!("{}/", src.display()))
        .replace("/secrets/", &format!("{}/", dst.display()))
        .replace("chown 65532:65532", &format!("chown {uid}:{gid}"));
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(&script)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for (sub, files) in [
        ("tls", &["tls.crt", "tls.key"][..]),
        (
            "airs",
            &["mgmt-client-id", "mgmt-client-secret", "scanner-monitor"][..],
        ),
    ] {
        let d = dst.join(sub);
        assert_eq!(
            std::fs::metadata(&d).unwrap().permissions().mode() & 0o777,
            0o500
        );
        for f in files {
            let path = d.join(f);
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o400,
                "{f}"
            );
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                format!("synthetic {f}")
            );
        }
    }
    let mut perms = std::fs::metadata(dst.join("tls")).unwrap().permissions();
    perms.set_mode(0o700);
    std::fs::set_permissions(dst.join("tls"), perms.clone()).unwrap();
    std::fs::set_permissions(dst.join("airs"), perms).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn the_kustomization_lists_every_manifest() {
    let k = load_in(&dir(), "kustomization.yaml");
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
        "lib-secrets.sh",
        "derive-config.sh",
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
    // M8 review 1 finding 3: the helper is offline. It validates an owner-supplied,
    // sanitized inventory, and makes no network call and reads no token.
    let urls: Vec<&str> = code.matches("https://").collect();
    assert_eq!(urls.len(), 0, "no network call");
    assert!(!code.contains("read -r -s"), "no token is read");
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

/// M8 review 4 finding 1: the orchestrator script itself, run against stateful stub
/// `kubectl`, `sleep` and `date`. A completed Job keeps its pod, so the volume-release
/// step must accept a retained terminal pod. Cases:
/// (a) the Job completes and its pod stays, phase `Succeeded`: the service is restored,
///     exit 0, all three events;
/// (b) the Job fails (pod `Failed`): the service is restored, exit 1,
///     `service_restored` without `backup_set_verified` or completion;
/// (c) SIGTERM while polling the Job: the exit trap restores the service, exit 143,
///     no completion;
/// (d) the Job never becomes terminal and its pod ends `Failed` (the Job's own deadline
///     killed it): the poll gives up, the service is restored, exit 1, no completion;
/// (e) the Job never becomes terminal and its pod never ends: no restoration is claimed,
///     exit 1, though the scale-up is still requested so the service returns once the
///     volume frees.
#[test]
fn the_backup_orchestrator_restores_the_service_and_reports_honestly() {
    use std::os::unix::fs::PermissionsExt;
    let list = load_in(&committed(), "backup-cronjob.json");
    let script = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "CronJob" && i["metadata"]["name"] == "airs-mcp-backup")
        .unwrap()["spec"]["jobTemplate"]["spec"]["template"]["spec"]["containers"][0]["command"][2]
        .as_str()
        .unwrap()
        .to_owned();
    // `job`: the Job's condition lines; `pod`: the set pod's phase lines; `available`: the
    // Deployment's availableReplicas; `term`: send SIGTERM to the orchestrator the first
    // time it sleeps after polling the Job.
    struct Case<'a> {
        job: &'a str,
        pod: &'a str,
        available: &'a str,
        term: bool,
    }
    let run = |c: &Case| -> (i32, String, Vec<String>) {
        let d = std::env::temp_dir().join(format!("airs-mcp-orch-{}", uuid::Uuid::new_v4()));
        let bin = d.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let log = d.join("log");
        let polled = d.join("polled");
        let sent = d.join("sent");
        let kubectl = format!(
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$*\" in\n  *'get job'*) touch '{polled}'; printf '%s' '{job}' ;;\n  *'get deployment'*) printf '%s' '{available}' ;;\n  *'get pod'*'job-name='*) printf '%s' '{pod}' ;;\n  *'get pod'*) printf '' ;;\nesac\nexit 0\n",
            log = log.display(),
            polled = polled.display(),
            job = c.job,
            available = c.available,
            pod = c.pod,
        );
        let sleep = if c.term {
            format!(
                "#!/bin/sh\nif [ -e '{polled}' ] && [ ! -e '{sent}' ]; then touch '{sent}'; kill -TERM $PPID; fi\nexit 0\n",
                polled = polled.display(),
                sent = sent.display()
            )
        } else {
            "#!/bin/sh\nexit 0\n".to_owned()
        };
        for (name, body) in [
            ("kubectl", kubectl.as_str()),
            ("sleep", sleep.as_str()),
            ("date", "#!/bin/sh\necho 20261107030000\n"),
        ] {
            let p = bin.join(name);
            std::fs::write(&p, body).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let out = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&script)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .output()
            .unwrap();
        let calls = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        let _ = std::fs::remove_dir_all(&d);
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            calls,
        )
    };
    let index = |calls: &[String], needle: &str| {
        calls
            .iter()
            .position(|c| c.contains(needle))
            .unwrap_or_else(|| panic!("no call {needle:?} in {calls:?}"))
    };
    let count =
        |calls: &[String], needle: &str| calls.iter().filter(|c| c.contains(needle)).count();
    // (a) Completed Job, retained pod.
    let (code, out, calls) = run(&Case {
        job: "Complete=True\n",
        pod: "Succeeded\n",
        available: "1",
        term: false,
    });
    assert_eq!(code, 0, "{out}");
    for event in [
        "backup_set_verified",
        "service_restored",
        "backup_set_daily_complete",
    ] {
        assert!(out.contains(event), "{event}: {out}");
    }
    let order = [
        index(&calls, "--replicas=0"),
        index(&calls, "app.kubernetes.io/name=airs-mcp"),
        index(&calls, "create job"),
        index(&calls, "get job"),
        index(&calls, "job-name="),
        index(&calls, "--replicas=1"),
        index(&calls, "get deployment"),
    ];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{calls:?}");
    assert!(
        calls.iter().all(|c| c.contains("--request-timeout=")),
        "every call is bounded: {calls:?}"
    );
    assert_eq!(
        count(&calls, "job-name="),
        1,
        "the retained pod releases at once"
    );
    // (b) Failed Job.
    let (code, out, calls) = run(&Case {
        job: "Failed=True\n",
        pod: "Failed\n",
        available: "1",
        term: false,
    });
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("service_restored") && !out.contains("backup_set_verified"));
    assert!(!out.contains("backup_set_daily_complete"));
    assert!(index(&calls, "job-name=") < index(&calls, "--replicas=1"));
    // (c) SIGTERM while polling: the trap still restores.
    let (code, out, calls) = run(&Case {
        job: "",
        pod: "Succeeded\n",
        available: "1",
        term: true,
    });
    assert_eq!(code, 143, "{out}");
    assert!(out.contains("service_restored"), "{out}");
    assert!(!out.contains("backup_set_verified") && !out.contains("backup_set_daily_complete"));
    assert!(
        index(&calls, "get job") < index(&calls, "--replicas=1"),
        "{calls:?}"
    );
    // (d) The Job never reports terminal, but its pod ended: the poll gives up and the
    // service is restored.
    let (code, out, calls) = run(&Case {
        job: "",
        pod: "Failed\n",
        available: "1",
        term: false,
    });
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("service_restored") && !out.contains("backup_set_verified"));
    assert!(!out.contains("backup_set_daily_complete"));
    assert_eq!(
        count(&calls, "get job"),
        1230 / 5,
        "the poll runs to its bound"
    );
    // (e) The pod never ends: no restoration is claimed, but scale-up is requested.
    let (code, out, calls) = run(&Case {
        job: "",
        pod: "Running\n",
        available: "1",
        term: false,
    });
    assert_eq!(code, 1, "{out}");
    assert!(!out.contains("service_restored") && !out.contains("backup_set_daily_complete"));
    assert_eq!(
        count(&calls, "job-name="),
        1260 / 5,
        "the release wait runs to its bound"
    );
    assert!(calls.iter().any(|c| c.contains("--replicas=1")));
    // A service that never becomes available: neither restoration nor completion.
    let (code, out, _) = run(&Case {
        job: "Complete=True\n",
        pod: "Succeeded\n",
        available: "",
        term: false,
    });
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("backup_set_verified"));
    assert!(!out.contains("service_restored") && !out.contains("backup_set_daily_complete"));
}
