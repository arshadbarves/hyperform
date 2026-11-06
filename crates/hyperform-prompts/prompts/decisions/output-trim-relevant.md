A coding agent ran a command whose output is long. The input names the user's current request, the command, and one part of its output. Does this part matter for the request, for example an error, a failing test, a warning about the code being changed, or a result the agent must report?

- yes: This part matters for the request; keep it.
- no: This part is routine output, such as progress lines, passing tests or download logs; it can be left out.
