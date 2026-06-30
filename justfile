dylint_repo := "target/dylint/dylint-v4.1.0"
dylint_tag := "v4.1.0"

check: clippy dylint

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

dylint: dylint-fetch
    env PATH="${DYLINT_PATH:-$HOME/.cargo/bin:$PATH}" DYLINT_RUSTFLAGS="-D warnings" cargo-dylint dylint --path {{dylint_repo}}/examples/general/basic_dead_store --all -- --all-targets
    env PATH="${DYLINT_PATH:-$HOME/.cargo/bin:$PATH}" DYLINT_RUSTFLAGS="-D warnings" cargo-dylint dylint --path {{dylint_repo}}/examples/general/crate_wide_allow --all -- --all-targets
    env PATH="${DYLINT_PATH:-$HOME/.cargo/bin:$PATH}" DYLINT_RUSTFLAGS="-D warnings" cargo-dylint dylint --path {{dylint_repo}}/examples/general/incorrect_matches_operation --all -- --all-targets
    env PATH="${DYLINT_PATH:-$HOME/.cargo/bin:$PATH}" DYLINT_RUSTFLAGS="-D warnings" cargo-dylint dylint --path {{dylint_repo}}/examples/restriction/try_io_result --all -- --all-targets
    env PATH="${DYLINT_PATH:-$HOME/.cargo/bin:$PATH}" DYLINT_RUSTFLAGS="-D warnings" cargo-dylint dylint --path {{dylint_repo}}/examples/supplementary/inconsistent_struct_pattern --all -- --all-targets

dylint-fetch:
    @mkdir -p target/dylint
    @test -d {{dylint_repo}} || git clone --depth 1 --branch {{dylint_tag}} https://github.com/trailofbits/dylint.git {{dylint_repo}}

dylint-install:
    cargo install cargo-dylint --version 4.1.0 --locked
    cargo install dylint-link --version 4.1.0 --locked

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
