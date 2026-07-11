#!/usr/bin/env python3
"""Measure the public NumPy/PyO3 batch boundary with prepacked inputs."""

from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import sys
import time
from typing import Any

EXPECTED_KEYS = {
    "elapsed_seconds",
    "cue_pocketed",
    "nine_pocketed",
    "legal_nine_pocketed",
    "first_cue_contact",
    "lowest_object_ball",
    "first_contact_lowest_object_ball",
    "event_count",
    "pocketed_mask",
    "final_state",
    "final_x",
    "final_y",
    "final_pocket",
}
MATRIX_KEYS = {"pocketed_mask", "final_state", "final_x", "final_y", "final_pocket"}
ABSENT_BALL_ID = 255


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch-sizes", type=int, nargs="+", default=[1, 8, 32, 128, 512])
    parser.add_argument(
        "--workloads",
        choices=("boundary", "mixed"),
        nargs="+",
        default=["boundary", "mixed"],
    )
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--warmup-calls", type=int, default=3)
    parser.add_argument("--minimum-sample-seconds", type=float, default=0.5)
    parser.add_argument("--threads", type=int)
    return parser.parse_args()


def build_inputs(np: Any, batch_size: int, workload: str) -> tuple[Any, Any, Any, Any]:
    max_balls = 10
    ball_ids = np.full((batch_size, max_balls), ABSENT_BALL_ID, dtype=np.uint8)
    ball_xs = np.zeros((batch_size, max_balls), dtype=np.float64)
    ball_ys = np.zeros((batch_size, max_balls), dtype=np.float64)
    shots = np.zeros((batch_size, 4), dtype=np.float64)

    for row in range(batch_size):
        ball_ids[row, 0] = 0
        ball_xs[row, 0] = 20.0 + (row % 4)
        ball_ys[row, 0] = 20.0

        if workload == "boundary":
            shots[row, 0] = float((row * 17) % 360)
            shots[row, 1] = 0.0
            continue

        object_count = 1 + row % 9
        for object_index in range(object_count):
            slot = object_index + 1
            ball_ids[row, slot] = slot
            ball_xs[row, slot] = 32.0 + 3.0 * (object_index % 3)
            ball_ys[row, slot] = 32.0 + 3.0 * (object_index // 3)
        shots[row, 0] = 35.0 + float(row % 5)
        shots[row, 1] = 24.0 + float(row % 4) * 8.0
        shots[row, 2] = (-0.20, 0.0, 0.20)[row % 3]
        shots[row, 3] = 0.10

    return ball_ids, ball_xs, ball_ys, shots


def validate_output(np: Any, output: dict[str, Any], batch_size: int) -> None:
    if set(output) != EXPECTED_KEYS:
        raise AssertionError(f"unexpected output keys: {sorted(output)}")
    for key, value in output.items():
        array = np.asarray(value)
        expected_shape = (batch_size, 10) if key in MATRIX_KEYS else (batch_size,)
        if array.shape != expected_shape:
            raise AssertionError(f"{key} has shape {array.shape}, expected {expected_shape}")
        if not array.flags.c_contiguous:
            raise AssertionError(f"{key} is not C-contiguous")


def median_absolute_deviation(values: list[float]) -> float:
    center = statistics.median(values)
    return statistics.median(abs(value - center) for value in values)


def measure_case(
    np: Any,
    simulate_shots_batch: Any,
    batch_size: int,
    workload: str,
    samples: int,
    warmup_calls: int,
    minimum_sample_seconds: float,
) -> dict[str, Any]:
    inputs = build_inputs(np, batch_size, workload)
    output = simulate_shots_batch(*inputs)
    validate_output(np, output, batch_size)

    for _ in range(warmup_calls):
        output = simulate_shots_batch(*inputs)

    started = time.perf_counter_ns()
    output = simulate_shots_batch(*inputs)
    one_call_seconds = max((time.perf_counter_ns() - started) / 1e9, 1e-9)
    iterations = max(1, int(minimum_sample_seconds / one_call_seconds))

    nanoseconds_per_call: list[float] = []
    checksum = 0
    for _ in range(samples):
        started = time.perf_counter_ns()
        for _ in range(iterations):
            output = simulate_shots_batch(*inputs)
        elapsed_ns = time.perf_counter_ns() - started
        checksum ^= int(np.asarray(output["event_count"], dtype=np.int64).sum())
        nanoseconds_per_call.append(elapsed_ns / iterations)

    median_ns = statistics.median(nanoseconds_per_call)
    return {
        "workload": workload,
        "batch_size": batch_size,
        "iterations_per_sample": iterations,
        "samples": samples,
        "median_nanoseconds_per_call": median_ns,
        "mad_nanoseconds_per_call": median_absolute_deviation(nanoseconds_per_call),
        "median_shots_per_second": batch_size * 1e9 / median_ns,
        "checksum": checksum,
        "sample_nanoseconds_per_call": nanoseconds_per_call,
    }


def main() -> int:
    args = parse_args()
    if args.threads is not None:
        if args.threads <= 0:
            raise SystemExit("--threads must be positive")
        os.environ["RAYON_NUM_THREADS"] = str(args.threads)

    import numpy as np

    from billiards_gymnasium import simulate_shots_batch

    if args.samples <= 0 or args.warmup_calls < 0 or args.minimum_sample_seconds <= 0.0:
        raise SystemExit("samples and minimum sample duration must be positive")
    if any(batch_size <= 0 for batch_size in args.batch_sizes):
        raise SystemExit("batch sizes must be positive")

    results = []
    for workload in args.workloads:
        for batch_size in args.batch_sizes:
            results.append(
                measure_case(
                    np,
                    simulate_shots_batch,
                    batch_size,
                    workload,
                    args.samples,
                    args.warmup_calls,
                    args.minimum_sample_seconds,
                )
            )

    print(
        json.dumps(
            {
                "python": sys.version,
                "numpy": np.__version__,
                "platform": platform.platform(),
                "rayon_num_threads": os.environ.get("RAYON_NUM_THREADS"),
                "results": results,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
