//! The documents the CLI's and the harness's tests run through the rules (choices 166, 172, 173, 176, 177): the
//! pinned documents ([`PINNED_DOCUMENTS`], one per rule and sub-rule, with the exact printed
//! form) and the generated ones ([`planted`]: a credential planted under every name shape the
//! name rule claims and in every container shape, which must never appear in any output
//! format). The generated names come from two sources: [`credential_names`] derives them from
//! [`super::names::NAME_RULES`] itself, so a new row or a new word is tested in every shape at
//! once; [`REAL_WORLD_NAMES`] is a fixed list written without the rules (environment variables
//! and member names that hold credentials in practice), so a name the rules miss fails a test
//! instead of passing every one. [`REAL_WORLD_TEXTS`] does the same for the text rules
//! (choice 181): credentials inside a string under a neutral name, and prose they must leave.
//!
//! Test data: compiled into test builds only (the CLI's unit tests, and the harness for the
//! workspace's other tests; choices 182, 185).

use super::names::{Class, NAME_RULES, Pattern, classify};

/// The pinned documents, one case per rule and sub-rule (choices 166, 172): `(received,
/// printed)`, where `printed` is what the CLI prints for `received` under the shared rules
/// alone (no resource policy). The CLI's tests check that it prints `printed` and that
/// [`super::check::withheld`] accepts it and refuses `received` unless the two are the same;
/// the harness's tests run the same table through the checker on `serde_json::Value`. A new
/// sub-rule adds its case here.
pub const PINNED_DOCUMENTS: &[(&str, &str)] = &[
    // Rule 3a: a camelCase credential name, a credential word inside a name.
    (
        r#"{"clientSecret":"PIN-REVIEW-0001","name":"n"}"#,
        r#"{"clientSecret":"[withheld]","name":"n"}"#,
    ),
    (
        r#"{"a":[{"adminpassword":"PIN-REVIEW-0002"}]}"#,
        r#"{"a":[{"adminpassword":"[withheld]"}]}"#,
    ),
    // Rule 3a, choice 163: plural and camelCase-plural token names.
    (
        r#"{"configurations":{"access_tokens":["PIN-REVIEW-0011"],"refreshTokens":"PIN-REVIEW-0012","apiTokens":{"ci":"PIN-REVIEW-0013"},"tokens":"PIN-REVIEW-0014"}}"#,
        r#"{"configurations":{"access_tokens":"[withheld]","refreshTokens":"[withheld]","apiTokens":"[withheld]","tokens":"[withheld]"}}"#,
    ),
    // Rule 3b: a generic name.
    (r#"{"token":"short"}"#, r#"{"token":"[withheld]"}"#),
    (r#"{"key":"PIN-REVIEW-0010"}"#, r#"{"key":"[withheld]"}"#),
    // Rule 3d: a credential member.
    (
        r#"{"oauthMetadata":{"issuer":"PIN-REVIEW-0003"}}"#,
        r#"{"oauthMetadata":"[withheld]"}"#,
    ),
    // Choice 159: a credential header pair; a tag pair.
    (
        r#"{"tags":[{"key":"x-functions-key","value":"PIN-REVIEW-0004"}]}"#,
        r#"{"tags":[{"key":"x-functions-key","value":"[withheld]"}]}"#,
    ),
    (
        r#"{"tags":[{"key":"region","value":"us-east"}]}"#,
        r#"{"tags":[{"key":"region","value":"us-east"}]}"#,
    ),
    // Rule 3c: header maps, as an object and as a list of pairs.
    (
        r#"{"custom_headers":{"x-env":"prod","accept":"*/*"}}"#,
        r#"{"custom_headers":{"x-env":"[withheld]","accept":"*/*"}}"#,
    ),
    (
        r#"{"headers":[{"key":"x-api","value":"PIN-REVIEW-0005"}]}"#,
        r#"{"headers":[{"key":"x-api","value":"[withheld]"}]}"#,
    ),
    // Rule 3e: an instance value, a credential named inside a schema, a describing text
    // showing a credential, and a schema that shows none.
    (
        r#"{"properties":{"apiKey":{"type":"string","default":"PIN-REVIEW-0006"}}}"#,
        r#"{"properties":{"apiKey":{"type":"string","default":"[withheld]"}}}"#,
    ),
    (
        r#"{"properties":{"db":{"type":"object","awsSessionToken":"PIN-REVIEW-0007"}}}"#,
        r#"{"properties":{"db":{"type":"object","awsSessionToken":"[withheld]"}}}"#,
    ),
    (
        r#"{"properties":{"token":{"type":"string","description":"e.g. sk-0123456789abcdef0"}}}"#,
        r#"{"properties":{"token":{"type":"string","description":"[withheld]"}}}"#,
    ),
    (
        r#"{"properties":{"apiKey":{"type":"string","label":"API Key","description":"The x-portkey-api-key header"}}}"#,
        r#"{"properties":{"apiKey":{"type":"string","label":"API Key","description":"The x-portkey-api-key header"}}}"#,
    ),
    // Rule 3f: a JSON document held as text.
    (
        r#"{"config":"{\"bearerToken\":\"PIN-REVIEW-0008\"}"}"#,
        r#"{"config":"{\"bearerToken\":\"[withheld]\"}"}"#,
    ),
    // Rule 3f, choice 167: a JSON text serde_json cannot read (a lone surrogate escape).
    (
        r#"{"config":"{\"api_key\":\"PIN-REVIEW-0016\",\"n\":\"\\ud83d\"}"}"#,
        r#"{"config":"[withheld]"}"#,
    ),
    // Rule 4: a URL's query.
    (
        r#"{"custom_host":"https://h/p?code=PIN-REVIEW-0009"}"#,
        r#"{"custom_host":"https://h/p?[withheld]"}"#,
    ),
    // Rule 4, choice 168: a password holding `/`; choice 169: a URL inside prose.
    (
        r#"{"custom_host":"https://u:PIN/REVIEW-0017@h/p"}"#,
        r#"{"custom_host":"https://[withheld]@h/p"}"#,
    ),
    (
        r#"{"error":"connect ECONNREFUSED https://u:PIN-REVIEW-0018@h/v1"}"#,
        r#"{"error":"connect ECONNREFUSED https://[withheld]@h/v1"}"#,
    ),
    // The printed names: fingerprints, counts, the guardrail settings (choice 165).
    (
        r#"{"name":"n","total_tokens":3,"masked_api_key":"sk-...abcd"}"#,
        r#"{"name":"n","total_tokens":3,"masked_api_key":"sk-...abcd"}"#,
    ),
    (
        r#"{"secrets_categories":["aws","github"],"maxTokenAge":"1h","tokenizer":"cl100k"}"#,
        r#"{"secrets_categories":["aws","github"],"maxTokenAge":"1h","tokenizer":"cl100k"}"#,
    ),
    // Choice 165: a value without text under a credential name is printed; one keyed by a
    // credential is not.
    (
        r#"{"pricing_config":{"pay_as_you_go":{"request_token":{"price":0.5},"response_token":{"price":1.5}}},"usage":{"prompt_tokens":3,"completion_tokens_details":{"reasoning_tokens":1}}}"#,
        r#"{"pricing_config":{"pay_as_you_go":{"request_token":{"price":0.5},"response_token":{"price":1.5}}},"usage":{"prompt_tokens":3,"completion_tokens_details":{"reasoning_tokens":1}}}"#,
    ),
    (
        r#"{"api_tokens":{"sk-0123456789abcdef0":1}}"#,
        r#"{"api_tokens":"[withheld]"}"#,
    ),
    // Choice 164: a secret mapping's `secret_key` is a location; elsewhere a credential.
    (
        r#"{"secret_mappings":[{"target_field":"api_key","secret_reference_id":"sr-1","secret_key":"api_key"}],"masked_api_key":"sk-...abcd"}"#,
        r#"{"secret_mappings":[{"target_field":"api_key","secret_reference_id":"sr-1","secret_key":"api_key"}],"masked_api_key":"sk-...abcd"}"#,
    ),
    (
        r#"{"aws":{"secret_key":"PIN-REVIEW-0015","region":"us"}}"#,
        r#"{"aws":{"secret_key":"[withheld]","region":"us"}}"#,
    ),
    // Choice 170: a `{name, value}` pair (an environment list, a header list) is read as a
    // header with that name, as a `{key, value}` pair is.
    (
        r#"{"deployment_config":{"env":[{"name":"OPENAI_API_KEY","value":"PIN-REVIEW-0019"},{"name":"LOG_LEVEL","value":"info"}]}}"#,
        r#"{"deployment_config":{"env":[{"name":"OPENAI_API_KEY","value":"[withheld]"},{"name":"LOG_LEVEL","value":"info"}]}}"#,
    ),
    (
        r#"{"forward_headers":[{"name":"Authorization","value":"Bearer PIN-REVIEW-0020"}]}"#,
        r#"{"forward_headers":[{"name":"Authorization","value":"[withheld]"}]}"#,
    ),
    // Choice 170: `pwd`, `passphrase`, `auth`, the `_key` suffix, a space or a `.` as a
    // separator.
    (
        r#"{"pwd":"hunter2x!","auth":"Bearer PIN-REVIEW-0021","account_key":"AAAAPIN-REVIEW-0022==","API KEY":"PIN-REVIEW-0023","api.key":"PIN-REVIEW-0024","passphrase":"PIN-REVIEW-0025","auth_type":"oauth"}"#,
        r#"{"pwd":"[withheld]","auth":"[withheld]","account_key":"[withheld]","API KEY":"[withheld]","api.key":"[withheld]","passphrase":"[withheld]","auth_type":"oauth"}"#,
    ),
    // Choice 170: a private key under a neutral name.
    (
        r#"{"cert":"-----BEGIN PRIVATE KEY-----\nPIN-REVIEW-0026\n-----END PRIVATE KEY-----","name":"n"}"#,
        r#"{"cert":"[withheld]","name":"n"}"#,
    ),
    // Choices 171 and 187: a number and an empty string under a credential name, and a number
    // inside a credential object, are withheld (TypeScript's `redactDeep` masks them); a
    // boolean and a count are printed.
    (
        r#"{"password":12345678,"credentials":{"pin":12345678},"is_secret":true,"prompt_tokens":5,"secret":""}"#,
        r#"{"password":"[withheld]","credentials":"[withheld]","is_secret":true,"prompt_tokens":5,"secret":"[withheld]"}"#,
    ),
    // Choice 187: a fingerprint prints unless it looks like a credential.
    (
        r#"{"api_key_hint":"sk-live-1234567890abcdefghij","token_prefix":"ghp_","api_key_last4":"wxyz","secret_last_four":1234}"#,
        r#"{"api_key_hint":"[withheld]","token_prefix":"ghp_","api_key_last4":"wxyz","secret_last_four":1234}"#,
    ),
    // Choice 187 (the live run of `workspaces get`, and the review's cascade): what describes a
    // credential is printed, a credential name's word-shaped value is withheld in place only.
    (
        r#"{"settings":{"user_api_key_count":3,"user_api_key_max_ttl":7776000,"user_api_key_ttl":"30d","user_api_key_rotation_period":""},"token_type":"Bearer","access_token":"Pl4ntR3v13wT0k3nXyZ9","note":"Send a Bearer header","secret_id":"PIN-REVIEW-0040x"}"#,
        r#"{"settings":{"user_api_key_count":3,"user_api_key_max_ttl":7776000,"user_api_key_ttl":"30d","user_api_key_rotation_period":""},"token_type":"Bearer","access_token":"[withheld]","note":"Send a Bearer header","secret_id":"[withheld]"}"#,
    ),
    // Choice 176: `pass` and `pat` as words (not inside one), the `_keys` suffix and `keys`,
    // `token` anywhere in a word but `tokenizer`, a connection string.
    (
        r#"{"db_pass":"hunter2x!","smtpPass":"PIN-REVIEW-0027","env":[{"name":"DB_PASS","value":"hunter2x!"}],"GITHUB_PAT":"PIN-REVIEW-0028","passthrough":"on","compass":"north","bypass":"cache"}"#,
        r#"{"db_pass":"[withheld]","smtpPass":"[withheld]","env":[{"name":"DB_PASS","value":"[withheld]"}],"GITHUB_PAT":"[withheld]","passthrough":"on","compass":"north","bypass":"cache"}"#,
    ),
    (
        r#"{"access_keys":["PIN-REVIEW-0029"],"keys":["sk-PIN-REVIEW-0030"],"virtual_keys":["openai-prod"],"api_key_ids":["k1"]}"#,
        r#"{"access_keys":"[withheld]","keys":"[withheld]","virtual_keys":["openai-prod"],"api_key_ids":["k1"]}"#,
    ),
    (
        r#"{"tokenvalue":"abcdefghij0123456789xyz","accesstokenhash":"PIN-REVIEW-0031","tokenizer":"cl100k","connectionString":"AccountName=a;AccountKey=PIN-REVIEW-0032=="}"#,
        r#"{"tokenvalue":"[withheld]","accesstokenhash":"[withheld]","tokenizer":"cl100k","connectionString":"[withheld]"}"#,
    ),
    // Choice 177: a credential after an authorization scheme under any name, in place (a
    // header value, a command line, a Basic credential); prose keeps its words. Choice 181
    // made `auth_header` a credential name, so it is withheld whole.
    (
        r#"{"auth_header":"Bearer sk-proj-PINREVIEW0033","authentication":"Basic dXNlcjpwYXNz","note":"curl -H 'Authorization: Bearer PINREVIEW0034abcdef' https://h/v1","about":"a basic introduction to Token auth"}"#,
        r#"{"auth_header":"[withheld]","authentication":"Basic [withheld]","note":"curl -H 'Authorization: Bearer [withheld]' https://h/v1","about":"a basic introduction to Token auth"}"#,
    ),
    // Choice 181: a credential after a header name joined with no space, a connection string
    // under a neutral name, a credential name the earlier rows missed; a model id after the
    // prose word `token` is withheld in that text only, so the `ai_model` column stays.
    (
        r#"[{"error":"token gpt-4o-mini-2024-07-18 quota exceeded"},{"ai_model":"gpt-4o-mini-2024-07-18"},{"cmd":"curl -H Authorization:Bearer sk-proj-PINREVIEW0039ab"},{"conn":"Server=x;User Id=sa;Password=hunter2x!;"},{"jwt":"PIN-REVIEW-0040"}]"#,
        r#"[{"error":"token [withheld] quota exceeded"},{"ai_model":"gpt-4o-mini-2024-07-18"},{"cmd":"curl -H Authorization:Bearer [withheld]"},{"conn":"Server=x;User Id=sa;Password=[withheld];"},{"jwt":"[withheld]"}]"#,
    ),
    // Choice 188: the lists of key names the `_keys` suffix would catch print; a mode word
    // under a credential name, and one repeating a sibling `*_type`, is withheld in place and
    // not registered; an empty schema value prints under a credential-named property.
    (
        r#"{"checks":[{"id":"default.requiredMetadataKeys","parameters":{"metadataKeys":["user_id","team"],"operator":"all"}}],"unique_metadata_keys":["env"],"unique_virtual_keys":["openai-prod"]}"#,
        r#"{"checks":[{"id":"default.requiredMetadataKeys","parameters":{"metadataKeys":["user_id","team"],"operator":"all"}}],"unique_metadata_keys":["env"],"unique_virtual_keys":["openai-prod"]}"#,
    ),
    (
        r#"[{"auth":"disabled","status":"disabled"},{"auth":"password","auth_type":"password","note":"Enter the password"}]"#,
        r#"[{"auth":"[withheld]","status":"disabled"},{"auth":"[withheld]","auth_type":"password","note":"Enter the password"}]"#,
    ),
    (
        r#"{"properties":{"client_secret":{"type":"string","default":"","examples":[]},"apiKey":{"type":"string","default":"PIN-REVIEW-0041"}}}"#,
        r#"{"properties":{"client_secret":{"type":"string","default":"","examples":[]},"apiKey":{"type":"string","default":"[withheld]"}}}"#,
    ),
    // Choice 177: a header map named by a `_headers` suffix, a `{header, value}` pair, a list
    // of header names.
    (
        r#"{"extra_headers":{"X-Signature":"PIN-REVIEW-0035","accept":"*/*"},"forward_headers":[{"header":"Authorization","value":"PIN-REVIEW-0036"}],"allowed_headers":["x-trace","Authorization","sk-proj-PINREVIEW0037"],"rules":[{"header":"x-api-key","value":"PIN-REVIEW-0038"}]}"#,
        r#"{"extra_headers":{"X-Signature":"[withheld]","accept":"*/*"},"forward_headers":[{"header":"Authorization","value":"[withheld]"}],"allowed_headers":["x-trace","Authorization","[withheld]"],"rules":[{"header":"x-api-key","value":"[withheld]"}]}"#,
    ),
];

/// Credential names written without the name rule (choice 176): environment variables and
/// member names that hold a credential in services, SDKs and deployment manifests. Each one
/// must be withheld in every container of [`planted`]; unlike [`credential_names`] it is not
/// derived from [`NAME_RULES`], so a shape the rules miss fails the tests.
///
/// Deliberately printed by name, so not listed (each still withheld by its value where a value
/// rule applies): `AWS_ACCESS_KEY_ID`, `access_key_id`, `api_key_id` (identifiers of a key,
/// not the key); `DATABASE_URL`, `REDIS_URL`, `SENTRY_DSN`, `SLACK_WEBHOOK_URL` (URLs: the URL
/// rule withholds a userinfo, query and fragment, a path is printed); `pin`, `otp`, `signature`,
/// `session_id` (words with other meanings; a one-time or per-request value, an identifier);
/// `tokenizer` (a model's tokenizer name).
pub const REAL_WORLD_NAMES: &[&str] = &[
    // Environment variables.
    "DB_PASS",
    "DB_PASSWORD",
    "MYSQL_ROOT_PASSWORD",
    "POSTGRES_PASSWORD",
    "PGPASSWORD",
    "REDIS_PASS",
    "SMTP_PASS",
    "SMTP_PASSWORD",
    "GITHUB_PAT",
    "GITLAB_PAT",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "NPM_TOKEN",
    "PYPI_TOKEN",
    "HF_TOKEN",
    "HUGGING_FACE_HUB_TOKEN",
    "SLACK_BOT_TOKEN",
    "TWILIO_AUTH_TOKEN",
    "SENTRY_AUTH_TOKEN",
    "VAULT_TOKEN",
    "CONSUL_HTTP_TOKEN",
    "DIGITALOCEAN_ACCESS_TOKEN",
    "AWS_SESSION_TOKEN",
    "AWS_SECRET_ACCESS_KEY",
    "AZURE_CLIENT_SECRET",
    "AZURE_OPENAI_KEY",
    "AZURE_STORAGE_KEY",
    "AZURE_STORAGE_CONNECTION_STRING",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "COHERE_KEY",
    "STRIPE_SECRET_KEY",
    "SENDGRID_API_KEY",
    "DD_API_KEY",
    "DD_APP_KEY",
    "NEW_RELIC_LICENSE_KEY",
    "JWT_SECRET",
    "SECRET_KEY_BASE",
    "ENCRYPTION_KEY",
    "SSH_PRIVATE_KEY",
    "WEBHOOK_SECRET",
    // Member names.
    "db_pass",
    "smtp_pass",
    "pass",
    "password",
    "passwd",
    "pwd",
    "passphrase",
    "access_keys",
    "secret_keys",
    "keys",
    "api_keys",
    "tokenvalue",
    "tokenhash",
    "accesstokenvalue",
    "accessToken",
    "refresh_token",
    "id_token",
    "idToken",
    "sas_token",
    "oauth_token_secret",
    "consumer_secret",
    "client_secret",
    "app_secret",
    "private_key",
    "privateKey",
    "signing_key",
    "license_key",
    "storage_account_key",
    "connectionString",
    "bearer_token",
    "authToken",
    "auth",
    "basic_auth",
    "cookie",
    "set-cookie",
    "x-api-key",
    "Authorization",
    "credentials",
    // Choice 181: a JSON Web Token, a bearer credential, an authorization header's value, and
    // run-together lowercase key names.
    "jwt",
    "JWT",
    "id_jwt",
    "bearer",
    "auth_header",
    "AuthHeader",
    "accesskey",
    "masterkey",
    "accountkey",
    // Choice 183: an Azure shared access signature and its signature parameter.
    "SharedAccessSignature",
    "sharedaccesssignature",
    "sig",
];

/// One text of [`REAL_WORLD_TEXTS`]: how the text rules (the URL, scheme and key=value rules,
/// choices 177, 181) print it under a neutral name, which of its words they register and
/// which they must not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealWorldText {
    /// What the text is, for a failure message.
    pub case: &'static str,
    /// The text as received.
    pub text: &'static str,
    /// The text as printed under a neutral name (`note`).
    pub printed: &'static str,
    /// The withheld credentials that are registered: each echoed bare under another neutral
    /// name in the same document is withheld there too.
    pub registered: &'static [&'static str],
    /// Words that stay printed where they are echoed under another neutral name: what a rule
    /// withholds in this text only (a model id after `token` in prose), or nothing withholds.
    pub kept: &'static [&'static str],
}

impl RealWorldText {
    /// The document the CLI's and the harness's tests read (as received, as printed): the
    /// text under `note`, then each registered credential and each kept word echoed bare under
    /// `echo`, one row each.
    pub fn documents(&self) -> (String, String) {
        let mut received = vec![format!("{{\"note\":{}}}", quoted(self.text))];
        let mut printed = vec![format!("{{\"note\":{}}}", quoted(self.printed))];
        for secret in self.registered {
            received.push(format!("{{\"echo\":{}}}", quoted(secret)));
            printed.push(format!("{{\"echo\":{}}}", quoted(super::WITHHELD)));
        }
        for word in self.kept {
            received.push(format!("{{\"echo\":{}}}", quoted(word)));
            printed.push(format!("{{\"echo\":{}}}", quoted(word)));
        }
        (
            format!("[{}]", received.join(",")),
            format!("[{}]", printed.join(",")),
        )
    }
}

/// Texts that hold a credential in practice, written without the rules (choices 181, 183, 186),
/// with the exact printed form: `curl` lines in both header spellings, with an API-key header
/// too, ADO.NET, ODBC, Azure storage, shared access signature and Service Bus connection strings
/// (a password holding a space or a `;` among them), a form body, a command-line flag, a JDBC
/// URL, an AWS credentials file and a configuration line; and the intended limits in the other direction,
/// prose that mentions a model id after `token` or `basic` (withheld in that text only, so the
/// model id under another name stays printed), a credential-named key whose value only says it
/// is absent, unset or masked (`auth=none`, `Password=true`, `Pwd=****`, `Pwd=;`) or is a short
/// word of letters and digits (`auth=oauth2`, `secret=sha256`): withheld in place, never
/// registered; and `key=value`, `key = value` or header text that names no credential. The
/// CLI's tests run each through its rules and [`super::check::violations`]; the harness's
/// through the checker and [`super::values::text_withheld`], so both read what is
/// registered by the same rule.
pub const REAL_WORLD_TEXTS: &[RealWorldText] = &[
    RealWorldText {
        case: "curl, header with a space",
        text: "curl -H 'Authorization: Bearer sk-proj-RWT0001abcdefghij' https://api.example.test/v1/chat",
        printed: "curl -H 'Authorization: Bearer [withheld]' https://api.example.test/v1/chat",
        registered: &["sk-proj-RWT0001abcdefghij"],
        kept: &[],
    },
    RealWorldText {
        case: "curl, header without a space",
        text: "curl -H Authorization:Bearer sk-proj-RWT0002abcdefghij https://api.example.test/v1/chat",
        printed: "curl -H Authorization:Bearer [withheld] https://api.example.test/v1/chat",
        registered: &["sk-proj-RWT0002abcdefghij"],
        kept: &[],
    },
    RealWorldText {
        case: "curl, quoted proxy header without a space",
        text: "curl -H \"Proxy-Authorization:Basic c3ZjOlJXVDAwMDNwdw==\" https://proxy.example.test",
        printed: "curl -H \"Proxy-Authorization:Basic [withheld]\" https://proxy.example.test",
        registered: &["c3ZjOlJXVDAwMDNwdw=="],
        kept: &[],
    },
    RealWorldText {
        case: "a header value",
        text: "Token RWT0004abcdefghijklm",
        printed: "Token [withheld]",
        registered: &["RWT0004abcdefghijklm"],
        kept: &[],
    },
    RealWorldText {
        case: "a bearer token in prose",
        text: "retry with Bearer RWT0005abcdefghijklm later",
        printed: "retry with Bearer [withheld] later",
        registered: &["RWT0005abcdefghijklm"],
        kept: &[],
    },
    RealWorldText {
        case: "ADO.NET connection string",
        text: "Server=tcp:db.example.test,1433;Database=app;User Id=sa;Password=RWT0006pass!;Encrypt=True;",
        printed: "Server=tcp:db.example.test,1433;Database=app;User Id=sa;Password=[withheld];Encrypt=True;",
        registered: &["RWT0006pass!"],
        kept: &["tcp:db.example.test,1433"],
    },
    RealWorldText {
        case: "ODBC connection string",
        text: "Driver={ODBC Driver 18};Server=db;Uid=app;Pwd=RWT0007pw!x;",
        printed: "Driver={ODBC Driver 18};Server=db;Uid=app;Pwd=[withheld];",
        registered: &["RWT0007pw!x"],
        kept: &[],
    },
    RealWorldText {
        case: "Azure storage connection string",
        text: "DefaultEndpointsProtocol=https;AccountName=acct;AccountKey=RWT0008abcdefghijklmnop+/==;EndpointSuffix=core.windows.net",
        printed: "DefaultEndpointsProtocol=https;AccountName=acct;AccountKey=[withheld];EndpointSuffix=core.windows.net",
        registered: &["RWT0008abcdefghijklmnop+/=="],
        kept: &[],
    },
    RealWorldText {
        case: "Service Bus connection string",
        text: "Endpoint=sb://ns.servicebus.windows.net/;SharedAccessKeyName=RootManageSharedAccessKey;SharedAccessKey=RWT0009abcdefghijklmn=",
        printed: "Endpoint=sb://ns.servicebus.windows.net/;SharedAccessKeyName=RootManageSharedAccessKey;SharedAccessKey=[withheld]",
        registered: &["RWT0009abcdefghijklmn="],
        kept: &["RootManageSharedAccessKey"],
    },
    RealWorldText {
        case: "a form body",
        text: "grant_type=client_credentials&client_id=app&client_secret=RWT0010secretvalue",
        printed: "grant_type=client_credentials&client_id=app&client_secret=[withheld]",
        registered: &["RWT0010secretvalue"],
        kept: &["client_credentials"],
    },
    RealWorldText {
        case: "a command-line token flag",
        text: "export AIRS_TOKEN && run --token=sk-RWT0011abcdefghijk",
        printed: "export AIRS_TOKEN && run --token=[withheld]",
        registered: &["sk-RWT0011abcdefghijk"],
        kept: &[],
    },
    RealWorldText {
        case: "a JDBC URL with a password in its query",
        text: "jdbc:postgresql://db.example.test:5432/app?user=app&password=RWT0012pass!",
        printed: "jdbc:postgresql://db.example.test:5432/app?[withheld]",
        registered: &["RWT0012pass!"],
        kept: &[],
    },
    RealWorldText {
        case: "an error naming a model after token",
        text: "token gpt-4o-mini-2024-07-18 quota exceeded",
        printed: "token [withheld] quota exceeded",
        registered: &[],
        kept: &["gpt-4o-mini-2024-07-18"],
    },
    RealWorldText {
        case: "a description naming a model after basic",
        text: "basic claude-3-5-sonnet-20241022 routing",
        printed: "basic [withheld] routing",
        registered: &[],
        kept: &["claude-3-5-sonnet-20241022"],
    },
    RealWorldText {
        case: "a sentence starting with Token",
        text: "Token budget for gpt-4o-mini-2024-07-18 exceeded",
        printed: "Token budget for gpt-4o-mini-2024-07-18 exceeded",
        registered: &[],
        kept: &["gpt-4o-mini-2024-07-18"],
    },
    RealWorldText {
        case: "a two-word label after the lowercase prose word token",
        text: "token gpt-4o-mini-2024-07-18",
        printed: "token [withheld]",
        registered: &[],
        kept: &["gpt-4o-mini-2024-07-18"],
    },
    RealWorldText {
        case: "a two-word label after the lowercase prose word basic",
        text: "basic claude-3-5-sonnet-20241022",
        printed: "basic [withheld]",
        registered: &[],
        kept: &["claude-3-5-sonnet-20241022"],
    },
    RealWorldText {
        case: "a credential key set to none",
        text: "Set auth=none for local dev",
        printed: "Set auth=[withheld] for local dev",
        registered: &[],
        kept: &["none", "nonexistent-check"],
    },
    RealWorldText {
        case: "a credential key set to a JSON literal",
        text: "note: Password=true; Encrypt=true",
        printed: "note: Password=[withheld]; Encrypt=true",
        registered: &[],
        kept: &["true"],
    },
    RealWorldText {
        case: "a credential key set to a mask",
        text: "Server=db;Uid=app;Pwd=********;",
        printed: "Server=db;Uid=app;Pwd=[withheld];",
        registered: &[],
        kept: &["********"],
    },
    RealWorldText {
        case: "ODBC with integrated security and an empty password",
        text: "Driver={ODBC Driver 18};Server=db;Trusted_Connection=yes;Uid=;Pwd=;",
        printed: "Driver={ODBC Driver 18};Server=db;Trusted_Connection=yes;Uid=;Pwd=;",
        registered: &[],
        kept: &["Trusted_Connection=yes"],
    },
    RealWorldText {
        case: "Azure blob shared access signature connection string",
        text: "BlobEndpoint=https://acct.blob.core.windows.net/;SharedAccessSignature=sv=2022-11-02&ss=b&srt=co&sp=rl&se=2026-12-31T00:00:00Z&sig=RWT0013abcdefghij%2Bklmn%3D",
        printed: "BlobEndpoint=https://acct.blob.core.windows.net/;SharedAccessSignature=[withheld]",
        registered: &["RWT0013abcdefghij%2Bklmn%3D"],
        kept: &["acct.blob.core.windows.net"],
    },
    RealWorldText {
        case: "a shared access signature parameter in prose",
        text: "append &sig=RWT0014abcdefghijklmn to the blob path",
        printed: "append &sig=[withheld] to the blob path",
        registered: &["RWT0014abcdefghijklmn"],
        kept: &[],
    },
    RealWorldText {
        case: "curl, an API-key header with a space",
        text: "curl -H 'X-Api-Key: RWT0015abcdefghijklm' https://api.example.test/v1/models",
        printed: "curl -H 'X-Api-Key: [withheld]' https://api.example.test/v1/models",
        registered: &["RWT0015abcdefghijklm"],
        kept: &[],
    },
    RealWorldText {
        case: "curl, an Azure api-key header without a space",
        text: "curl -H \"api-key:RWT0016abcdefghijklm\" https://res.openai.azure.com/openai/models",
        printed: "curl -H \"api-key:[withheld]\" https://res.openai.azure.com/openai/models",
        registered: &["RWT0016abcdefghijklm"],
        kept: &[],
    },
    // Choice 186: a connection string's password holding a space or a `;`, an INI or TOML
    // credential line with spaces around the `=`, and a credential key set to a short word of
    // letters and digits (withheld in place, never registered).
    RealWorldText {
        case: "ADO.NET connection string, a password holding a space",
        text: "Server=db;User Id=sa;Password=RWT0017p@ss w0rd;Encrypt=True;",
        printed: "Server=db;User Id=sa;Password=[withheld];Encrypt=True;",
        registered: &["RWT0017p@ss w0rd"],
        kept: &[],
    },
    RealWorldText {
        case: "ODBC connection string, a braced password holding a `;` and an escaped `}`",
        text: "Driver={ODBC Driver 18};Server=db;UID=app;PWD={RWT0018p@ss;w0}}rd};",
        printed: "Driver={ODBC Driver 18};Server=db;UID=app;PWD=[withheld];",
        registered: &["RWT0018p@ss;w0}rd"],
        kept: &[],
    },
    RealWorldText {
        case: "an AWS credentials file",
        text: "[default]\naws_access_key_id = AKIAIOSFODNN7EXAMPLE\naws_secret_access_key = RWT0019wJalrXUtnFEMI/K7MDENG/bPxRfiCY\n",
        printed: "[default]\naws_access_key_id = AKIAIOSFODNN7EXAMPLE\naws_secret_access_key = [withheld]\n",
        registered: &["RWT0019wJalrXUtnFEMI/K7MDENG/bPxRfiCY"],
        kept: &["AKIAIOSFODNN7EXAMPLE"],
    },
    RealWorldText {
        case: "a configuration line with spaces around the `=`",
        text: "set password = RWT0020hunter2x! in the config",
        printed: "set password = [withheld] in the config",
        registered: &["RWT0020hunter2x!"],
        kept: &[],
    },
    RealWorldText {
        case: "a credential key set to a short letters-and-digits word",
        text: "Use auth=oauth2 for remote",
        printed: "Use auth=[withheld] for remote",
        registered: &[],
        kept: &["oauth2"],
    },
    RealWorldText {
        case: "credential keys set to an algorithm and a version",
        text: "signed with secret=sha256 and pwd=v2.0 today",
        printed: "signed with secret=[withheld] and pwd=[withheld] today",
        registered: &[],
        kept: &["sha256", "v2.0"],
    },
    RealWorldText {
        case: "header text that names no credential",
        text: "send the x-api-key: header; x-ratelimit-remaining-tokens: 1999 x-request-id: req-abc123def456gh",
        printed: "send the x-api-key: header; x-ratelimit-remaining-tokens: 1999 x-request-id: req-abc123def456gh",
        registered: &[],
        kept: &["req-abc123def456gh"],
    },
    RealWorldText {
        case: "key=value text that names no credential",
        text: "max_tokens=100 prompt_tokens=12 AccountName=acct sort key=name model=gpt-4o-mini-2024-07-18",
        printed: "max_tokens=100 prompt_tokens=12 AccountName=acct sort key=name model=gpt-4o-mini-2024-07-18",
        registered: &[],
        kept: &["gpt-4o-mini-2024-07-18"],
    },
    RealWorldText {
        case: "spaced key = value text that names no credential",
        text: "if a = b then max_tokens = 100 and sort key = name; x == y",
        printed: "if a = b then max_tokens = 100 and sort key = name; x == y",
        registered: &[],
        kept: &["name"],
    },
];

/// The `_` words of a word of [`NAME_RULES`] (`apikey` is `api` `key`), for its spaced,
/// dotted and camelCase forms.
fn words_of(word: &str) -> Vec<&str> {
    match word {
        "apikey" => vec!["api", "key"],
        "clientauth" => vec!["client", "auth"],
        "authcode" => vec!["auth", "code"],
        "privatekey" => vec!["private", "key"],
        word => word.split('_').filter(|part| !part.is_empty()).collect(),
    }
}

/// A word with its first letter uppercased.
fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The written forms of a name made of `words` (choice 173): snake, kebab, spaced, dotted,
/// camel and Pascal case, upper snake, title-case kebab, and, when `joined`, the words run
/// together in lower and upper case.
pub fn forms(words: &[&str], joined: bool) -> Vec<String> {
    let lower: Vec<String> = words.iter().map(|word| word.to_lowercase()).collect();
    let mut out = vec![
        lower.join("_"),
        lower.join("-"),
        lower.join(" "),
        lower.join("."),
        lower
            .iter()
            .map(|word| word.to_uppercase())
            .collect::<Vec<_>>()
            .join("_"),
        lower
            .iter()
            .map(|word| capitalised(word))
            .collect::<Vec<_>>()
            .join("-"),
        lower
            .iter()
            .map(|word| capitalised(word))
            .collect::<Vec<_>>()
            .concat(),
        lower
            .iter()
            .enumerate()
            .map(|(at, word)| {
                if at == 0 {
                    word.clone()
                } else {
                    capitalised(word)
                }
            })
            .collect::<Vec<_>>()
            .concat(),
    ];
    if joined {
        out.push(lower.concat());
        out.push(lower.concat().to_uppercase());
    }
    let mut unique = Vec::new();
    for form in out {
        if !unique.contains(&form) {
            unique.push(form);
        }
    }
    unique
}

/// The prefixes a suffix row is tried with (`account_key`, `aws_secret_access_key`).
const SUFFIX_PREFIXES: &[&[&str]] = &[
    &["access"],
    &["account"],
    &["master"],
    &["encryption"],
    &["x", "api"],
    &["aws", "secret", "access"],
    &["basic"],
    &["proxy"],
];

/// The prefix and suffix words a credential word is tried with (`db_password`,
/// `password_value`).
const WORD_PREFIXES: &[&[&str]] = &[&[], &["db"], &["x"]];
const WORD_SUFFIXES: &[&[&str]] = &[&[], &["value"]];

/// The prefixes a whole-word credential word is tried with (`db_pass`, `GITHUB_PAT`).
const WORD_ONLY_PREFIXES: &[&[&str]] = &[&[], &["db"], &["smtp"], &["github"]];

/// The prefixes a last-word credential name is tried with (`vault_secret_id`, `db_secret_url`).
const LAST_PREFIXES: &[&[&str]] = &[&[], &["vault"], &["db"]];

/// The prefixes a token word is tried with (`session_token`, `bearerTokens`, `tokenvalue`).
const TOKEN_PREFIXES: &[&[&str]] = &[
    &[],
    &["session"],
    &["bearer"],
    &["refresh"],
    &["api"],
    &["access"],
];

/// The suffix words a token word is tried with (`token_value`, `tokenhash`).
const TOKEN_SUFFIXES: &[&[&str]] = &[&[], &["value"], &["hash"]];

/// Every credential-name shape the name rule claims (choice 173), from the rows of
/// [`NAME_RULES`] that withhold: an exact name in every form; a suffix after each of
/// [`SUFFIX_PREFIXES`] whose last word it is; a credential word alone or with a prefix and a
/// suffix word, also run together; a whole-word credential word alone or after a prefix, as
/// separate words only; a token word, singular or plural, alone or after a prefix and before
/// a suffix word, also run together. Each one classifies as its row says or stronger (a test in
/// this module).
pub fn credential_names() -> Vec<(String, Class)> {
    let mut out: Vec<(String, Class)> = Vec::new();
    let mut push = |name: String, class: Class| {
        if !out.iter().any(|(known, _)| *known == name) {
            out.push((name, class));
        }
    };
    for rule in NAME_RULES.iter().filter(|rule| rule.class >= Class::Count) {
        for word in rule.pattern.words() {
            match rule.pattern {
                Pattern::Exact(_) | Pattern::Prefix(_) => {
                    for form in forms(&words_of(word), false) {
                        push(form, rule.class);
                    }
                }
                Pattern::Suffix(_) => {
                    let last = word.trim_start_matches('_');
                    for prefix in SUFFIX_PREFIXES {
                        let mut words = prefix.to_vec();
                        words.push(last);
                        for form in forms(&words, false) {
                            push(form, rule.class);
                        }
                    }
                }
                Pattern::Joined(_) => {
                    for prefix in WORD_PREFIXES {
                        for suffix in WORD_SUFFIXES {
                            let mut words = prefix.to_vec();
                            words.extend(words_of(word));
                            words.extend(suffix.iter());
                            for form in forms(&words, true) {
                                push(form, rule.class);
                            }
                        }
                    }
                }
                Pattern::Word(_) => {
                    for prefix in WORD_ONLY_PREFIXES {
                        for suffix in WORD_SUFFIXES {
                            let mut words = prefix.to_vec();
                            words.push(word);
                            words.extend(suffix.iter());
                            for form in forms(&words, false) {
                                push(form, rule.class);
                            }
                        }
                    }
                }
                Pattern::Last(_) => {
                    for prefix in LAST_PREFIXES {
                        let mut words = prefix.to_vec();
                        words.extend(words_of(word));
                        for form in forms(&words, false) {
                            push(form, rule.class);
                        }
                    }
                }
                Pattern::LastAfter(_, within) => {
                    for fragment in within.iter() {
                        for prefix in LAST_PREFIXES {
                            let mut words = prefix.to_vec();
                            words.extend(words_of(fragment));
                            words.push(word);
                            for form in forms(&words, false) {
                                push(form, rule.class);
                            }
                        }
                    }
                }
                Pattern::Within(..) => {
                    let plural = format!("{word}s");
                    for prefix in TOKEN_PREFIXES {
                        for suffix in TOKEN_SUFFIXES {
                            for word in [word, plural.as_str()] {
                                let mut words = prefix.to_vec();
                                words.push(word);
                                words.extend(suffix.iter());
                                for form in forms(&words, true) {
                                    push(form, rule.class);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// One generated document: `secret` must appear in no output format of `document`, and
/// `control` (printed under a plain name) must appear in each, so the check reads real output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planted {
    /// The document as JSON text.
    pub document: String,
    /// The planted credential.
    pub secret: String,
    /// A plain value the document prints.
    pub control: String,
    /// The name and container shape, for a failure message.
    pub case: String,
}

/// A JSON string literal of `text` (`"` and `\` escaped, the control characters as `\n` or
/// `\u00XX`).
pub fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The container shapes a credential named `name` is planted in (choice 173), each as
/// `(label, member)`: the member is the JSON text of one top-level member holding `value` (a
/// JSON value) somewhere under `name`. `depth` nests the plain-member case that deep.
fn containers(name: &str, value: &str, depth: usize) -> Vec<(&'static str, String)> {
    let n = quoted(name);
    let mut nested = format!("{{{n}:{value}}}");
    for level in 0..depth {
        nested = if level % 2 == 0 {
            format!("[{nested}]")
        } else {
            format!("{{\"level{level}\":{nested}}}")
        };
    }
    let as_text = quoted(&format!("{{{n}:{value}}}"));
    vec![
        ("member", format!("{n}:{value}")),
        ("nested", format!("\"wrapper\":{nested}")),
        ("list under the name", format!("{n}:[{value}]")),
        (
            "object under the name",
            format!("{n}:{{\"inner\":{value}}}"),
        ),
        (
            "name/value pair",
            format!("\"env\":[{{\"name\":{n},\"value\":{value}}}]"),
        ),
        (
            "key/value pair",
            format!("\"tags\":[{{\"key\":{n},\"value\":{value}}}]"),
        ),
        (
            "header/value pair",
            format!("\"rules\":[{{\"header\":{n},\"value\":{value}}}]"),
        ),
        ("header map", format!("\"headers\":{{{n}:{value}}}")),
        (
            "suffixed header map",
            format!("\"extra_headers\":{{{n}:{value}}}"),
        ),
        (
            "header list",
            format!("\"forward_headers\":[{{\"name\":{n},\"value\":{value}}}]"),
        ),
        ("JSON text", format!("\"config\":{as_text}")),
        (
            "schema default",
            format!("\"properties\":{{{n}:{{\"type\":\"string\",\"default\":{value}}}}}"),
        ),
    ]
}

/// Every name [`planted`] plants a credential under: [`credential_names`], then each of
/// [`REAL_WORLD_NAMES`] the rows did not already give, with the class the rules give it.
pub fn planted_names() -> Vec<(String, Class)> {
    let mut names = credential_names();
    for name in REAL_WORLD_NAMES {
        if !names.iter().any(|(known, _)| known == name) {
            names.push(((*name).to_owned(), classify(name)));
        }
    }
    names
}

/// The generated documents (choices 173, 176, 177, 187): for every name of [`planted_names`], a
/// string credential in every container shape, its echo under a plain name and the token after
/// `Bearer`, and under every credential name and member an eight-digit number (a numeric
/// password or PIN) in every container shape and with its echo; then a private
/// key under neutral names, a credential in a URL's userinfo and query, and a credential after
/// an authorization scheme under neutral names. Deterministic: the secrets are numbered, the
/// nesting depth cycles from 0 to 3.
pub fn planted() -> Vec<Planted> {
    let mut out = Vec::new();
    let mut add = |member: String, secret: String, case: String| {
        let control = format!("control{:05}visible", out.len());
        out.push(Planted {
            document: format!("{{\"label\":{},{member}}}", quoted(&control)),
            secret,
            control,
            case,
        });
    };
    for (at, (name, _)) in planted_names().into_iter().enumerate() {
        let secret = |kind: usize| format!("Pl4nt{at:04}k{kind}xQ9vT7mZ0kR3");
        for (label, member) in containers(&name, &quoted(&secret(0)), at % 4) {
            add(member, secret(0), format!("{name} ({label})"));
        }
        let echo = secret(1);
        add(
            format!(
                "{}:{},\"note\":{}",
                quoted(&name),
                quoted(&echo),
                quoted(&format!("see {echo} here"))
            ),
            echo,
            format!("{name} (echo)"),
        );
        let bearer = secret(2);
        add(
            format!(
                "{}:{},\"note\":{}",
                quoted(&name),
                quoted(&format!("Bearer {bearer}")),
                quoted(&bearer)
            ),
            bearer,
            format!("{name} (bearer echo)"),
        );
        if matches!(classify(&name), Class::Credential | Class::Member) {
            let pin = (31_000_000 + at).to_string();
            for (label, member) in containers(&name, &pin, at % 4) {
                add(member, pin.clone(), format!("{name} (number, {label})"));
            }
            add(
                format!(
                    "{}:{pin},\"note\":{}",
                    quoted(&name),
                    quoted(&format!("pin {pin}"))
                ),
                pin,
                format!("{name} (number echo)"),
            );
        }
    }
    let armours = [
        "PRIVATE KEY",
        "RSA PRIVATE KEY",
        "EC PRIVATE KEY",
        "ENCRYPTED PRIVATE KEY",
        "OPENSSH PRIVATE KEY",
        "PGP PRIVATE KEY BLOCK",
    ];
    for (at, armour) in armours.iter().enumerate() {
        for name in ["cert", "pem", "value", "description", "certificate_chain"] {
            let secret = format!("MIIEvQIBADANBgkqhkiG9w0BAQEFAASC{at}{}", name.len());
            let pem = format!("-----BEGIN {armour}-----\n{secret}\n-----END {armour}-----\n");
            for (label, member) in containers(name, &quoted(&pem), at % 4) {
                add(
                    member,
                    secret.clone(),
                    format!("{armour} under {name} ({label})"),
                );
            }
            add(
                format!("\"headers\":{{\"accept\":{}}}", quoted(&pem)),
                secret.clone(),
                format!("{armour} in a safe header"),
            );
        }
    }
    for (at, url) in [
        "https://user:{s}@llm.example/v1",
        "connect ECONNREFUSED https://admin:{s}@db.internal/x",
        "https://h.example/x?sig={s}",
        "see https://h.example/x#{s} and more",
    ]
    .iter()
    .enumerate()
    {
        let secret = format!("Ur1{at}zQ9vT7mZ0kR3w");
        let text = url.replace("{s}", &secret);
        for name in ["base_url", "error", "description"] {
            add(
                format!("{}:{}", quoted(name), quoted(&text)),
                secret.clone(),
                format!("URL {at} under {name}"),
            );
        }
    }
    for (at, text) in [
        "Bearer {s}",
        "bearer {s}",
        "Token {s}",
        "Basic {s}==",
        "curl -H 'Authorization: Bearer {s}' https://h.example/v1",
        "-H \"Authorization: Basic {s}\"",
        "(Token {s}),",
    ]
    .iter()
    .enumerate()
    {
        let secret = format!("Sch3m{at}zQ9vT7mZ0kR3w");
        let text = text.replace("{s}", &secret);
        for name in ["auth_header", "authentication", "note", "description"] {
            for (label, member) in containers(name, &quoted(&text), at % 4)
                .into_iter()
                .filter(|(label, _)| matches!(*label, "member" | "nested" | "JSON text"))
            {
                add(
                    member,
                    secret.clone(),
                    format!("scheme text {at} under {name} ({label})"),
                );
            }
        }
    }
    out
}

/// Whether a generated name classifies as its row says or stronger (a weaker class would
/// mean the generator claims a shape the rule does not withhold).
pub fn classifies_as_claimed(name: &str, class: Class) -> bool {
    classify(name) >= class
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_generated_name_is_a_credential_name_of_its_row() {
        let names = credential_names();
        assert!(names.len() > 300, "{}", names.len());
        for (name, class) in &names {
            assert!(
                classifies_as_claimed(name, *class),
                "{name}: {:?}",
                classify(name)
            );
        }
        for name in [
            "api key",
            "api.key",
            "dbPwd",
            "x-passphrase-value",
            "SESSION_TOKEN",
            "session token",
            "MasterKey",
            "Basic-Auth",
            "DBPASSWORDVALUE",
            "db_pass",
            "SMTP_PASS",
            "dbPass",
            "GITHUB_PAT",
            "access_keys",
            "AccessKeys",
            "tokenvalue",
            "TOKENVALUE",
            "accesstokenhash",
            "sessionTokensValue",
        ] {
            assert!(names.iter().any(|(known, _)| known == name), "{name}");
        }
    }

    /// Choice 176: every name of the rule-independent list is one the rules withhold (a name
    /// they miss fails here and in every planted container of the CLI's test).
    #[test]
    fn every_real_world_credential_name_is_withheld_by_name() {
        for name in REAL_WORLD_NAMES {
            assert!(
                classify(name) >= Class::Count,
                "{name}: {:?}",
                classify(name)
            );
        }
        for name in [
            "AWS_ACCESS_KEY_ID",
            "DATABASE_URL",
            "SENTRY_DSN",
            "session_id",
            "tokenizer",
        ] {
            assert!(!REAL_WORLD_NAMES.contains(&name), "{name} is an exclusion");
            assert!(
                classify(name) < Class::Count,
                "{name}: {:?}",
                classify(name)
            );
        }
    }

    /// Choices 181, 183: the text rules alone (`values::text_withheld`) print every
    /// rule-independent text as pinned, register what can be registered only
    /// (`names::registrable`), carry each `registered` credential (so its echo under
    /// another name is withheld by the `SecretSet` rule) and carry no `kept` word (so the word
    /// under another name stays printed: `auth=none` does not withhold `mode: none`).
    #[test]
    fn every_real_world_text_registers_as_pinned() {
        use super::super::names::registrable;
        use super::super::values::text_withheld;
        for case in REAL_WORLD_TEXTS {
            let (shown, parts) =
                text_withheld(case.text).unwrap_or_else(|| (case.text.to_owned(), Vec::new()));
            assert_eq!(shown, case.printed, "{}", case.case);
            assert_eq!(text_withheld(&shown), None, "{}: idempotent", case.case);
            for part in &parts {
                assert!(registrable(part), "{}", case.case);
            }
            let carried = |word: &str| parts.iter().any(|part| word.contains(part.as_str()));
            for secret in case.registered {
                assert!(carried(secret), "{}: {secret}", case.case);
            }
            for word in case.kept {
                assert!(!carried(word), "{}: {word} {parts:?}", case.case);
            }
        }
    }

    #[test]
    fn generated_documents_are_distinct_and_numbered() {
        let planted = planted();
        assert!(planted.len() > 5000, "{}", planted.len());
        for case in planted.iter().take(50) {
            assert!(case.document.contains(&case.secret), "{}", case.case);
            assert!(case.document.contains(&case.control), "{}", case.case);
        }
    }
}
