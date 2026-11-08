---
title: Performance
sidebar_position: 2
---

# Performance

Speed and footprint are why the TypeScript Prisma AIRS CLI was rewritten in Rust. Every run
of the TypeScript CLI first starts Node.js, then loads and compiles its JavaScript and 65
production packages, before it reads its first argument. `airs` is one native executable
with no runtime: it starts in about 6 ms.

## Speed

`airs` 8.0.0 against the TypeScript CLI 7.3.0, measured on 2026-10-23 on an Apple M5 Pro.
Each figure is the median of 30 runs against a loopback fixture server, so it measures the
CLI's own work: process start, argument parsing, tenant loading, the HTTP client, JSON and
output.

| Workload | `airs` 8.0.0 | TypeScript 7.3.0 | Faster |
|---|---:|---:|---:|
| Start-up (`airs --version`) | 6.3 ms | 80.6 ms | **13x** |
| Local commands (`--help`, `tenant list`, `tenant read`) | 8.7–8.9 ms | 82.4–85.9 ms | **9.3–9.9x** |
| One API call (a scan, one profiles page) | 17.5–17.7 ms | 96.8–98.9 ms | **5.5–5.6x** |
| `doctor` | 18.5 ms | 117 ms | **6.3x** |
| Paging 2,000 profiles or topics (`--all`) | 29.8–44.4 ms | 118–130 ms | **2.9–4.0x** |
| 50 scans in a row | 0.85 s | 4.91 s | **5.8x** |

With Node's compile cache (`NODE_COMPILE_CACHE`) the TypeScript CLI is 10–12 ms faster per
run, and `airs` is still 2.7–11x faster.

## Resources

| Per command | `airs` 8.0.0 | TypeScript 7.3.0 | Less |
|---|---:|---:|---:|
| CPU time (user + system) | 3.5–34 ms | 95–207 ms | **6.1–27x** |
| Peak memory (RSS) | 6.5–72 MiB | 106–174 MiB | **2.0–16x** |

## Footprint

| | `airs` 8.0.0 | TypeScript 7.3.0 |
|---|---:|---:|
| Program | one executable | 0.88 MiB of JavaScript in 5 files |
| Runtime | none | Node.js (the `node` binary is 116 MiB) |
| Dependencies | compiled in | 65 npm packages |
| Installed on disk (macOS arm64) | **31.1 MiB** | **219.6 MiB** (103 MiB in 3,104 files, plus Node.js) |

The executables of 8.0.0:

| Platform | Binary | Release tarball |
|---|---:|---:|
| macOS arm64 (signed and notarised) | 31.07 MiB | 12.96 MiB |
| Linux x86_64 (static musl) | 38.65 MiB | 14.49 MiB |
| Linux arm64 (static musl) | 30.63 MiB | 13.33 MiB |

The Linux builds are fully static; the macOS build links only system libraries. The npm
channel installs the same binary as a platform package (13.4–14.6 MiB) behind an 8 KiB
launcher, which is the only part that needs Node.js.

### How the executable grew

The binary grew with each command group ported from the TypeScript CLI:

| Release | Added | macOS binary |
|---|---|---:|
| 0.1.0 | tenants, `doctor`, profile and topic reads, scans (10 commands) | 9.63 MiB |
| 0.5.0 | tenant and runtime writes, bulk scanning (0.3–0.5) | 11.25 MiB |
| 0.6.0 | Red Team | 15.79 MiB |
| 0.7.0 | DLP | 17.94 MiB |
| 0.8.0 | Model Security | 19.52 MiB |
| 0.9.0 | runtime extras, the HTML reports | 21.39 MiB |
| 0.10.0 | AI Gateway | 30.47 MiB |
| 0.11.0 | Supplychain | 30.82 MiB |
| 8.0.0 | TypeScript 7.3.0's directional profiles (277 commands) | 31.07 MiB |

The growth has a cost: against 0.1.0, a parsed command takes about 2.5 ms longer and an API
command about 4 ms longer. Every factor above already includes it.

## On a real tenant

A management read on a real tenant spends 0.7–0.8 s waiting for the service, so the 80 ms
that `airs` saves per command appears there as a constant, not a multiple (1.0–1.2x in the
read-only live runs). The factors apply in full to local commands, to scripts and agents that
run the CLI many times, and to the CPU and memory a host spends on each run.

## Method and raw data

The benchmark, its method, the environment, the per-run spread, the caveats and the live
results are in
[docs/BENCHMARKS.md](https://github.com/cdot65/prisma-airs-cli/blob/main/docs/BENCHMARKS.md).
The benchmark harness that compared both CLIs was removed with the TypeScript parity
tests on 2026-11-08; the figures above are the last measurement (2026-10-23).
