# Target Init Command — Design Spec

## Summary

Add `airs redteam targets init <provider>` command that scaffolds a target config JSON file from AIRS provider templates, bridging the gap between `targets templates` (read-only display) and `targets create --config` (requires hand-crafted JSON).

## Command Interface

```
airs redteam targets init <provider> [--output <file>]
```

- `<provider>` — required positional arg, case-insensitive, validated against template keys: `OPENAI`, `HUGGING_FACE`, `DATABRICKS`, `BEDROCK`, `REST`, `STREAMING`, `WEBSOCKET`
- `--output <file>` — optional, defaults to `./<provider-lowercase>-target.json`
- Invalid provider: exit 1, error listing valid providers
- File already exists: exit 1, error suggesting `--output` for a different path
- No `--force` overwrite flag

## Data Flow

1. Handler calls `service.getTargetTemplates()` (existing method, hits `/v1/template/target-templates`)
2. Normalize user input to uppercase, look up `templates[PROVIDER]`
3. Build scaffold:

```json
{
  "name": "",
  "target_type": "OPENAI",
  "connection_params": { "<template blob from API>" },
  "background": {},
  "additional_context": {},
  "metadata": {}
}
```

4. `JSON.stringify(scaffold, null, 2)` → `fs.writeFileSync()` to output path
5. Print: file path, reminder to fill `name` + credentials, follow-up command (`airs redteam targets create --config <path> [--validate]`)

## What's NOT Needed

- No new service methods
- No new types
- No renderer changes
- No template caching
- No interactive wizard

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Invalid provider | `Error: Unknown provider "azure". Valid providers: OPENAI, HUGGING_FACE, DATABRICKS, BEDROCK, REST, STREAMING, WEBSOCKET` |
| File already exists | `Error: File already exists: ./openai-target.json (use --output to specify a different path)` |
| API failure | Existing `renderError` + `process.exit(1)` pattern |

## Testing

- 1 unit test: valid provider → correct scaffold structure (mock `getTargetTemplates`)
- 1 unit test: invalid provider → error with valid provider list

## User Workflow

```bash
# 1. Scaffold a config file from the OpenAI template
airs redteam targets init openai

# 2. Edit the file — fill in name, API key, endpoint
vim openai-target.json

# 3. Create the target
airs redteam targets create --config openai-target.json --validate
```
