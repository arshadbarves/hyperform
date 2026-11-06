Launches a subagent: a separate agent run with its own context and tools that carries out one task and reports back.

Available agent types for `subagent_type`:
{{agents}}

When to delegate:
- Broad research that would take many searches or flood your context with intermediate output. An `explore` agent returns only its findings.
- Independent pieces of work that can run in parallel.
- Implementation that should stay out of the user's tree until it is reviewed (see `isolation`).
- An independent check: a `review` agent after substantial changes, or a `verify` agent to run a slow suite in the background.

Do the work yourself when one or two direct calls answer the question (a known file or symbol), when it depends on conversation details that are hard to pass on, or when the edit is small.

Writing the prompt:
- The agent cannot see this conversation. Make `prompt` self-contained: the goal, what you already know (paths, symbols, findings), the constraints, whether to change files or only research, and exactly what to return.
- For `explore`, say how thorough to be: quick, medium, or very thorough.
- `description` is a short label (3-5 words) that names the agent in the agent tree.

Running agents:
- Launch several agents in the same message to run them in parallel.
- By default the call waits and returns the agent's final report. With `run_in_background: true` it returns a job id at once; you are notified when the agent finishes and read its report with JobOutput. Use it when you have other work to do meanwhile.
- `resume` takes the `agent_id` of an earlier agent and continues it with its full context; `prompt` is then your follow-up message to it.
- `isolation: "worktree"` runs the agent in its own git worktree on a separate branch, for independent implementation work. Its changes reach the project only when you apply them with ApplyAgentChanges (after reviewing its report and diff) or when the user applies them. The default, `"shared"`, works in the project tree.

Using the result:
- The user does not see the agent's report. Relay what matters in your reply.
- A report is a claim, not evidence. Check anything critical before relying on it, for example by reading the cited code or running the check again.
