The user started a new task in a chat with a coding agent. The state holds the new request and a digest of one earlier exchange: its request, the files it read and edited, the tools and commands it ran, the checks that failed, and the agent's last reply. Does the new task need the full text of this earlier exchange?

- yes: The new task depends on it, for example it refers to that work, continues it, or touches the same code.
- no: The new task does not need it; a one-line mention of it is enough.
