#!/usr/bin/env python3
"""Minimal third-party MCP stdio server exposing one `echo` tool."""
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

TOOLS = [{
    "name": "echo",
    "description": "Echoes back its input text prefixed with PONG:",
    "inputSchema": {
        "type": "object",
        "properties": {"text": {"type": "string"}},
        "required": ["text"],
    },
}]

def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        method = msg.get("method")
        if "id" in msg and method == "initialize":
            send({"jsonrpc": "2.0", "id": msg["id"],
                  "result": {"protocolVersion": "2024-11-05",
                             "capabilities": {"tools": {}},
                             "serverInfo": {"name": "echo-server", "version": "0.1"}}})
        elif "id" in msg and method == "tools/list":
            send({"jsonrpc": "2.0", "id": msg["id"], "result": {"tools": TOOLS}})
        elif "id" in msg and method == "tools/call":
            args = msg["params"].get("arguments", {})
            text = args.get("text", "")
            send({"jsonrpc": "2.0", "id": msg["id"],
                  "result": {"content": [{"type": "text", "text": f"PONG:{text}"}]}})
        elif "id" not in msg:
            pass  # notifications (initialized etc.) need no reply

if __name__ == "__main__":
    main()
