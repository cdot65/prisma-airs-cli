# model-security pypi-auth

## model-security pypi-auth

Get PyPI authentication URL for Google Artifact Registry

```text
airs model-security pypi-auth [options]
```

### Examples

*Fetches a short-lived OAuth-based PyPI index URL for the AIRS Model
Security private registry. The token in the URL is redacted here; in
real output it is a Google OAuth access token valid until `Expires`.
*

```bash
airs model-security pypi-auth
```

```text
Prisma AIRS — Model Security
ML model supply chain security


PyPI Authentication:

  URL:     https://oauth2accesstoken:ya29.<redacted-oauth-token>@us-west1-python.pkg.dev/pairs-modelsec-prd-l9mu/ms-prod-usw1-mp-main-pypi/simple/
  Expires: 2026-05-25T15:06:25
```
