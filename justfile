perf:
    cargo bench --bench physics -- --quick
    cargo bench --bench throughput -- --quick

perf-full:
    cargo bench --bench physics
    cargo bench --bench throughput

perf-build:
    cargo bench --bench physics --no-run
    cargo bench --bench throughput --no-run

perf-open:
    @if [ ! -f target/criterion/report/index.html ]; then \
        echo "target/criterion/report/index.html not found; run 'just perf' or 'just perf-full' first"; \
        exit 1; \
    elif command -v open >/dev/null 2>&1; then \
        open target/criterion/report/index.html; \
    elif command -v xdg-open >/dev/null 2>&1; then \
        xdg-open target/criterion/report/index.html; \
    else \
        echo "open target/criterion/report/index.html"; \
    fi
