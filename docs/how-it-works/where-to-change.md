# Where do I change X?

Which files to touch for the most common changes. What each crate owns is
in [The crates](crates.md); the binding rules are the
[structure contract](../../AGENTS.md#how-to-add-things-follow-exactly).
Part of [How HyperForm works](README.md).

Paths are relative to `crates/`. Every row also means updating the docs
listed in the [documentation contract](../AGENTS.md#3-what-to-update-for-each-kind-of-change).

| To… | Change |
|---|---|
| add a tool | `hyperform-tools/src/builtin/<name>.rs` implementing `Tool`; its name in `names.rs` and an instance in `builtin/list.rs`; description `hyperform-prompts/prompts/tools/<name>.md` with a `pub const` in `src/tools.rs` |
| change a prompt or tool description | the markdown in [`hyperform-prompts/prompts/<area>/`](../../crates/hyperform-prompts/prompts/); a new file also needs a `pub const` in `src/<area>.rs` |
| add a setting | field and default in [`hyperform-config/src/settings/<section>.rs`](../../crates/hyperform-config/src/settings/) (clamps in `normalize.rs`), a commented example in `default_config.toml`, a v1 mapping in `migrate/convert.rs` if v1 had it; run `cargo test -p hyperform-config` |
| add a slash command | engine built-in: [`hyperform-engine/src/commands/`](../../crates/hyperform-engine/src/commands/) (`catalog.rs`); prompt built-in: `hyperform-prompts/prompts/commands/<name>.md` plus `BUILTIN` in `src/commands.rs`; app-only: `hyperform-gui/ui/src/lib/stores/uiCommands.ts` |
| add a built-in agent | `hyperform-prompts/prompts/agents/<name>.md` (frontmatter) plus a `BUILTIN` entry in [`src/agents.rs`](../../crates/hyperform-prompts/src/agents.rs) |
| add an experimental feature | a `FeatureId` variant in [`hyperform-config/src/features/id.rs`](../../crates/hyperform-config/src/features/id.rs) and its `FeatureSpec` in `registry.rs`; gate the code on `settings.feature(id)`; run `cargo test -p hyperform-config` |
| add a decision use | `hyperform-engine/src/decisions/uses/<name>.rs` (or a `<name>/` folder) implementing `DecisionUse` for its seams, added to `USES` in [`decisions/registry.rs`](../../crates/hyperform-engine/src/decisions/registry.rs); its question in `hyperform-prompts/prompts/decisions/<name>.md` with a `pub const` in `src/decisions.rs` |
| change a work-plan rule | who may do what in [`hyperform-work/src/authority.rs`](../../crates/hyperform-work/src/authority.rs), which status may follow which in `transitions.rs`, when a task is done in `kernel/gate.rs`, refusal texts in `error.rs`; how tools and the engine call it in `hyperform-engine/src/work/` |
| change what a replan holds, or what "needs you" counts | the hold in [`hyperform-work/src/held.rs`](../../crates/hyperform-work/src/held.rs) (with `Readiness::ReplanHeld` in `graph.rs`); the count in `needs.rs`, and the Plan tab's strip in `hyperform-gui/ui/src/lib/domain/work/needsYou.ts` |
| change how a plan pauses, resumes or moves | which helpers stop and how in [`hyperform-engine/src/work/interrupt.rs`](../../crates/hyperform-engine/src/work/interrupt.rs) and `bound.rs`; what a resume finds in `hyperform-work/src/resume.rs` and `hyperform-engine/src/work/resume.rs` (with `checkpoint.rs`); its reminder in `hyperform-prompts/prompts/reminders/work-resume.md`; moving a plan in `work/driver.rs` |
| add a hook event | `HOOK_EVENTS` in [`hyperform-config/src/settings/hooks.rs`](../../crates/hyperform-config/src/settings/hooks.rs); fire it from `hyperform-engine/src/hooks/` |
| add a protocol event | a variant of `Event` in [`hyperform-protocol/src/events.rs`](../../crates/hyperform-protocol/src/events.rs); emit it in the engine; a `case` in `hyperform-gui/ui/src/lib/domain/sessionView/reduce.ts`; run `cargo test -p hyperform-protocol` and commit the TypeScript |
| add an IPC command | a `#[tauri::command]` fn in `hyperform-gui/src-tauri/src/commands/<domain>.rs`, listed in `generate_handler!` in `main.rs`; a wrapper in `ui/src/lib/commands/<domain>.ts`; engine data from [`engine/queries/<topic>.rs`](../../crates/hyperform-engine/src/engine/queries/) |
| add a settings screen | `hyperform-gui/ui/src/components/settings/<Name>Tab.svelte`; its id in `SettingsTab` (`ui/src/lib/stores/ui.svelte.ts`), an entry in `SETTINGS_TABS` (`SettingsNav.svelte`), a `SETTINGS_SECTIONS` group and its `SETTING_ENTRIES` for search (`ui/src/lib/domain/settings/searchIndex.ts`), and a branch in [`SettingsPage.svelte`](../../crates/hyperform-gui/ui/src/components/settings/SettingsPage.svelte); form logic in `ui/src/lib/domain/settings/` |
| change permission logic | rules and syntax in [`hyperform-policy/src/rules/`](../../crates/hyperform-policy/src/rules/), the decision pipeline in `src/decide/`, shell analysis in `src/shell/`; settings to policy in `hyperform-engine/src/settings/policy.rs` |
| add a check parser | `hyperform-verify/src/parse/<runner>.rs` plus runner detection in [`parse/dispatch.rs`](../../crates/hyperform-verify/src/parse/dispatch.rs); discovery for a new ecosystem in `discovery/ecosystems/` |
| add a provider adapter | a module like `anthropic/` or `openai/` in [`hyperform-llm/src/`](../../crates/hyperform-llm/src/) implementing `ModelClient`; construction in `provider/build.rs`, endpoint detection in `provider/detect.rs`; the settings-side `ProviderKind` in `hyperform-config/src/settings/provider.rs`, mapped in `hyperform-engine/src/settings/client.rs` |

See also: [The crates](crates.md) · [AGENTS.md](../../AGENTS.md) ·
[Documentation contract](../AGENTS.md) ·
[v2 engine architecture](../architecture/v2-engine.md)
