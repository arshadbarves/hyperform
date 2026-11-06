# Models, providers and cost

This page explains how HyperForm reaches an AI model: providers and API keys,
the three model roles, choosing and switching models, reasoning effort,
fallback models, the context window, and how cost is tracked.

## Providers and API keys

A *provider* is the service that runs the model. HyperForm speaks two wire
formats:

- **Anthropic** (the Messages API), used automatically for hosts on
  `anthropic.com`, with native prompt caching and extended thinking;
- **OpenAI-compatible** chat completions, used for everything else:
  OpenRouter, OpenAI, OpenCode Zen, Google Gemini, DeepSeek, Groq, Mistral,
  Ollama, LM Studio and custom servers.

GitHub Copilot is the exception: one sign-in reaches models that Copilot
serves in either format, and HyperForm picks the right one per model
([GitHub Copilot](#github-copilot)).

On a fresh install, the setup guide's model step connects one for you:
**Start free** (OpenCode Zen's free models, no key), **Use my API key**, or
**Run on this computer** (Ollama or LM Studio); see
[First launch](01-getting-started.md#first-launch). Later, connect a
provider in **Settings → Providers** (see
[Getting started](01-getting-started.md#connect-a-model-provider)). This
writes `provider.kind`, `provider.base_url` and `model.main` to the
settings file chosen under **Saving to** at the top of the page. The
default is OpenRouter (`https://openrouter.ai/api/v1`) with
`anthropic/claude-sonnet-4.5`.

```toml
[provider]
kind = "auto"                      # auto | openai_chat | anthropic
base_url = "https://api.anthropic.com"
headers = { "X-Team" = "platform" } # extra HTTP headers on every request
# cache_control = true             # unset: on for Anthropic and OpenRouter
```

**Settings → Providers** shows the default model and its endpoint under
**In use**, with whether a key is stored. When the provider needs a key and
has none, the project home's setup card lists **Connect a model**; click it
to open **Settings → Providers**.

### Where keys are stored

Keys you enter in Settings go to `auth.json` in your config folder
(`~/.config/z-engine/auth.json`; Windows `%APPDATA%\z-engine\auth.json`),
readable only by your user account on macOS and Linux. They are never
written to settings files. The Providers page shows the last four
characters of a stored key; **Remove key** deletes it.

Keys are stored per API host: `openrouter`, `opencode`, `anthropic`,
`openai`, `github-copilot`, or `custom:<host:port>` for any other endpoint.
A key is only sent to its own host.

### Keys from environment variables

HyperForm looks for a key in this order:

1. `ZENGINE_API_KEY` – used for whichever provider is configured;
2. the provider's own variable: `OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`,
   `OPENAI_API_KEY` or `OPENCODE_API_KEY`;
3. the key stored in `auth.json`.

Keys set in the environment count as present on the Providers page. Web
search keys work the same way with `BRAVE_API_KEY`, `TAVILY_API_KEY` and
`EXA_API_KEY`.

> **Note:** Environment variables must be set in the environment HyperForm
> starts from. Apps opened from the Dock, Start menu or an app launcher
> often don't see variables from your shell profile, so storing keys in
> Settings is simpler.

OpenCode Zen's free models and local servers (Ollama, LM Studio) need no
key.

### GitHub Copilot

If you have a GitHub Copilot plan, you can chat with the models it
includes. Instead of pasting a key, you sign in with GitHub.

1. In **Settings → Providers**, click **Connect** next to **GitHub
   Copilot** (tagged **Sign in**). In the setup guide, choose **Use my API
   key** and pick **GitHub Copilot**.
2. Under **GitHub account**, choose **GitHub.com**, or **Enterprise** if
   your company runs GitHub Enterprise; then type its domain, for example
   `company.ghe.com`.
3. Click **Sign in with GitHub**. A one-time code appears; copy it, click
   **Open GitHub**, enter the code on GitHub's page and approve.
4. The dialog says **Waiting for you to approve…** and checks with GitHub
   until you do (**Cancel** stops it). Then it says **Signed in**.
5. Check the **Model** field (default `gpt-5-mini`) and click **Connect and
   use**.

What to know:

- **Which address.** GitHub.com uses `https://api.githubcopilot.com`;
  Enterprise uses `https://copilot-api.<your domain>`. The dialog sets it
  for you, so **Endpoint and advanced options** has no base URL or wire
  format for Copilot.
- **Which GitHub app.** Copilot only accepts sign-ins from apps it trusts,
  so HyperForm signs in through the same GitHub app OpenCode uses and asks
  only to read your profile (`read:user`). The GitHub token you get is sent
  only to the Copilot address.
- **Where the token goes.** It is stored in `auth.json` under
  `github-copilot` (Enterprise: `custom:copilot-api.<your domain>`).
  There is no Copilot environment variable, but `ZENGINE_API_KEY`, when
  set, is used instead.
- **Signing out.** **Sign out** on the provider's row removes the token.
  Without one, the row says **Active · needs sign-in**.
- **Which format each model gets.** Before a chat's first request, HyperForm
  asks Copilot which models you can use and how each one is served, then
  sends Claude models in the Anthropic format, GPT-5 class models through
  OpenAI's newer *Responses* API, and the rest as chat completions. If
  Copilot doesn't answer that question, it decides by the model's name.
- **Premium requests.** Copilot counts requests against your plan. Only a
  request that answers something you wrote is marked as yours; the agent's
  follow-up rounds after tool calls are marked as the agent's.
- **Models.** The model picker lists the models the catalog knows for
  GitHub Copilot, not the live list from your account. A model your
  organization or plan turned off fails with "enable it in your GitHub
  Copilot settings".

## Model roles

| Role | Setting | Used for | Default |
|---|---|---|---|
| **Main** | `model.main` | The main agent, and subagents with `model: inherit` (such as `general` and `plan`). | `anthropic/claude-sonnet-4.5` |
| **Fast** | `model.fast` | Chat titles, compaction summaries, answering WebFetch questions about a page, and the `explore` and `verify` agents. | The main model |
| **Review** | `model.review` | The `review` agent. | The main model |

Set them in **Settings → Models**. Model ids are written the way the
active provider names them (OpenRouter uses `vendor/model`, Anthropic uses
`claude-sonnet-4-5`, Ollama uses the local model name).

> **Tip:** A cheaper, faster model for the Fast role saves money on
> exploration and summaries without changing the main agent.

## Choosing a model for a chat

The model chip in the composer's bar shows the chat's model (and its
reasoning effort, when set). Click it for the model picker: the active
provider's models from the model catalog, with context size, output limit
and a brain icon for models that think before answering. Type in **Search
models or providers…** to filter. Picking one switches **this chat** only;
the field at the bottom (**Another model id, e.g.
anthropic/claude-sonnet-4.5**) with **Use** accepts any id. `/model <id>`
does the same, and `/model` alone shows the current one.

To change the default for new chats, set **Main model** in Settings →
Models. The **Main**, **Fast** and **Review model** fields there suggest
models from the same list as you type.

Which models the list shows:

- **Retired models are hidden**, for every provider. When the catalog marks
  a model as retired (*deprecated*), it leaves the list, but you can still
  type its id in the field at the bottom.
- **Free OpenCode Zen models are tagged Free**: the Zen models whose input
  price in the catalog is 0. Only OpenCode Zen gets the tag, because other
  providers can list models at a price of 0 that you pay for another way
  (GitHub Copilot models come with your plan).
- **OpenCode Zen without a key lists only its free models**, since its paid
  models fail without one. Once a Zen key is stored in **Settings →
  Providers** or set in `OPENCODE_API_KEY` or `ZENGINE_API_KEY`, the list
  shows every Zen model, free ones first.

The list is empty when the active provider needs a key and has none, or
when the provider isn't in the catalog (for example Ollama or LM Studio):
type the model name in the field at the bottom instead.

### The model catalog

Context sizes, output limits, image support and prices come from the
public catalog at [models.dev](https://models.dev). HyperForm downloads it
the first time you open the model list and caches it in the `cache`
folder of its data folder. When the cached copy is more than a day old,
opening the model list shows the cached copy and downloads a fresh one in
the background; the next time you open the list, newly released models
appear. To add or correct models, create `models.json` in your config
folder:

```json
{
  "openrouter": {
    "models": {
      "acme/private-coder": { "name": "Private Coder", "context": 128000, "output": 8192, "reasoning": true }
    }
  }
}
```

An entry can also set `"status"`: `"deprecated"` hides that model from the
model list, and any other value (for example `"active"`) shows a model the
catalog marks as retired.

## Reasoning effort

Some models can think before they answer. *Effort* controls how much:
`low`, `medium`, `high` or `max`. Unset (**default**) sends nothing and the
model uses its own default.

- The **Effort** row at the top of the model picker (**auto**, **low**,
  **medium**, **high**, **max**; shown for models the catalog marks as
  reasoning models, or when an effort is set) changes it for this chat;
  **auto** sends none. `/effort high` or `/effort default` does the same.
  The model chip shows the chosen effort next to the model's name.
- **Settings → Models → Requests → Reasoning effort** (`model.effort`) sets
  the default for new chats.
- On Anthropic, effort turns on *extended thinking* with a thinking budget
  of about 2k (low), 8k (medium), 16k (high) or 32k (max) tokens. On
  OpenAI-compatible providers it is sent as a reasoning-effort value; `max`
  is sent as `high`.

The agent's thinking appears in the transcript as a collapsible section.
More effort is slower and costs more.

> **Note:** With the experimental
> [Per-task routing](16-decision-features.md#per-task-routing) feature On
> and effort left on **auto**, HyperForm picks low, medium or high effort
> once per task and keeps it for the follow-ups. With
> `[decisions.routing] allow_model_switch = true` it may also run a simple
> task on the fast model. A **Routed** chip in the transcript shows the
> choice and why.

## Fallback models and retries

When a request fails for a temporary reason (rate limit, overload, server
error, timeout, lost connection), HyperForm retries automatically: up to 5
attempts in total, waiting 0.5 s, 1 s, 2 s, ... (at most 16 s), or as long
as the provider asks (at most 60 s). Meanwhile the island in the title bar
says **Provider busy** and counts down (**retry 2 in 4s**).

If the request still fails before any answer text arrived, HyperForm tries
the **fallback models** in order:

```toml
[model]
main = "anthropic/claude-sonnet-4.5"
fallbacks = ["openai/gpt-4o", "google/gemini-2.5-pro"]
```

Fallbacks use the same provider and key. A list in a higher settings file
replaces the lower one.

## Context window and output length

A provider can serve a model with smaller limits than its maker does. So
HyperForm reads a model's context size and output limit from the active
provider's own catalog listing when it has one (GitHub Copilot,
OpenRouter, Anthropic, OpenAI, OpenCode Zen, Groq and LM Studio). For
example, `gpt-4o` on GitHub Copilot gets Copilot's window, not OpenAI's.
When that listing lacks the model or the limit, or for other providers,
it uses the catalog's preferred listing for the model id.

- `model.context_window` overrides the catalog's context size for the main
  model; unknown models are assumed to have 128,000 tokens. The **Context
  window** field in **Settings → Models** shows the catalog's size when
  empty. See
  [Memory and context](07-memory-and-context.md#the-context-window).
- `model.max_output_tokens` (default 16,384, allowed 256–200,000) limits
  each answer; it's lowered automatically to the model's own limit when the
  catalog knows it.

## Cost tracking

HyperForm prices every request with the model's rates (from `[pricing]` in
your settings, else the catalog, else a small built-in price table) and
adds it up:

- hovering the receipt under a turn shows its time, tokens and cost; the
  **Compact** and **Detailed** views of **Settings → Appearance → Task
  report detail** show the cost on the receipt itself;
- the open island (click it in the title bar) shows this turn's cost while
  the agent works, and the chat's total;
- the project home's **Continue** card shows each recent chat's cost;
- `/cost` shows the chat's cost and its input, output, cache-read and
  cache-write tokens, plus tokens per agent;
- **Usage by agent**, folded at the bottom of the side panel's **Agents**
  tab, shows input tokens, output tokens and cost for the main agent and
  each subagent.

The catalog can list the same model id under several providers, at
different prices. As with [limits](#context-window-and-output-length),
HyperForm uses the active provider's own listing when it has a price for
the model, and otherwise the catalog's preferred listing, usually
OpenRouter's or the maker's.

Costs are estimates from published prices; your provider's bill is
authoritative. Models without a known price show no cost.

> **Note:** A GitHub Copilot model is priced from the catalog's GitHub
> Copilot listing for that model, falling back to its price elsewhere when
> Copilot's listing has none. Copilot itself charges through your plan's
> premium requests, so the cost shown is an estimate, not your bill.

Set your own
prices (US dollars per million tokens):

```toml
[pricing."acme/private-coder"]
input = 1.0
output = 4.0
cache_read = 0.1    # optional; defaults to the input price
cache_write = 1.25  # optional; defaults to the input price
```

To limit spending, set **Settings → Advanced → Agent limits → Session cost
cap** (`agents.session_cost_cap_usd`; typing "budget" in the Settings
search finds it). When a chat reaches it, the agent stops with "Stopped ·
reached the session cost cap".

### Prompt caching and cost

With prompt caching, the unchanged beginning of each request is read from
the provider's cache at a much lower price. HyperForm keeps that beginning
stable (system prompt, instructions, tool list) and marks cache points, so
long chats get cheaper per request. Caching is on by default for Anthropic
and OpenRouter; for other providers turn it on under **Settings →
Providers → Configure → Endpoint and advanced options → Prompt caching** if
the provider supports it. Details: [Memory and context](07-memory-and-context.md#prompt-caching).

See also: [Getting started](01-getting-started.md) · [Settings reference](12-settings-reference.md#model) · [Troubleshooting](14-troubleshooting.md)
