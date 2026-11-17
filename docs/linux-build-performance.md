# Linux build continuity and cache

Push workflows use a per-commit concurrency group with cancellation disabled.
Both parts matter: disabling active cancellation alone still lets a newer push
replace an older pending run in a shared branch group. Each main commit now
retains its own run. All jobs remain self-hosted, Linux x64.

The portable Ubuntu 22.04 builder image is keyed by the Dockerfile hash. System
packages and Rust 1.98.0 install once per recipe instead of once per push.
Docker named volumes retain Cargo registry, Git dependencies and target output.
A content-hash manifest restores mtimes only for byte-identical source files,
so git archive timestamps do not force unchanged Rust crates to recompile.
A host flock serializes writers to those caches. Source still enters from
`git archive HEAD`, with no host checkout or secret directories mounted.
No cache pruning or new runners are introduced. The 25 GiB disk floor remains.
Documentation-only pushes skip the binary artifact workflow, but not CI.
CI uses the fixed toolchain rather than updating to a changing stable version.

The script prints separate builder-setup, compile/verify/copy and total seconds.
Before baseline: workflow run 43847275604 on 2026-11-16 took 12m43s in the
portable-build step, 13m24s for the job. Run 44038681983 never received a runner
and had no executed steps before cancellation; its 49m21s was queue time, not
compile time. Do not compare that queue wait to a compile duration.
After timings must come from an executed new run. An offline runner cannot
provide a measured speedup; the first new run also has a cold cache.
