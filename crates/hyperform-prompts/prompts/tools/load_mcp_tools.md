Loads the full definitions of MCP tools so you can call them.

This session has more MCP tools than fit in your tool list, so they are listed by name and summary in a system reminder instead. Before you call one of them:

- Pass the exact names from that list (for example `mcp__github__create_issue`) in `names`; load several at once when you expect to need them.
- The loaded tools become callable from your next response on; calling a tool that is not loaded fails as an unknown tool.
- Load only what the task needs: every loaded definition takes context space.
