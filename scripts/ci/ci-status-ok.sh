#!/usr/bin/env bash
# ci-status-ok.sh < combined-status.json: exit 0 only when a Forgejo combined commit status
# (GET /repos/{owner}/{repo}/commits/{sha}/status) shows every required CI job as passed.
# Each of the jobs linux, musl and docs-site must report under "CI / <job> (push)" or
# "CI / <job> (pull_request)"; at least one of the two must be present, and every present one
# must be `success`. The aggregate state must also be `success`. Missing, pending, failed,
# errored or malformed input exits 1 with the reason on stderr. release.sh uses it.
set -u
command -v jq >/dev/null || { echo "ci-status-ok: jq is required" >&2; exit 1; }
input="$(cat)" || { echo "ci-status-ok: cannot read the status" >&2; exit 1; }
# Every verdict is an explicit word from jq: on empty input jq 1.6 prints nothing and exits 0
# even with -e, so neither no output nor a zero exit may ever count as a pass. jq can also print
# a verdict and then fail on trailing garbage, so its exit status must be zero as well.
shape="$(jq -r 'if type == "object" and (.statuses | type == "array") then "combined" else "other" end' <<<"$input" 2>/dev/null)" || shape=unreadable
[ "$shape" = combined ] || { echo "ci-status-ok: not a combined status object" >&2; exit 1; }
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
  | if length == 0 then "all-passed" else join("; ") end' <<<"$input" 2>/dev/null)" || problem="unreadable status"
if [ "$problem" = all-passed ]; then
  exit 0
fi
echo "ci-status-ok: ${problem:-unreadable status}" >&2
exit 1
