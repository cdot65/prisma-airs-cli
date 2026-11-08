#!/usr/bin/env python3
"""ci-status-ok.py < combined-status.json: exit 0 only when every required CI job passed.

The input is a Forgejo combined commit status (GET /repos/{owner}/{repo}/commits/{sha}/status).
Each of the jobs linux, musl and docs-site must report under "CI / <job> (push)" or
"CI / <job> (pull_request)"; at least one of the two must be present, and every present one must
be "success". The aggregate state must also be "success". release.sh uses this, run as
`python3 -I`.

One strict parser both validates and evaluates, so no input can mean different things to two
tools. Anything else exits 1 with the reason on stderr: bytes that are not UTF-8, anything but
exactly one RFC 8259 JSON document (NaN, Infinity, 01, +1, 1., NUL bytes, trailing data), duplicate
object keys, unpaired surrogate escapes, and any unexpected error.
"""

import json
import sys

REQUIRED_JOBS = ("linux", "musl", "docs-site")


def refuse_constant(name):
    raise ValueError(f"{name} is not JSON")


def refuse_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate key {key!r}")
        result[key] = value
    return result


def refuse_surrogates(value):
    """Decoded strings must be encodable: `\\ud800` alone is not a Unicode string."""
    stack = [value]
    while stack:
        item = stack.pop()
        if isinstance(item, str):
            item.encode("utf-8")
        elif isinstance(item, list):
            stack.extend(item)
        elif isinstance(item, dict):
            stack.extend(item.keys())
            stack.extend(item.values())


def problems(status):
    if not isinstance(status, dict) or not isinstance(status.get("statuses"), list):
        return ["not a combined status object"]
    entries = status["statuses"]
    if not all(isinstance(e, dict) and isinstance(e.get("context"), str) for e in entries):
        return ["a status entry is malformed"]
    found = []
    if status.get("state") != "success":
        found.append(f"aggregate state is {status.get('state')!r}")
    for job in REQUIRED_JOBS:
        names = (f"CI / {job} (push)", f"CI / {job} (pull_request)")
        reported = [e for e in entries if e["context"] in names]
        if not reported:
            found.append(f"{job}: no status")
        for entry in reported:
            if entry.get("status") != "success":
                found.append(f"{entry['context']}={entry.get('status')!r}")
    return found


def main():
    try:
        text = sys.stdin.buffer.read().decode("utf-8", errors="strict")
        status = json.loads(
            text,
            parse_constant=refuse_constant,
            object_pairs_hook=refuse_duplicates,
        )
        refuse_surrogates(status)
        found = problems(status)
    except Exception as error:  # Any doubt refuses the release.
        print(f"ci-status-ok: unreadable status: {error}", file=sys.stderr)
        return 1
    if found:
        print("ci-status-ok: " + "; ".join(found), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
