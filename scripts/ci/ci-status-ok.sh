#!/usr/bin/env bash
# ci-status-ok.sh < combined-status.json: exit 0 only when a Forgejo combined commit status
# (GET /repos/{owner}/{repo}/commits/{sha}/status) shows every required CI job as passed.
# Each of the jobs linux, musl and docs-site must report under "CI / <job> (push)" or
# "CI / <job> (pull_request)"; at least one of the two must be present, and every present one
# must be `success`. The aggregate state must also be `success`. Missing, pending, failed,
# errored or malformed input exits 1 with the reason on stderr. release.sh uses it.
set -u
command -v jq >/dev/null || { echo "ci-status-ok: jq is required" >&2; exit 1; }
input="$(cat)"
jq -e 'type == "object" and (.statuses | type == "array")' >/dev/null 2>&1 <<<"$input" \
  || { echo "ci-status-ok: not a combined status object" >&2; exit 1; }
problem="$(jq -r '
  def job_ok($job):
    [.statuses[] | select(.context == "CI / \($job) (push)" or .context == "CI / \($job) (pull_request)")]
    as $found
    | if ($found | length) == 0 then "\($job): no status"
      elif ($found | all(.status == "success")) then empty
      else "\($job): " + ([$found[] | "\(.context)=\(.status)"] | join(", "))
      end;
  [ (if .state == "success" then empty else "aggregate state is \(.state // "missing")" end),
    job_ok("linux"), job_ok("musl"), job_ok("docs-site") ]
  | join("; ")' <<<"$input" 2>/dev/null)" || { echo "ci-status-ok: unreadable status" >&2; exit 1; }
if [ -n "$problem" ]; then
  echo "ci-status-ok: $problem" >&2
  exit 1
fi
exit 0
