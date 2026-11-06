Applies the changes of a finished agent that ran with `isolation: "worktree"` to the project's working tree.

- `agent_id` is the id reported when the agent finished.
- The agent's changes are merged into the working tree with a three-way merge. If they conflict with the current tree, nothing is changed and the result reports the conflicts.
- Review the agent's report and diff before applying, and verify the result with the project's checks afterwards.
- Agents that worked in the shared tree have nothing to apply.
