The context of a coding agent is filling up, so older tool output may be cleared; the full output stays in a file the agent can read again. The state holds the user's latest request, the open todo items, and one earlier tool call: the tool, a digest of its input, and the start of its output. Is this output still needed for the request?

- yes: The output still matters for the request or the open todos, for example code the agent is changing, an error it is fixing, or facts it must report.
- no: The output served an earlier step or another task and can be cleared; the agent can read it again if needed.
