# Changelog

All notable changes to the **HyperForm AI** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-06

### Added
- **HyperForm AI Initial Release**: Local-first, intelligent AI development environment (*"Think Beyond. Build Beyond."*).
- **Core Engine & Work Plans**:
  - Process-isolated engine architecture with a single unified permission gate, hooks, and immutable event streams.
  - Structured work plans with task graphs, acceptance criteria verification, and parallel helper dispatch in isolated git worktrees.
  - Shadow git checkpoints providing automatic workspace snapshots and turn-by-turn rewind.
  - Fine-grained permission system with interactive approvals, glob rule matching, and optional sandbox confinement.
- **AI Models & Providers**:
  - Unified multi-provider ModelClient supporting Anthropic (extended thinking & prompt caching), OpenAI, OpenRouter, and GitHub Copilot.
  - Local model execution and calibration via ONNX runtime and sidecar endpoints.
  - Automated cost estimation, fallback chains, and semantic context compaction with repo maps.
- **Desktop Application**:
  - Modern cross-platform GUI built with Tauri and Svelte 5.
  - Real-time streaming transcript with interactive approval cards, tool-run groups, and diff viewers.
  - Multi-tab workspace sidepanel for Changes, Work Plans, Subagents, and Context Inspector.
  - Interactive 3D companion pet rendered with Three.js featuring level growth and activity streaks.
- **Platform Support & Distribution**:
  - Windows x64 and Windows ARM64 (native MSVC bundles and NSIS installers).
  - macOS Apple Silicon (aarch64) and Intel (x86_64) DMG packages.
  - Linux x86_64 AppImage, Debian (.deb), and RPM bundles.
  - Automated release builds, signature verification, and built-in self-updater via GitHub Releases.
