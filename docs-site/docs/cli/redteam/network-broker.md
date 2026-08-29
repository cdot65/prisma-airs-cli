---
sidebar_label: network-broker
---

# redteam network-broker

Manage Red Team **network broker channels** — the data-plane relays that connect
red team clients to targets. Channels live on a distinct network-broker endpoint;
set `PANW_RED_TEAM_NETWORK_BROKER_ENDPOINT` (or `redTeamNetworkBrokerEndpoint` in
config) to override it. OAuth credentials are shared with the other Red Team
commands (`PANW_MGMT_*`).

### redteam network-broker channels list

List network broker channels.

```text
airs redteam network-broker channels list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | — | Max results |
| `--offset <n>` | No | — | Starting offset |
| `--search <text>` | No | — | Filter by search text |
| `--status <status...>` | No | — | Filter by status (`ONLINE`, `OFFLINE`, `DRAFT`) |
| `--all` | No | — | Walk every page |
| `--max <n>` | No | `10000` | Safety cap for `--all`; `0` removes the cap |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

#### Examples

```bash
airs redteam network-broker channels list
airs redteam network-broker channels list --status ONLINE DRAFT --output json
```

### redteam network-broker channels get

Get a single channel by ID.

```text
airs redteam network-broker channels get <channelId> [options]
```

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

### redteam network-broker channels create

Create a channel.

```text
airs redteam network-broker channels create --name <name> [options]
```

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | Yes | — | Channel name |
| `--description <text>` | No | — | Channel description |

#### Examples

```bash
airs redteam network-broker channels create --name "prod-relay" --description "Production broker"
```

### redteam network-broker channels update

Update a channel. At least one of `--name` / `--description` is required.

```text
airs redteam network-broker channels update <channelId> [options]
```

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | No | — | New channel name |
| `--description <text>` | No | — | New channel description |

### redteam network-broker stats

Show aggregate channel statistics (online/total channel counts, docker registry,
helm chart, docker image, client version).

```text
airs redteam network-broker stats [options]
```

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |
