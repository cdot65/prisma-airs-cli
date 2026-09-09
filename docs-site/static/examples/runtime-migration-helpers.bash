#!/usr/bin/env bash
# Source this file in Bash. Defines helpers only: sourcing makes no API calls or writes.
# Use set -euo pipefail in the calling workflow. Never suppress errors around create_test_dlp.
capture() {
  local file="$1" rc
  shift
  local destination
  for destination in "$file" "$file.command.txt" "$file.stderr" "$file.exit-code.txt"; do
    if [[ -e "$destination" || -L "$destination" ]]; then
      printf 'Existing evidence is not overwritten: %s\n' "$destination" >&2
      return 1
    fi
  done
  printf '%q ' "$@" > "$file.command.txt"
  printf '\n' >> "$file.command.txt"
  if "$@" > "$file" 2> "$file.stderr"; then rc=0; else rc=$?; fi
  printf '%s\n' "$rc" > "$file.exit-code.txt"
  cat "$file"
  cat "$file.stderr" >&2
  return "$rc"
}

json_id() {
  node -e '
    const p = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8"));
    const id = p.id;
    if (!((typeof id === "string" && id.length > 0) ||
      (typeof id === "number" && Number.isSafeInteger(id)))) throw Error("Invalid resource ID");
    process.stdout.write(String(id));
  ' "$1"
}

assert_empty() {
  node -e '
    for (const f of process.argv.slice(1)) {
      const v = JSON.parse(require("node:fs").readFileSync(f, "utf8"));
      if (!Array.isArray(v) || v.length) throw Error(`Expected empty inventory: ${f}`);
    }
  ' "$@"
}

load_tenant_ids() {
  local ids
  ids=$(airs tenant list --output json | node -e '
    const fs = require("node:fs");
    const tenants = JSON.parse(fs.readFileSync(0, "utf8"));
    const prod = tenants.find(t => t.name === "prod"), dev = tenants.find(t => t.name === "dev");
    if (!/^[0-9]+$/.test(prod?.tsgId ?? "") || !/^[0-9]+$/.test(dev?.tsgId ?? "") || prod.tsgId === dev.tsgId)
      throw Error("Register distinct prod/dev tenants before continuing");
    console.log(prod.tsgId, dev.tsgId);
  ') || return
  read -r PROD_TSG DEV_TSG <<< "$ids"
  export PROD_TSG DEV_TSG
}

create_test_dlp() {
  local prefix="$1" pattern_id dlp_id
  capture "$prefix-pattern-create.json" airs runtime dlp patterns create \
    --name dlp-test-pattern --type custom --technique regex \
    --description 'Synthetic Runtime migration acceptance pattern' \
    --confidence-levels high --regex 'AIRS-E2E-[0-9]{6}' --output json
  pattern_id=$(json_id "$prefix-pattern-create.json")
  capture "$prefix-pattern.json" airs runtime dlp patterns get "$pattern_id" --output json

  node - "$prefix" <<'NODE'
const fs = require('node:fs');
const prefix = process.argv[2];
const p = JSON.parse(fs.readFileSync(`${prefix}-pattern.json`, 'utf8'));
if (!p.id || p.name !== 'dlp-test-pattern') throw Error('Unexpected pattern read-back');
const leaf = {
  detection_technique: 'regex', id: String(p.id), name: p.name,
  match_type: 'include', confidence_level: 'high',
  occurrence_operator_type: 'any', occurrence_count: 1,
  ...(Number.isInteger(p.version) ? {version: p.version} : {}),
};
const body = {
  name: 'dlp-test', profile_type: 'advanced',
  description: 'Synthetic Runtime migration acceptance DLP profile',
  detection_rules: [{rule_type: 'expression_tree', expression_tree: {
    operator_type: 'or', sub_expressions: [{rule_item: leaf}],
  }}],
};
fs.writeFileSync(`${prefix}-dlp-test-request.json`, JSON.stringify(body, null, 2),
  {flag: 'wx', mode: 0o600});
NODE

  capture "$prefix-dlp-create.json" airs runtime dlp profiles create \
    --body-file "$prefix-dlp-test-request.json" --output json
  dlp_id=$(json_id "$prefix-dlp-create.json")
  capture "$prefix-dlp-test.json" airs runtime dlp profiles get "$dlp_id" --output json

  # A create acknowledgement alone is not sufficient: check the stored rule and pattern.
  node - "$prefix" <<'NODE'
const fs = require('node:fs'), assert = require('node:assert/strict');
const prefix = process.argv[2];
const p = JSON.parse(fs.readFileSync(`${prefix}-pattern.json`, 'utf8'));
const d = JSON.parse(fs.readFileSync(`${prefix}-dlp-test.json`, 'utf8'));
assert.equal(p.detectionConfig.technique, 'regex');
assert.ok(p.matchingRules.regexes.some(r => r.regex === 'AIRS-E2E-[0-9]{6}'));
assert.equal(d.name, 'dlp-test');
assert.ok(Number.isInteger(d.version), 'DLP profile version must be known');
const leaves = [];
function visit(v) {
  if (!v || typeof v !== 'object') return;
  if (v.ruleItem) leaves.push(v.ruleItem);
  Object.values(v).forEach(visit);
}
visit(d.detectionRules);
assert.equal(leaves.length, 1, 'Expected exactly one stored rule leaf');
assert.equal(String(leaves[0].id), String(p.id));
assert.equal(leaves[0].detectionTechnique, 'regex');
assert.equal(leaves[0].confidenceLevel, 'high');
assert.equal(leaves[0].matchType, 'include');
assert.equal(leaves[0].occurrenceOperatorType, 'any');
console.log(`${prefix}: custom DLP rule and pattern verified`);
NODE
}

