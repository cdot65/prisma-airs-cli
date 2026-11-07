//! Spike tools. Each call is authorized again here against the principal the HTTP
//! layer verified for *this* request (contract §3.5); tool annotations are hints.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::auth::Principal;
use crate::config::{AdminMode, Config, Permission};
use crate::storage::{Admitted, OwnerRecord};

/// Agent-facing paged text cap (contract §5.3): 16 KiB serialized JSON per result.
pub const AGENT_PAGE_LIMIT: usize = 16 * 1024;
/// Synthetic 32 MiB artifact used to measure chunked download call count/time.
pub const VIRTUAL_ARTIFACT_BYTES: u64 = 32 * 1024 * 1024;
const PREVIEW_TTL: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct Spike {
    pub config: Arc<Config>,
    pub storage: Option<(Admitted, OwnerRecord)>,
    pub state: Arc<Mutex<State>>,
    pub artifact_sha256: Arc<String>,
}

#[derive(Default)]
pub struct State {
    pub operator_noops: u64,
    pub previews: HashMap<String, Preview>,
}

pub struct Preview {
    nonce: String,
    subject: String,
    client_id: String,
    digest: String,
    created: Instant,
    consumed: bool,
}

struct ToolSpec {
    name: &'static str,
    permission: Permission,
    description: &'static str,
    read_only: bool,
    destructive: bool,
    schema: fn() -> Value,
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "additionalProperties": false, "properties": props, "required": required})
}

const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "service_info",
        permission: Permission::Reader,
        read_only: true,
        destructive: false,
        description: "Spike identity, negotiated protocol, verified claim shapes and storage ownership (no tenant data).",
        schema: || obj(json!({}), &[]),
    },
    ToolSpec {
        name: "echo",
        permission: Permission::Reader,
        read_only: true,
        destructive: false,
        description: "Echo bounded UTF-8 text back with its length and SHA-256 (response-bound measurement).",
        schema: || {
            obj(
                json!({"text": {"type": "string"}, "repeat": {"type": "integer", "minimum": 1, "maximum": 65536}}),
                &["text"],
            )
        },
    },
    ToolSpec {
        name: "text_page",
        permission: Permission::Reader,
        read_only: true,
        destructive: false,
        description: "Page through a synthetic Markdown document in <=16 KiB results using an opaque cursor.",
        schema: || obj(json!({"cursor": {"type": "string"}}), &[]),
    },
    ToolSpec {
        name: "artifact_chunk",
        permission: Permission::Reader,
        read_only: true,
        destructive: false,
        description: "CLI byte transfer: a base64 chunk of a synthetic 32 MiB artifact at a server-checked offset.",
        schema: || {
            obj(
                json!({"offset": {"type": "integer", "minimum": 0}, "length": {"type": "integer", "minimum": 1}}),
                &["offset", "length"],
            )
        },
    },
    ToolSpec {
        name: "operator_noop",
        permission: Permission::Operator,
        read_only: false,
        destructive: false,
        description: "Operator-tier no-op mutation (counts calls) to test explicit Operator grants and allowed-tool lists.",
        schema: || obj(json!({}), &[]),
    },
    ToolSpec {
        name: "admin_preview",
        permission: Permission::Administrator,
        read_only: true,
        destructive: false,
        description: "Prepare a bound administrative no-op preview with a server nonce. Grants nothing by itself.",
        schema: || {
            obj(
                json!({"parameter": {"type": "string", "maxLength": 128}}),
                &["parameter"],
            )
        },
    },
    ToolSpec {
        name: "admin_execute",
        permission: Permission::Administrator,
        read_only: false,
        destructive: true,
        description: "Execute a previewed administrative no-op. Requires the configured fresh, nonce-bound proof.",
        schema: || {
            obj(
                json!({"preview_id": {"type": "string"}, "parameter": {"type": "string", "maxLength": 128}}),
                &["preview_id", "parameter"],
            )
        },
    },
];

fn deny(code: &str, reason: &str) -> CallToolResponse {
    CallToolResult::structured_error(json!({"schema_version": 1, "code": code, "reason": reason}))
        .into()
}

/// Complete serialized size of a tool result as a client receives it. rmcp's
/// `structured` duplicates the value into text content, so the bound applies to
/// the whole `CallToolResult`, not just the inner value (contract §5).
pub fn result_bytes(r: &CallToolResult) -> usize {
    serde_json::to_vec(r).map(|v| v.len()).unwrap_or(usize::MAX)
}

/// CLI-facing ceiling for one complete tool result.
pub const CLI_RESULT_LIMIT: usize = 256 * 1024;

/// The tier a catalogue tool requires, or `None` for an unknown name (config
/// validation: a grant may list only known tools at or below its own tier).
pub fn required_tier(name: &str) -> Option<Permission> {
    TOOLS.iter().find(|t| t.name == name).map(|t| t.permission)
}

/// Tier ∩ grant: the token scope tier, the grant tier and the allowed-tool list all admit it.
fn granted(p: &Principal, t: &ToolSpec) -> bool {
    p.effective_tier() >= t.permission && p.grant.tools.iter().any(|n| n == t.name)
}

/// An administrative identity mutates only through `admin_execute`, which carries the
/// fresh operation-bound proof (contract §3, R1a).
fn admin_side_door(p: &Principal, t: &ToolSpec) -> bool {
    p.grant.permission == Permission::Administrator && !t.read_only && t.name != "admin_execute"
}

/// Whether the tool can run for this principal under this configuration: dispatch would
/// admit it (grant and tier, no administrative side door) and it is not permanently
/// disabled (`admin_execute` with administrative mode `disabled`). The single predicate
/// behind `tools/list` and `service_info.principal.tools`; dispatch keeps its own
/// refusals, so a hidden tool called by name still fails closed with its code.
fn callable(p: &Principal, t: &ToolSpec, mode: AdminMode) -> bool {
    granted(p, t)
        && !admin_side_door(p, t)
        && !(t.name == "admin_execute" && mode == AdminMode::Disabled)
}

fn ok(limit: usize, value: Value) -> CallToolResponse {
    let result = CallToolResult::structured(value);
    if result_bytes(&result) > limit {
        return deny("result_too_large", "result exceeds its response bound");
    }
    result.into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Deterministic synthetic artifact bytes (no customer data): byte i = SHA-256 stream.
pub fn artifact_bytes(offset: u64, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut block = offset / 32;
    let mut skip = (offset % 32) as usize;
    while out.len() < len {
        let digest = Sha256::digest(block.to_be_bytes());
        let take = (32 - skip).min(len - out.len());
        out.extend_from_slice(&digest[skip..skip + take]);
        skip = 0;
        block += 1;
    }
    out
}

pub fn artifact_sha256() -> String {
    let mut h = Sha256::new();
    let mut offset = 0;
    while offset < VIRTUAL_ARTIFACT_BYTES {
        let len = (1 << 20).min(VIRTUAL_ARTIFACT_BYTES - offset) as usize;
        h.update(artifact_bytes(offset, len));
        offset += len as u64;
    }
    hex(&h.finalize())
}

fn synthetic_document() -> String {
    let mut doc = String::from("# Synthetic review packet (M0 spike)\n\n");
    for i in 0..400 {
        doc.push_str(&format!(
            "## Section {i}\n\nSynthetic evidence line {i}: no tenant data. ünïcödé ✓\n\n"
        ));
    }
    doc
}

fn principal(ctx: &RequestContext<RoleServer>) -> Option<Principal> {
    ctx.extensions
        .get::<axum::http::request::Parts>()?
        .extensions
        .get::<Principal>()
        .cloned()
}

fn negotiated(ctx: &RequestContext<RoleServer>) -> Option<String> {
    let parts = ctx.extensions.get::<axum::http::request::Parts>()?;
    parts
        .headers
        .get("mcp-protocol-version")?
        .to_str()
        .ok()
        .map(str::to_owned)
}

fn arg_str<'a>(args: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    args.get(k).and_then(Value::as_str)
}

impl Spike {
    async fn dispatch(
        &self,
        name: &str,
        args: Map<String, Value>,
        p: &Principal,
        ctx: &RequestContext<RoleServer>,
    ) -> CallToolResponse {
        let Some(spec) = TOOLS.iter().find(|t| t.name == name) else {
            return deny("not_found", "unknown tool");
        };
        // Effective authority = token scope tier ∩ grant tier ∩ allowed-tool list.
        if !granted(p, spec) {
            return deny("forbidden", "tool not granted to this caller and agent");
        }
        // Lower-tier mutations reached through an administrative identity are refused
        // in every mode.
        if admin_side_door(p, spec) {
            return deny(
                "forbidden",
                "administrative identities mutate only through proof-bound admin_execute",
            );
        }
        let limit = match name {
            "service_info" | "text_page" | "admin_preview" => AGENT_PAGE_LIMIT,
            _ => CLI_RESULT_LIMIT,
        };
        let allowed: &[&str] = match name {
            "service_info" | "operator_noop" => &[],
            "echo" => &["text", "repeat"],
            "text_page" => &["cursor"],
            "artifact_chunk" => &["offset", "length"],
            "admin_preview" => &["parameter"],
            _ => &["preview_id", "parameter"],
        };
        if args.keys().any(|k| !allowed.contains(&k.as_str())) {
            return deny("invalid_request", "unknown argument");
        }
        let s = &self.config.server;
        match name {
            "service_info" => ok(
                limit,
                json!({
                    "schema_version": 1,
                    "server": "airs-mcp-spike",
                    "version": env!("CARGO_PKG_VERSION"),
                    "config_revision": self.config.revision,
                    "negotiated_protocol_header": negotiated(ctx),
                    "principal": {"grant_tier": format!("{:?}", p.grant.permission), "token_tier": format!("{:?}", p.token_tier),
                        "effective_tier": format!("{:?}", p.effective_tier()),
                        // The tools dispatch admits for this principal (M0.6 finding, review 9).
                        "tools": TOOLS.iter().filter(|t| callable(p, t, self.config.admin.mode)).map(|t| t.name).collect::<Vec<_>>()},
                    "claim_shapes": {"upstream": p.upstream_shape, "assertion": p.assertion_shape},
                    "admin_mode": format!("{:?}", self.config.admin.mode),
                    "storage": self.storage.as_ref().map(|(a, o)| json!({"admitted": a, "owner": o})),
                    "limits": {"agent_page_bytes": AGENT_PAGE_LIMIT, "max_chunk_bytes": s.max_chunk_bytes, "max_echo_bytes": s.max_echo_bytes},
                }),
            ),
            "echo" => {
                let Some(text) = arg_str(&args, "text") else {
                    return deny("invalid_request", "text required");
                };
                let repeat = args.get("repeat").and_then(Value::as_u64).unwrap_or(1);
                let total = (text.len() as u64).saturating_mul(repeat);
                if !(1..=65536).contains(&repeat) || total > s.max_echo_bytes as u64 {
                    return deny("invalid_request", "echo exceeds the configured bound");
                }
                let body = text.repeat(repeat as usize);
                ok(
                    limit,
                    json!({"bytes": body.len(), "sha256": hex(&Sha256::digest(body.as_bytes())), "text": body}),
                )
            }
            "text_page" => {
                let doc = synthetic_document();
                let start: usize = match arg_str(&args, "cursor") {
                    None => 0,
                    Some(c) => match c.strip_prefix("p1:").and_then(|n| n.parse().ok()) {
                        Some(n) if n <= doc.len() && doc.is_char_boundary(n) => n,
                        _ => return deny("invalid_request", "invalid cursor"),
                    },
                };
                // Size the segment so the COMPLETE result (value + duplicated text
                // content + escaping) stays within the agent cap.
                let mut take = (8 * 1024).min(doc.len() - start);
                loop {
                    let mut end = start + take;
                    while !doc.is_char_boundary(end) {
                        end -= 1;
                    }
                    let next = (end < doc.len()).then(|| format!("p1:{end}"));
                    let value = json!({"segment": &doc[start..end], "next_cursor": next, "total_bytes": doc.len()});
                    if result_bytes(&CallToolResult::structured(value.clone())) <= AGENT_PAGE_LIMIT
                    {
                        break ok(limit, value);
                    }
                    if take <= 512 {
                        break deny("result_too_large", "page exceeds the agent cap");
                    }
                    take -= 512;
                }
            }
            "artifact_chunk" => {
                let offset = args.get("offset").and_then(Value::as_u64);
                let length = args.get("length").and_then(Value::as_u64);
                let (Some(offset), Some(length)) = (offset, length) else {
                    return deny("invalid_request", "offset and length required");
                };
                if length == 0
                    || length > s.max_chunk_bytes as u64
                    || offset >= VIRTUAL_ARTIFACT_BYTES
                {
                    return deny("invalid_request", "chunk outside bounds");
                }
                let len = length.min(VIRTUAL_ARTIFACT_BYTES - offset) as usize;
                let bytes = artifact_bytes(offset, len);
                ok(
                    limit,
                    json!({
                        "artifact_id": "synthetic-32mib",
                        "total_bytes": VIRTUAL_ARTIFACT_BYTES,
                        "total_sha256": *self.artifact_sha256,
                        "offset": offset,
                        "length": len,
                        "chunk_sha256": hex(&Sha256::digest(&bytes)),
                        "data_base64": base64::engine::general_purpose::STANDARD.encode(&bytes),
                    }),
                )
            }
            "operator_noop" => {
                let mut st = self.state.lock().await;
                st.operator_noops += 1;
                ok(limit, json!({"operator_noops": st.operator_noops}))
            }
            "admin_preview" => {
                let Some(param) = arg_str(&args, "parameter") else {
                    return deny("invalid_request", "parameter required");
                };
                let id = uuid::Uuid::new_v4().to_string();
                let nonce = uuid::Uuid::new_v4().simple().to_string();
                let digest = hex(&Sha256::digest(format!("noop\0{param}").as_bytes()));
                let mut st = self.state.lock().await;
                st.previews.retain(|_, v| v.created.elapsed() < PREVIEW_TTL);
                st.previews.insert(
                    id.clone(),
                    Preview {
                        nonce: nonce.clone(),
                        subject: p.subject.clone(),
                        client_id: p.client_id.clone(),
                        digest: digest.clone(),
                        created: Instant::now(),
                        consumed: false,
                    },
                );
                ok(
                    limit,
                    json!({"preview_id": id, "action": "noop", "parameter_digest": digest,
                    "expires_in_seconds": PREVIEW_TTL.as_secs(), "proof_nonce": nonce,
                    "admin_mode": format!("{:?}", self.config.admin.mode)}),
                )
            }
            _ => self.admin_execute(&args, p).await,
        }
    }

    /// Fresh-proof gate (contract §3.6): every administrative mutation needs a
    /// gateway-signed assertion that carries this preview's server nonce and a
    /// recent authentication time. A recent token alone never qualifies.
    async fn admin_execute(&self, args: &Map<String, Value>, p: &Principal) -> CallToolResponse {
        let admin = &self.config.admin;
        if admin.mode != AdminMode::FreshProof {
            return deny(
                "forbidden",
                "administrative mutation is disabled: no qualified authority option",
            );
        }
        let (Some(id), Some(param)) = (arg_str(args, "preview_id"), arg_str(args, "parameter"))
        else {
            return deny("invalid_request", "preview_id and parameter required");
        };
        let digest = hex(&Sha256::digest(format!("noop\0{param}").as_bytes()));
        let mut st = self.state.lock().await;
        let Some(preview) = st.previews.get_mut(id) else {
            return deny("not_found", "preview not found or expired");
        };
        if preview.created.elapsed() >= PREVIEW_TTL || preview.consumed {
            return deny("conflict", "preview expired or already consumed");
        }
        if preview.digest != digest
            || preview.subject != p.subject
            || preview.client_id != p.client_id
        {
            return deny("conflict", "parameters or actor differ from the preview");
        }
        let claim = admin.proof_nonce_claim.as_deref().unwrap_or("nonce");
        let max_age = admin.proof_max_age_seconds.unwrap_or(0);
        let nonce_ok =
            p.assertion_claims.get(claim).and_then(Value::as_str) == Some(preview.nonce.as_str());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let fresh = p
            .assertion_claims
            .get("auth_time")
            .and_then(Value::as_u64)
            .is_some_and(|t| t <= now && now - t <= max_age);
        if !nonce_ok || !fresh {
            return deny("forbidden", "fresh operation-bound proof missing or stale");
        }
        // Consumed once at admission; a retry resolves to this same operation.
        preview.consumed = true;
        ok(
            AGENT_PAGE_LIMIT,
            json!({"executed": "noop", "preview_id": id}),
        )
    }
}

impl ServerHandler for Spike {
    fn get_info(&self) -> ServerConfig {
        let mut info = Implementation::default();
        info.name = "airs-mcp-spike".into();
        info.version = env!("CARGO_PKG_VERSION").into();
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(info)
            .with_instructions(
                "Disposable M0.6 feasibility backend. No AIRS credentials or tenant data.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // Only tools this principal may call are listed; listing is not authorization.
        let p = principal(&ctx);
        let tools = TOOLS
            .iter()
            .filter(|t| {
                p.as_ref()
                    .is_some_and(|p| callable(p, t, self.config.admin.mode))
            })
            .map(|t| {
                let schema = match (t.schema)() {
                    Value::Object(m) => m,
                    _ => Map::new(),
                };
                Tool::new(t.name, t.description, Arc::new(schema)).with_annotations(
                    ToolAnnotations::new()
                        .read_only(t.read_only)
                        .destructive(t.destructive),
                )
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Some(p) = principal(&ctx) else {
            return Ok(deny(
                "unauthenticated",
                "no verified principal for this request",
            ));
        };
        let args = request.arguments.clone().unwrap_or_default();
        Ok(self.dispatch(&request.name, args, &p, &ctx).await)
    }
}
