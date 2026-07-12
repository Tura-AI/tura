# DeepSWE

Source: https://www.everydev.ai/tools/deepswe

# DeepSWE

> A benchmark for measuring frontier coding agents on 113 original, long-horizon software engineering tasks drawn from active open-source repositories across 5 languages.

DeepSWE is a benchmark created by Datacurve AI to evaluate frontier coding agents on original, long-horizon software engineering tasks. It covers 113 tasks across TypeScript, Go, Python, JavaScript, and Rust, drawn from 91 active open-source repositories, with isolated environments and program-based verifiers. The project is hosted on GitHub and pairs with Pier, a sandboxed coding-agent evaluation framework.

## What It Is

DeepSWE is a software engineering benchmark designed to differentiate the performance of today's top AI coding agents in scenarios where existing public benchmarks have begun to saturate. Rather than adapting tasks from existing commits or pull requests, DeepSWE tasks are written from scratch to avoid contamination from model pretraining data. Each task includes a structured format with metadata, an agent-facing instruction, a reproducible Docker environment, a hand-written verifier, and a held-out reference solution for human and AI reviewers.

## What Separates It From Other Benchmarks

The DeepSWE project page describes four advances over existing public benchmarks:

- **Contamination-free**: Tasks are written from scratch, not adapted from existing commits or PRs.
- **High diversity**: Tasks span 91 repositories across 5 programming languages.
- **Real-world complexity**: The project page states that prompts are roughly half the length of SWE-bench Pro's, yet solutions require 5.5x more code and approximately 2x more output tokens.
- **Reliable verification**: Verifiers are hand-written to test software behavior rather than implementation details, accepting any solution with correct observable behavior regardless of internal symbol names or structure.

## Leaderboard and Results

DeepSWE publishes a public leaderboard showing scores for 12 frontier models, all evaluated using mini-swe-agent. According to the leaderboard, GPT-5.5 leads at 70%±4%, followed by GPT-5.4 at 56%±5% and Claude Opus 4.7 at 54%±5%. Lower-ranked models include Gemini 3.5 Flash at 28%±4% and DeepSeek V4 Pro at 8%±2%. All scores were produced with Pier running mini-swe-agent on Modal.

## Task Format and Evaluation Harness

Tasks use the Harbor task format, with each task directory containing a `task.toml` for metadata, an `instruction.md` for the agent prompt, an `environment/` Dockerfile, a `tests/` verifier, and a `solution/` reference patch. The evaluation harness, Pier, is a Harbor-compatible framework that adds per-agent network allowlists for air-gapped tasks, more complete trajectory metadata, a trajectory viewer, and a `pier critique run` command for analyzing agent trajectories. Pier supports running agents including mini-swe-agent, claude-code, codex, gemini-cli, and opencode, and can parallelize runs on Modal.

## Current Status

The repository was created in May 2026 and last updated in late May 2026, indicating it is a recently launched and actively maintained project. It has 71 stars and 2 forks on GitHub. No license is specified in the repository at this time.

## Features
- 113 original long-horizon software engineering tasks
- Tasks span TypeScript, Go, Python, JavaScript, and Rust
- 91 open-source repositories covered
- Contamination-free task design (written from scratch)
- Hand-written program-based verifiers testing behavior not implementation
- Isolated Docker environments per task
- Held-out reference solutions for reviewers
- Public leaderboard with confidence intervals
- Pier evaluation harness with per-agent network allowlists
- Support for mini-swe-agent, claude-code, codex, gemini-cli, opencode
- Parallel sandbox execution on Modal
- Trajectory metadata and viewer
- pier critique run for agent trajectory analysis
- Deterministic random subset sampling for partial runs

## Integrations
Anthropic Claude, OpenAI GPT, Google Gemini, Moonshot Kimi, DeepSeek, mini-swe-agent, claude-code, codex, gemini-cli, opencode, Modal, Harbor framework, Pier

## Platforms
CLI, API

## Pricing
Open Source

## Version
main

## Links
- Website: https://deepswe.datacurve.ai/
- Documentation: https://deepswe.datacurve.ai/blog
- Repository: https://github.com/datacurve-ai/deep-swe
- EveryDev.ai: https://www.everydev.ai/tools/deepswe
