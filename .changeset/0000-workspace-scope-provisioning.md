---
"@cdot65/prisma-airs-cli": minor
---

`airs aigateway workspaces create` now provisions a workspace the way Strata Cloud Manager's UI does: it creates the IAM scope first, creates the workspace with that `scope_name`, then binds the scope to the new workspace slug. `--scope-name` is optional (defaults to SCM's `ws_<name>_<suffix>` convention) and `--existing-scope` binds a scope that already exists. New `airs aigateway scopes {list,get,create,bind,delete}` manage SCM IAM scopes directly; `PANW_IAM_ENDPOINT` / `iamEndpoint` override the IAM base URL. Requires SDK 0.31.0.
