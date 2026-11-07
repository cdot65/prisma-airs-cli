//! The tenant file's `mcp` namespace (RUNTIME_MCP_CONTRACT §7.3): named connections to
//! the optional runtime MCP service. Strict keys; names use tenant-name syntax; at most
//! 16 connections; HTTPS only (loopback HTTP for tests); no secrets (a public client).

use std::collections::BTreeMap;

use crate::error::CliError;
use crate::store::toml_string;

/// At most this many connections per tenant (contract §7.3).
pub const MAX_CONNECTIONS: usize = 16;

/// One named connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpConnection {
    pub url: String,
    pub client_id: String,
    pub service_tenant: String,
    pub expected_tsg: String,
    pub discovery_origins: Vec<String>,
    pub role: Role,
    pub cache: Cache,
    pub revision: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Ordinary,
    Admin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cache {
    Private,
    None,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ordinary => "ordinary",
            Self::Admin => "admin",
        }
    }
}

impl Cache {
    pub fn name(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::None => "none",
        }
    }
}

const KEYS: [&str; 8] = [
    "url",
    "clientId",
    "serviceTenant",
    "expectedTsg",
    "discoveryOrigins",
    "role",
    "cache",
    "revision",
];

/// Tenant-name syntax for connection names: 1–63 of `[a-z0-9-]`, starting alphanumeric.
pub fn valid_name(name: &str) -> bool {
    (1..=63).contains(&name.len())
        && name.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

/// The scheme, host and port of an absolute URL whose parts are acceptable (HTTPS, or
/// loopback HTTP); `None` otherwise. Credentials, queries and fragments are refused.
fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let authority = rest.split('/').next()?;
    if authority.is_empty() || authority.contains('@') || rest.contains('?') || rest.contains('#') {
        return None;
    }
    let host = if authority.starts_with('[') {
        authority.split(']').next()?.to_owned() + "]"
    } else {
        authority.split(':').next()?.to_owned()
    };
    match scheme {
        "https" => {}
        "http" if loopback(&host) => {}
        _ => return None,
    }
    Some(format!("{scheme}://{}", authority.to_ascii_lowercase()))
}

/// The canonical origin of an acceptable URL (contract §7.3), or `None`.
pub fn url_origin(url: &str) -> Option<String> {
    origin(url)
}

fn uuid_like(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit() && !b.is_ascii_uppercase()
            }
        })
}

impl McpConnection {
    /// Every rule of contract §7.3, as a fixed message naming the field.
    pub fn validate(&self) -> Result<(), &'static str> {
        let Some(own) = origin(&self.url) else {
            return Err(
                "url must be an HTTPS URL without credentials, query or fragment (HTTP only on loopback)",
            );
        };
        if self.client_id.is_empty()
            || self.client_id.len() > 256
            || !self.client_id.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err("clientId must be 1-256 printable characters");
        }
        let tenant_ok = (1..=63).contains(&self.service_tenant.len())
            && self
                .service_tenant
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !tenant_ok {
            return Err("serviceTenant must be 1-63 of [a-z0-9-]");
        }
        if self.expected_tsg.is_empty()
            || self.expected_tsg.len() > 32
            || !self.expected_tsg.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("expectedTsg must be 1-32 ASCII digits");
        }
        if self.discovery_origins.is_empty() || self.discovery_origins.len() > 8 {
            return Err("discoveryOrigins must list 1-8 origins");
        }
        for o in &self.discovery_origins {
            if origin(o).as_deref() != Some(o.trim_end_matches('/')) || o.ends_with('/') {
                return Err("discoveryOrigins must be bare origins (scheme://host[:port])");
            }
        }
        if !self.discovery_origins.contains(&own) {
            return Err("the url origin must be one of discoveryOrigins");
        }
        if self.role == Role::Admin && self.cache != Cache::None {
            return Err("an admin connection keeps no token cache (cache = \"none\")");
        }
        if !uuid_like(&self.revision) {
            return Err("revision must be a lowercase UUID");
        }
        Ok(())
    }

    /// The connection's TOML body (inside `[mcp.connections.NAME]`).
    pub fn fields_toml(&self) -> String {
        let origins = self
            .discovery_origins
            .iter()
            .map(|o| toml_string(o))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "url = {}\nclientId = {}\nserviceTenant = {}\nexpectedTsg = {}\ndiscoveryOrigins = [{origins}]\nrole = {}\ncache = {}\nrevision = {}\n",
            toml_string(&self.url),
            toml_string(&self.client_id),
            toml_string(&self.service_tenant),
            toml_string(&self.expected_tsg),
            toml_string(self.role.name()),
            toml_string(self.cache.name()),
            toml_string(&self.revision),
        )
    }

    /// The public view (every field is public: the connection holds no secret).
    pub fn public(&self) -> serde_json::Value {
        serde_json::json!({
            "url": self.url,
            "clientId": self.client_id,
            "serviceTenant": self.service_tenant,
            "expectedTsg": self.expected_tsg,
            "discoveryOrigins": self.discovery_origins,
            "role": self.role.name(),
            "cache": self.cache.name(),
            "revision": self.revision,
        })
    }
}

fn text(table: &toml::Table, key: &str) -> Option<String> {
    match table.get(key)? {
        toml::Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// Parse a `[mcp]` table: `{connections = {NAME = {...}}}` and nothing else. `None` when
/// any rule fails.
pub fn parse_namespace(value: &toml::Value) -> Option<BTreeMap<String, McpConnection>> {
    let table = value.as_table()?;
    if table.keys().any(|k| k != "connections") {
        return None;
    }
    let mut out = BTreeMap::new();
    let Some(connections) = table.get("connections") else {
        return Some(out);
    };
    let connections = connections.as_table()?;
    if connections.len() > MAX_CONNECTIONS {
        return None;
    }
    for (name, body) in connections {
        if !valid_name(name) {
            return None;
        }
        let body = body.as_table()?;
        if body.keys().any(|k| !KEYS.contains(&k.as_str())) {
            return None;
        }
        let origins = body
            .get("discoveryOrigins")?
            .as_array()?
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()?;
        let connection = McpConnection {
            url: text(body, "url")?,
            client_id: text(body, "clientId")?,
            service_tenant: text(body, "serviceTenant")?,
            expected_tsg: text(body, "expectedTsg")?,
            discovery_origins: origins,
            role: match text(body, "role").as_deref() {
                None | Some("ordinary") => Role::Ordinary,
                Some("admin") => Role::Admin,
                _ => return None,
            },
            cache: match text(body, "cache").as_deref() {
                None | Some("private") => Cache::Private,
                Some("none") => Cache::None,
                _ => return None,
            },
            revision: text(body, "revision")?,
        };
        connection.validate().ok()?;
        out.insert(name.clone(), connection);
    }
    Some(out)
}

/// A connection by its explicit name, or the usage error naming the problem.
pub fn select<'a>(
    map: Option<&'a BTreeMap<String, McpConnection>>,
    name: &str,
) -> Result<&'a McpConnection, CliError> {
    if !valid_name(name) {
        return Err(CliError::usage(
            "--mcp must name a connection (1-63 of [a-z0-9-])",
        ));
    }
    map.and_then(|m| m.get(name)).ok_or_else(|| {
        CliError::operational(
            "No such MCP connection in the selected tenant; add it with 'airs mcp connections set'",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> McpConnection {
        McpConnection {
            url: "https://mcp.example.test/ws/airs/mcp".into(),
            client_id: "00000000-0000-4000-8000-000000000001".into(),
            service_tenant: "airs-tsg-1234567890".into(),
            expected_tsg: "1234567890".into(),
            discovery_origins: vec![
                "https://mcp.example.test".into(),
                "https://idp.example.test".into(),
            ],
            role: Role::Ordinary,
            cache: Cache::Private,
            revision: "00000000-0000-4000-8000-0000000000aa".into(),
        }
    }

    #[test]
    fn connections_round_trip_and_every_rule_is_enforced() {
        let c = good();
        c.validate().unwrap();
        let text = format!("[mcp.connections.goshawks]\n{}", c.fields_toml());
        let table: toml::Table = text.parse().unwrap();
        let parsed = parse_namespace(&table["mcp"]).unwrap();
        assert_eq!(parsed["goshawks"], c);
        let mut bad = Vec::new();
        for (field, edit) in [
            (
                "http",
                Box::new(|c: &mut McpConnection| c.url = "http://mcp.example.test/mcp".into())
                    as Box<dyn Fn(&mut McpConnection)>,
            ),
            (
                "creds",
                Box::new(|c: &mut McpConnection| c.url = "https://u:p@mcp.example.test/mcp".into()),
            ),
            (
                "query",
                Box::new(|c: &mut McpConnection| c.url = "https://mcp.example.test/mcp?x=1".into()),
            ),
            (
                "origin",
                Box::new(|c: &mut McpConnection| {
                    c.discovery_origins = vec!["https://other.example.test".into()]
                }),
            ),
            (
                "admin cache",
                Box::new(|c: &mut McpConnection| c.role = Role::Admin),
            ),
            (
                "tsg",
                Box::new(|c: &mut McpConnection| c.expected_tsg = "12a".into()),
            ),
            (
                "revision",
                Box::new(|c: &mut McpConnection| c.revision = "x".into()),
            ),
        ] {
            let mut c = good();
            edit(&mut c);
            if c.validate().is_ok() {
                bad.push(field);
            }
        }
        assert!(bad.is_empty(), "accepted: {bad:?}");
        let mut loopback = good();
        loopback.url = "http://127.0.0.1:8080/mcp".into();
        loopback.discovery_origins = vec!["http://127.0.0.1:8080".into()];
        loopback.validate().unwrap();
        for text in [
            "[mcp]\nother = 1\n",
            "[mcp.connections.Bad]\nurl = \"x\"\n",
            &format!("[mcp.connections.a]\n{}extra = 1\n", good().fields_toml()),
        ] {
            let table: toml::Table = text.parse().unwrap();
            assert!(parse_namespace(&table["mcp"]).is_none(), "{text}");
        }
        let many: String = (0..=MAX_CONNECTIONS)
            .map(|n| format!("[mcp.connections.c{n}]\n{}", good().fields_toml()))
            .collect();
        let table: toml::Table = many.parse().unwrap();
        assert!(parse_namespace(&table["mcp"]).is_none(), "at most 16");
    }
}
