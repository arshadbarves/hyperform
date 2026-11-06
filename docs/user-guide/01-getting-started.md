# Getting started

This page takes you from download to your first finished task: installing
HyperForm, the first-run setup, connecting it to an AI model provider,
opening a project folder, and approving the agent's first actions.

## Install HyperForm

Download the installer for your system from
[GitHub Releases](https://github.com/arshadbarves/z-engine/releases).

- **macOS:** choose the Apple Silicon or Intel `.dmg`, open it, and drag
  **HyperForm** to your Applications folder.
- **Windows:** run the `.exe` or `.msi` installer.
- **Linux:** install the `.deb` or `.rpm` package, or download the
  `.AppImage`, make it executable (`chmod +x`), and run it.

> **Tip:** If macOS refuses to open the app the first time, Control-click
> it in Applications and choose **Open**.

Two free tools make HyperForm better if they are installed:

- **git** – needed for code checkpoints (undo), the **Changes** tab and
  agent worktrees. Without git, those features are off.
- **ripgrep** (`rg`) – faster code search. Without it, search uses a
  built-in engine.

HyperForm updates itself. It checks for a new version when it starts; when
one is ready, a notice says so and an **Update** button appears at the
bottom of the sidebar. Click it (or open **Settings → About & Updates**) and
choose **Update & Restart**.

## First launch

While HyperForm loads, a short splash shows your
[pet](02-everyday-use.md#your-pet) dropping in and waking up. When the app
is ready, the pet flies into its place in the title bar. (With your
system's Reduce Motion setting on, the splash simply fades.) On a fresh
install (no projects and no chats yet), a setup guide opens instead and the
pet lands above the setup card. It stays there on every step and reacts to
each one. The dots at the top show which of its five steps you are on, and
**Back** returns to the previous one.

1. **Meet your pet**: it says **Hi, I'm Zen**. Type another name under
   **Call me** (up to 24 characters) and pick a **Look** if you like, then
   click **Get started**, or **Skip setup** to go straight to the app. Both
   save the name and look to your user settings.
2. **Choose how HyperForm thinks**: pick where the AI model runs.
   - **Start free**: OpenCode Zen's free models, with no account or key.
   - **Use my API key**: pick Anthropic, OpenAI, OpenRouter, GitHub
     Copilot, Google AI (Gemini), DeepSeek, Groq, Mistral AI or a custom
     endpoint, paste your key (for GitHub Copilot, sign in with GitHub
     instead), and click **Connect and continue**.
   - **Run on this computer**: Ollama or LM Studio
     ([below](#use-a-model-on-your-own-computer)).
   - **Keep …** is offered first when a provider is already set up on this
     computer.
3. **Pick a project to work on**: drop a project folder on the window, or
   click to choose one. HyperForm shows its git branch and asks **Do you
   trust this folder?** Answer **Trust it** or **Not now**
   ([workspace trust](03-permissions-and-safety.md#workspace-trust)).
   **Skip for now** continues without a project.
4. **Decide how it works with you**: how much the agent may do on its own
   (**Ask before changes**, **Edit on its own** or **Plan first**, the
   permission modes Ask, Auto-accept edits and Plan), how lively the pet
   is (**Lively**, **Calm** or **Off**) and, at **Lively**, whether it
   **Roams** or **Stays put**. These are saved to your user settings.
5. **You're all set**: click a suggested first prompt to open the project
   home with it in the composer, or click **Open HyperForm**.

You can change every choice later in **Settings**. Once you have a project
or a chat, the app opens straight to it.

## Connect a model provider

A *model provider* is the service that runs the AI model. HyperForm needs one
before it can do anything. The setup guide connects one on a fresh install;
to connect another or change it later:

1. Open **Settings** (⌘, on macOS, Ctrl+, on Windows and Linux, or the
   gear at the bottom of the sidebar) and choose **Providers**.
2. Click **Connect** next to a provider (**Configure** for the active one).
3. Paste an API key if the provider needs one (see below). An *API key* is a
   secret string the provider gives you so it can bill your account.
   **Get a key** opens the provider's key page.
4. Check the **Model** field; it is filled with a sensible default.
5. Click **Connect and use**.

The provider becomes **Active**. Keys are stored in a separate file,
`auth.json`, never in settings files. Each key belongs to one API host and
is only sent to that host.

| Provider | Key | Notes |
|---|---|---|
| OpenRouter | Required | The default. One key gives access to hundreds of models. |
| Anthropic | Required | Claude models through Anthropic's own API, with prompt caching and extended thinking. |
| OpenAI | Required | GPT and o-series models. |
| OpenCode Zen | Optional | Free chat models work **without a key**. A key is only needed for paid models. |
| GitHub Copilot | Sign in | The models your Copilot plan includes. **Sign in with GitHub** replaces the key field ([details](11-models-providers-and-cost.md#github-copilot)). |
| Google AI (Gemini), DeepSeek, Groq, Mistral AI | Required | OpenAI-compatible endpoints. |
| Ollama (Local) | None | A model running on your computer at `http://localhost:11434/v1`. |
| LM Studio (Local) | None | LM Studio's local server at `http://localhost:1234/v1`. |
| Custom endpoint | Usually | Any OpenAI-compatible or Anthropic-compatible server or proxy. |

Under **Endpoint and advanced options** you can change the API base URL,
the wire format (**Auto**, **OpenAI-compatible**, **Anthropic**), extra HTTP
headers, and prompt caching (**Auto**, **On**, **Off**). The line next to
the buttons says which settings file the dialog saves to, for example
**Saves to User settings**.

### Try it without an account: OpenCode Zen

1. In **Settings → Providers**, click **Connect** next to **OpenCode Zen**
   (or choose **Start free** in the setup guide, which connects it with
   the defaults below).
2. Leave the key empty, keep the **Model** field's default (`big-pickle`,
   one of Zen's free models), and click **Connect and use**.

You can now chat with Zen's free models. Until you add a Zen key, the model
picker lists only those, each tagged **Free**; with a key, Zen's paid
models appear too
([choosing a model](11-models-providers-and-cost.md#choosing-a-model-for-a-chat)).

### Use a model on your own computer

1. Start Ollama or LM Studio's local server and load a model.
2. Click **Connect** next to **Ollama (Local)** or **LM Studio (Local)**.
3. In **Model**, type the model name your server serves (for example
   `llama3.3` for Ollama). LM Studio has no default, so the field starts
   empty.
4. Click **Connect and use**.

> **Note:** Local models vary a lot in how well they use tools. If the agent
> gets confused, try a larger model.

More about keys, environment variables and model choice:
[Models, providers and cost](11-models-providers-and-cost.md).

## Open a project

HyperForm works inside a *project*: a folder on your disk (trust settings
call it a *workspace*).

1. In the sidebar, click **+** next to **Projects** (or **Add a project**
   when you have none yet).
2. Choose your project's folder.

The folder appears under **Projects** and becomes the active project. Its
home shows whenever no chat is open (click **Home** in the sidebar, or
**New chat**, ⌘N / Ctrl+N): the project name and branch, the composer,
starter prompts and setup cards
([the project home](02-everyday-use.md#the-project-home)). Type in the
composer to start a chat there. With no project yet, **New chat** asks you
for a folder first.

> **Tip:** HyperForm works best in a git repository. Run `git init` in a new
> project so checkpoints and the Changes tab work.

If the project's own settings would run programs or loosen yours (hooks,
MCP servers, checks, permission rules and a few more; see
[workspace trust](03-permissions-and-safety.md#workspace-trust)), a banner
in the chat asks whether you trust the workspace. Choose **Not now** if you
are unsure; everything else still works. The home's **Set up this project**
card has a **Trust** button for later.

## Your first task

1. Type a request in the composer, for example:

   ```text
   Explain what this project does and where the entry point is.
   ```

2. Press **Enter** to send. (Shift+Enter adds a new line.)
3. Watch the transcript. Each tool the agent uses (Read, Grep, Bash, ...)
   appears as a one-line card with a status and duration; several in a row
   fold into one line such as "Read 3 files · searched 2×", and once the
   turn is done its work folds into one line such as "Worked for 12s · read
   3 files". Click a line to see the details.
4. While the agent works, the island in the middle of the title bar says
   what it is doing and for how long. When it finishes, the island briefly
   shows the result; click it to see what the chat has cost so far.

Reading files inside the project and running read-only commands such as
`ls` or `git status` do not need approval, so a question like this usually
runs without interruption.

Now ask for a change:

```text
Add a --version flag to the command-line parser and a test for it.
```

This time the agent needs to edit files and run commands, so it asks first.

## The approval card

An *approval card* takes the place of the composer's text box whenever the
agent wants to do something your permission settings don't already allow
(your draft is kept for later). It asks a question,
such as **Allow Edit to change src/cli.rs?** or **Allow Bash to run cargo
test?**, and shows:

- why approval is needed (for example "edits src/cli.rs" or "the command
  needs approval");
- a preview: the diff of an edit, or the exact command. Previews longer
  than six lines fold; **Show all N lines** unfolds them.

Answer with a button, or with a key while the card has keyboard focus (it
takes focus unless you are typing in another field, and the key hints
appear on it):

| Button | Key | Effect |
|---|---|---|
| **Allow once** | `y` | Run this one action. |
| **Always allow…** → **In this chat** | `s` | Run it and allow matching actions for the rest of this chat. The menu shows the rule, for example `Bash(cargo test:*)`. |
| **Always allow…** → **In this project** | `p` | Also save the rule to your personal project settings. |
| **Deny…** | `n` | Refuse. You can type what the agent should do instead; press Enter or **Deny** to send. |

While a card waits, the agent is paused and the island turns amber and says
**Needs your approval**, with **Approve** to go to the card. To tell the
agent something else, use **Deny…** with your feedback; your draft comes
back once every card is answered. Details and rule syntax:
[Permissions and safety](03-permissions-and-safety.md).

> **Tip:** If you trust the agent with edits in this project, switch the
> permission mode to **Auto-accept edits** with the permission chip in the
> composer's bar. Commands still ask.

## Where to go next

- Learn the daily workflow in [Everyday use](02-everyday-use.md).
- Stop repeated prompts with rules in
  [Permissions and safety](03-permissions-and-safety.md).
- Tell the agent about your project in [Memory and context](07-memory-and-context.md)
  (try `/init`).

See also: [Everyday use](02-everyday-use.md) · [Models, providers and cost](11-models-providers-and-cost.md) · [Troubleshooting](14-troubleshooting.md)
