---
layout: home
title: HyperForm AI

hero:
  name: HyperForm AI
  text: Think Beyond. Build Beyond.
  tagline: An intelligent development environment that helps developers understand, plan, build, modify, and evolve software through AI agents, tools, and controlled execution.
  actions:
    - theme: brand
      text: Get started
      link: /docs/user-guide/01-getting-started
    - theme: alt
      text: How it works
      link: /docs/how-it-works/README
    - theme: alt
      text: Download
      link: https://github.com/arshadbarves/hyperform/releases

features:
  - title: One permission gate
    details: Every tool, hook and check goes through one permission gate. Approve gated actions one by one, or write rules that approve them for you.
    link: /docs/user-guide/03-permissions-and-safety
  - title: Multi-agent orchestration
    details: Built-in and custom subagents run in parallel, nested, in the background or resumed, each with its own model, tools and permission mode. An agent can work in its own git worktree and hand back changes you apply or discard.
    link: /docs/user-guide/04-agents
  - title: Claude Code compatible
    details: Tools, commands, hooks and <code>.claude/</code> folders follow Claude Code, so existing agents, commands, skills and <code>CLAUDE.md</code> files work as-is.
    link: /docs/user-guide/06-commands-and-skills
  - title: Steering and interrupts
    details: Messages you send while the agent works are injected at the next step; nothing is dropped while an approval waits.
    link: /docs/user-guide/02-everyday-use#steer-interrupt-cancel
  - title: Evidence-backed badges
    details: Every turn that changes files is marked Verified, Unverified or Failed from the checks that actually ran.
    link: /docs/user-guide/10-verification
  - title: Durable checkpoints
    details: Code, including shell-made changes, and the conversation can be rewound to any prompt; sessions survive crashes.
    link: /docs/user-guide/13-sessions-and-data#checkpoints-and-rewind
  - title: Models, caching and cost
    details: Native Anthropic caching and extended thinking, OpenAI-compatible providers, fallback models, and per-agent cost tracking.
    link: /docs/user-guide/11-models-providers-and-cost
  - title: Optional sandbox
    details: Confines shell commands to the workspace.
    link: /docs/user-guide/03-permissions-and-safety#the-sandbox
---
