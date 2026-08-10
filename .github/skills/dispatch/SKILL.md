---
name: dispatch
description: Dispatch tasks to high-performance subagents. Use when asked to investigate, implement, audit, or delegate work to subagents.
---

# Dispatch

How to delegate tasks to frontier subagents.

## Defaults

- **Investigate / audit / research:** dispatch two agents in parallel — one Claude (opus series, highest available) and one OpenAI (GPT sol series, highest available). Read-only — forbid edits.
- **Implement / fix / build:** dispatch one Claude agent (opus series). Allow edits, forbid commits.

Always use `long_context` context tier and the highest reasoning effort available for the model.

## Seed prompt rules

1. Explain enough context that the agent can work without asking.
2. State the user's exact instructions — what to do, what to report.
3. If the agent needs prior conversation context, summarize it in the prompt — agents are stateless.

## Model selection

Pick the largest number in each series. Examples:
- Anthropic: `claude-opus-5`, `claude-opus-4.8`, `claude-opus-4.7`
- OpenAI: `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.5`

These are examples — versions change. If unsure which models are available, look up the available models list or check the Copilot CLI documentation.

## Agent type

- Needs to search GitHub / web / external repos → `research`
- Needs to read, grep, run commands in this repo → `general-purpose`
- Needs to critique a plan or implementation → `rubber-duck`

## Constraints

- Investigating agents must not edit files or run destructive commands.
- Implementing agents must not commit. The user reviews and commits.
- Always use `mode: "background"` and report results when agents complete.
