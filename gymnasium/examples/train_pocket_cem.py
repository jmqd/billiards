#!/usr/bin/env python3
"""Tiny dependency-light RL baseline for BilliardsPocketBall-v0.

This trains a two-output linear policy with the Cross-Entropy Method (CEM). It is intentionally
small and NumPy-only so the initial RL workflow does not require torch/stable-baselines3. The policy
is not meant to be state of the art; it is a smoke-testable baseline for training against the PyO3
Gymnasium environment.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

import gymnasium as gym
import numpy as np

import billiards_gymnasium  # noqa: F401 - registers Gymnasium envs
from billiards_gymnasium.spaces import BALL_INDEX


@dataclass(frozen=True)
class CemConfig:
    iterations: int
    population: int
    elite_fraction: float
    episodes_per_candidate: int
    seed: int
    theta_std: float
    min_std: float


def sigmoid(x: np.ndarray) -> np.ndarray:
    return 1.0 / (1.0 + np.exp(-np.clip(x, -40.0, 40.0)))


def logit(probability: float) -> float:
    probability = float(np.clip(probability, 1e-6, 1.0 - 1e-6))
    return float(np.log(probability / (1.0 - probability)))


def policy_features(obs: np.ndarray, target_ball: str) -> np.ndarray:
    cue = obs[BALL_INDEX["cue"]]
    target = obs[BALL_INDEX[target_ball]]
    dx = target[1] - cue[1]
    dy = target[2] - cue[2]
    distance = float(np.hypot(dx, dy))
    return np.array(
        [
            1.0,
            cue[1],
            cue[2],
            target[1],
            target[2],
            dx,
            dy,
            distance,
        ],
        dtype=np.float64,
    )


def action_from_theta(theta: np.ndarray, obs: np.ndarray, target_ball: str) -> np.ndarray:
    features = policy_features(obs, target_ball)
    return sigmoid(theta.reshape(2, features.size) @ features).astype(np.float32)


def evaluate_theta(
    env: gym.Env,
    theta: np.ndarray,
    *,
    target_ball: str,
    episodes: int,
    rng: np.random.Generator,
) -> float:
    rewards = []
    for _ in range(episodes):
        obs, _ = env.reset(seed=int(rng.integers(0, 2**32 - 1)))
        action = action_from_theta(theta, obs, target_ball)
        _, reward, _, _, _ = env.step(action)
        rewards.append(float(reward))
    return float(np.mean(rewards))


def train_cem(env: gym.Env, *, target_ball: str, config: CemConfig) -> tuple[np.ndarray, float]:
    rng = np.random.default_rng(config.seed)
    feature_count = policy_features(env.reset(seed=config.seed)[0], target_ball).size
    theta_size = 2 * feature_count
    mean = np.zeros(theta_size, dtype=np.float64)
    # Useful default for side-pocket curricula: rightward heading, medium speed. The policy can
    # still move anywhere in action space; this just gives sparse-reward CEM a sane anchor.
    mean[0] = logit(0.25)
    mean[feature_count] = logit(0.5)
    std = np.full(theta_size, config.theta_std, dtype=np.float64)
    elite_count = max(1, int(round(config.population * config.elite_fraction)))
    best_theta = mean.copy()
    best_score = -np.inf

    for iteration in range(1, config.iterations + 1):
        population = rng.normal(mean, std, size=(config.population, theta_size))
        population[0] = mean
        scores = np.array(
            [
                evaluate_theta(
                    env,
                    theta,
                    target_ball=target_ball,
                    episodes=config.episodes_per_candidate,
                    rng=rng,
                )
                for theta in population
            ],
            dtype=np.float64,
        )
        elite_indices = np.argsort(scores)[-elite_count:]
        elites = population[elite_indices]
        mean = elites.mean(axis=0)
        std = np.maximum(elites.std(axis=0), config.min_std)

        iteration_best_index = int(np.argmax(scores))
        if scores[iteration_best_index] > best_score:
            best_score = float(scores[iteration_best_index])
            best_theta = population[iteration_best_index].copy()

        print(
            f"iter={iteration:03d} best={scores.max():.3f} "
            f"mean={scores.mean():.3f} elite_mean={scores[elite_indices].mean():.3f}"
        )

    return best_theta, best_score


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--layout", default="random_direct_side_pocket")
    parser.add_argument("--target-ball", default="one")
    parser.add_argument("--iterations", type=int, default=20)
    parser.add_argument("--population", type=int, default=64)
    parser.add_argument("--elite-fraction", type=float, default=0.2)
    parser.add_argument("--episodes-per-candidate", type=int, default=4)
    parser.add_argument("--eval-episodes", type=int, default=50)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--theta-std", type=float, default=4.0)
    parser.add_argument("--min-std", type=float, default=0.05)
    parser.add_argument("--output", type=Path, default=Path("pocket_cem_policy.npz"))
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    env = gym.make(
        "BilliardsPocketBall-v0",
        layout=args.layout,
        target_ball=args.target_ball,
    )
    config = CemConfig(
        iterations=args.iterations,
        population=args.population,
        elite_fraction=args.elite_fraction,
        episodes_per_candidate=args.episodes_per_candidate,
        seed=args.seed,
        theta_std=args.theta_std,
        min_std=args.min_std,
    )
    theta, train_score = train_cem(env, target_ball=args.target_ball, config=config)
    eval_rng = np.random.default_rng(args.seed + 10_000)
    eval_score = evaluate_theta(
        env,
        theta,
        target_ball=args.target_ball,
        episodes=args.eval_episodes,
        rng=eval_rng,
    )

    np.savez(
        args.output,
        theta=theta,
        train_score=np.array(train_score),
        eval_score=np.array(eval_score),
        target_ball=np.array(args.target_ball),
        layout=np.array(args.layout),
    )
    print(f"saved {args.output}")
    print(f"best_train_score={train_score:.3f} eval_score={eval_score:.3f}")


if __name__ == "__main__":
    main()
