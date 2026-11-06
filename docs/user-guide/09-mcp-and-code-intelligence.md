# MCP and code intelligence

HyperForm can talk to two kinds of helper programs. **MCP servers** (Model
Context Protocol) give the agent extra tools and data, such as GitHub
issues, a database or a browser. **Language servers** (LSP) give it exact
code intelligence: go to definition, find references, and compile errors
right after an edit.

## Connect an MCP server

An MCP server either runs as a local program that HyperForm starts
(*stdio*) or is reached over the network at a URL (*HTTP*, the streamable
HTTP transport).

### From Settings

1. Open **Settings → MCP** and choose the settings file under **Saving
   to** at the top of the page (**User** for all projects, **This
   project**, or **Personal (local)**).
2. Click **Add server**.
3. Enter a **Name** (it becomes part of the tool names) and choose
   **Command (stdio)** or **URL (HTTP)**.
4. Fill in the command and arguments, or the URL and headers.
5. Optionally list **Hidden tools** and change the **Timeout**.
6. Click **Save server**, then **Test** on its row. The test connects,
   lists the server's tools, resources and prompts, and disconnects.

Servers from other settings files are listed read-only; copy one to
override it. The toggle on each row enables or disables a server.

### In a settings file

```toml
# A local server started by HyperForm
[mcp.servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

# A local server that needs a token
[mcp.servers.github]
command = "github-mcp-server"
args = ["stdio"]
env = { GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_..." }
disabled_tools = ["delete_repository"]
timeout_secs = 30

# A remote server over HTTP
[mcp.servers.docs]
url = "https://mcp.example.com/mcp"
headers = { Authorization = "Bearer sk-..." }
```

| Field | Meaning |
|---|---|
| `command`, `args` | The program to start and its arguments (stdio). The program is looked up on `PATH` unless it is a path. |
| `env` | Environment variables for the program. |
| `cwd` | Working folder, relative to the project; default: the project folder. `~` means your home folder. |
| `url` | The endpoint of an HTTP server. |
| `headers` | HTTP headers, for example `Authorization`. |
| `enabled` | `false` keeps the server configured but off. Default `true`. |
| `disabled_tools` | Tool names hidden from the agent. |
| `timeout_secs` | Seconds to wait for the server. Default 60. |

Set exactly one of `command` and `url`; an entry with both or neither is
skipped with a warning. A server in a higher settings file replaces a
server of the same name entirely.

> **Warning:** Settings files are plain text. Put tokens in your user
> settings or in `.hyperform/settings.local.toml`, never in the shared
> `.hyperform/settings.toml`.

> **Note:** A stdio server does not get your full environment. It gets
> basics (such as `PATH` and `HOME`), common toolchain and proxy variables
> (for example `JAVA_HOME`, `VIRTUAL_ENV`, `HTTPS_PROXY`), and whatever you
> put in `env`. Pass tokens through `env`.

Project-defined servers start only in [trusted workspaces](03-permissions-and-safety.md#workspace-trust).

## How MCP servers appear

When a chat opens, its servers start in the background. A notification
reports which are ready (with their tool counts) and which failed. `/mcp`
shows each server's state (connecting, ready, failed, disabled), its
tools, resources and prompts, and the error and last lines of error output
for a failed server.

- **Tools** appear to the agent as `mcp__<server>__<tool>`, for example
  `mcp__github__create_issue`. Characters other than letters, digits, `_`
  and `-` become `_`. If a server announces that its tools changed, the list
  is refreshed.
- **Permissions:** MCP tools ask for approval by default, even read-only
  ones; in plan mode, tools that change things are refused. Allow them with
  rules such as `mcp__github__get_issue` or `mcp__github` (every tool of
  that server). See [rule syntax](03-permissions-and-safety.md#rule-syntax).
- **Resources** (files, schemas, documents a server exposes) are read with
  the `ListMcpResources` and `ReadMcpResource` tools, available while a
  server is ready.
- **Prompts** become slash commands: `/mcp__<server>__<prompt>` (see
  [Commands and skills](06-commands-and-skills.md#mcp-prompts-as-commands)).

### Large tool sets

Tool definitions take up context. When all servers together offer more
than 40 tools, HyperForm sends only their names and one-line summaries. The
agent then loads the definitions it needs with the `LoadMcpTools` tool;
loaded tools can be called from its next response on. Hiding tools you
never use (`disabled_tools`) keeps the list short.

## Language servers

A language server understands one programming language the way your
editor does. HyperForm starts one automatically when it is installed and
the agent works on a matching file.

| Server | Program | Files |
|---|---|---|
| rust-analyzer | `rust-analyzer` | `.rs` |
| typescript-language-server | `typescript-language-server --stdio` | `.ts .tsx .js .jsx .mjs .cjs .mts .cts` |
| pyright | `pyright-langserver --stdio` | `.py` |
| basedpyright | `basedpyright-langserver --stdio` | `.py` (used if pyright isn't installed) |
| gopls | `gopls` | `.go` |
| clangd | `clangd` | `.c .h .cc .cpp .cxx .hpp` |

The programs must be on your `PATH`; HyperForm doesn't install them.

- **Automatic start:** servers start the first time the agent asks about a
  file of their language, or in the background after it writes one.
- **The `LSP` tool** lets the agent ask for: `definition`, `references`,
  `hover`, `implementations`, `incomingCalls`, `outgoingCalls`,
  `documentSymbols`, `workspaceSymbols`, `diagnostics` (errors and
  warnings for a file or the whole project) and `renamePreview` (the edits
  a rename would make; nothing is changed). It is a reading tool, so
  `Read(...)` rules apply.
- **Errors after edits:** when the agent edits a file whose language server
  is already running, HyperForm waits up to two seconds for fresh
  diagnostics and appends any **errors** (up to 20 per file) to the edit's
  result, so the agent can fix them right away. Warnings are not added.
- `/doctor` lists the configured servers and whether each is starting,
  running or failed.

### Configure language servers

In **Settings → Advanced → Language servers**, or:

```toml
[lsp]
enabled = true

# Add a server for another language
[lsp.servers.ruby]
command = "ruby-lsp"
extensions = ["rb"]
root_markers = ["Gemfile"]

# Replace a built-in server by using its name
[lsp.servers.rust-analyzer]
command = "/opt/tools/rust-analyzer"
extensions = ["rs"]
root_markers = ["Cargo.toml"]

# Turn a built-in server off
[lsp.servers.clangd]
enabled = false
```

| Field | Meaning |
|---|---|
| `command`, `args` | The program and its arguments. `command` is required for a server that is on; an enabled entry without it is skipped with a warning. |
| `extensions` | File extensions without the dot. |
| `root_markers` | Files that mark the root of a project for this server, such as `Cargo.toml`. |
| `enabled` | `false` turns the server off (also removes a built-in of that name). |

Servers you add take precedence for the extensions they claim. Set
`lsp.enabled = false` to turn off all language servers; the `LSP` tool then
disappears.

See also: [Permissions and safety](03-permissions-and-safety.md) · [Commands and skills](06-commands-and-skills.md) · [Troubleshooting](14-troubleshooting.md)
