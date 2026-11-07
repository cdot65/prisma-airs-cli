//! Planned (programmed) responses.

use std::time::Duration;

use serde_json::Value;

/// A response the fixture sends instead of (or as) its built-in behaviour.
///
/// Built with the constructors below and refined with the `with_*` methods:
///
/// ```
/// use airs_test_harness::Planned;
/// use serde_json::json;
/// use std::time::Duration;
///
/// let busy = Planned::json(503, &json!({"message": "busy"})).with_header("retry-after", "0");
/// let slow = Planned::json(200, &json!([])).with_delay(Duration::from_millis(200));
/// let hung = Planned::hang();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    /// Optional fixture ownership guards, checked even for programmed responses.
    pub required_headers: Vec<(String, String)>,
    /// HTTP status code.
    pub status: u16,
    /// Response headers (name, value).
    pub headers: Vec<(String, String)>,
    /// Response body bytes.
    pub body: Vec<u8>,
    /// Wait this long before answering.
    pub delay: Option<Duration>,
    /// Never answer; the request stays pending until the client disconnects (counted by
    /// [`crate::FixtureServer::disconnects`]).
    pub hang: bool,
    /// Replace [`Planned::BEARER`] in header values and the body with the bearer token
    /// the request carried (a server that echoes credentials).
    pub echo_bearer: bool,
    /// Wait until the test releases the request ([`crate::FixtureServer::release_held`])
    /// before answering; the request is recorded on arrival, so a test can act on it.
    pub hold: bool,
    /// Answer with the route's built-in behaviour (after any hold or delay): a planned
    /// response that only adds timing (DESIGN §10 M5 "held request").
    pub builtin: bool,
    /// Close the connection without answering, after the request was read.
    pub disconnect: bool,
}

impl Planned {
    pub fn with_required_header(mut self, name: &str, value: &str) -> Self {
        self.required_headers
            .push((name.to_owned(), value.to_owned()));
        self
    }
    /// A JSON body with `content-type: application/json`.
    pub fn json(status: u16, body: &Value) -> Self {
        Self::raw(status, "application/json", body.to_string())
    }

    /// `{}` with `content-type: application/json` (the fixture's error body).
    pub fn empty_json(status: u16) -> Self {
        Self::json(status, &Value::Object(serde_json::Map::new()))
    }

    /// Arbitrary body bytes with the given content type (e.g. non-JSON or truncated JSON).
    pub fn raw(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            required_headers: Vec::new(),
            status,
            headers: vec![("content-type".into(), content_type.into())],
            body: body.into(),
            delay: None,
            hang: false,
            echo_bearer: false,
            hold: false,
            builtin: false,
            disconnect: false,
        }
    }

    /// A status with no body and no content type.
    pub fn status(status: u16) -> Self {
        Self {
            required_headers: Vec::new(),
            status,
            headers: Vec::new(),
            body: Vec::new(),
            delay: None,
            hang: false,
            echo_bearer: false,
            hold: false,
            builtin: false,
            disconnect: false,
        }
    }

    /// The route's built-in answer, held until the test releases it
    /// ([`crate::FixtureServer::release_held`]). A held async submission creates its
    /// receipt on arrival (the service accepted it), whatever the client does next.
    pub fn held() -> Self {
        Self {
            hold: true,
            builtin: true,
            ..Self::status(200)
        }
    }

    /// The route's built-in answer (useful with [`Planned::with_delay`]).
    pub fn builtin() -> Self {
        Self {
            builtin: true,
            ..Self::status(200)
        }
    }

    /// The route's built-in answer is applied (a write takes effect), then the
    /// connection is closed without a response: an applied write whose outcome the
    /// client cannot know.
    pub fn applied_then_dropped() -> Self {
        Self {
            builtin: true,
            disconnect: true,
            ..Self::status(200)
        }
    }

    /// Read the request, then close the connection without a response.
    pub fn disconnect() -> Self {
        Self {
            disconnect: true,
            ..Self::status(200)
        }
    }

    /// Hold this response until the test releases it.
    pub fn held_until_released(mut self) -> Self {
        self.hold = true;
        self
    }

    /// The placeholder [`Planned::echoing_bearer`] replaces.
    pub const BEARER: &'static str = "{{bearer}}";

    /// A redirect (`301`/`302`/`307`/...) to `location`, with no body.
    ///
    /// # Panics
    ///
    /// Unless `location` is relative (`/…`) or a loopback URL: the TypeScript CLI
    /// follows redirects and re-sends its credentials (D30), so an off-box location
    /// would send fixture credentials off the machine.
    pub fn redirect(status: u16, location: &str) -> Self {
        Self::status(status).with_header("location", location)
    }

    /// A request that is never answered.
    pub fn hang() -> Self {
        Self {
            hang: true,
            ..Self::status(200)
        }
    }

    /// Adds a response header.
    ///
    /// # Panics
    ///
    /// For a `location` or `content-location` header that is neither relative nor a
    /// loopback URL (see [`Planned::redirect`]).
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        let name = name.to_ascii_lowercase();
        if matches!(name.as_str(), "location" | "content-location") {
            let relative = value.starts_with('/') && !value.starts_with("//");
            assert!(
                relative || crate::sandbox::is_loopback_url(value),
                "refusing to plan a non-loopback redirect: {name} {value}"
            );
        }
        self.headers.push((name, value.into()));
        self
    }

    /// Echo the request's bearer token: every [`Planned::BEARER`] in the header values
    /// and the body is replaced by it when the response is sent.
    pub fn echoing_bearer(mut self) -> Self {
        self.echo_bearer = true;
        self
    }

    /// Adds `retry-after: <seconds>`.
    pub fn with_retry_after(self, seconds: u64) -> Self {
        self.with_header("retry-after", &seconds.to_string())
    }

    /// Answers only after `delay`.
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}
