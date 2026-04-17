perf:
    cargo bench --bench physics -- --quick
    cargo bench --bench throughput -- --quick

perf-full:
    cargo bench --bench physics
    cargo bench --bench throughput

perf-build:
    cargo bench --bench physics --no-run
    cargo bench --bench throughput --no-run
