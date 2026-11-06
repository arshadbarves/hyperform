import type { AgentDef } from "../../protocol/config/AgentDef";
import type { CommandDef } from "../../protocol/config/CommandDef";
import type { OutputStyleDef } from "../../protocol/config/OutputStyleDef";
import type { RuleDef } from "../../protocol/config/RuleDef";
import type { SkillDef } from "../../protocol/config/SkillDef";
import type { ExtensionKind } from "./extensions";

type Field = [key: string, value: string | number | boolean | readonly string[] | null | undefined];

/** Plain words stay bare; anything YAML could misread is JSON-quoted (valid YAML). */
function scalar(value: string): string {
  return /^[A-Za-z0-9][\w .,/@()+-]*$/.test(value) && !/: |\s#|,\s*$/.test(value) ? value : JSON.stringify(value);
}

function line([key, value]: Field): string | null {
  if (value === null || value === undefined || value === "") return null;
  if (Array.isArray(value)) return value.length ? `${key}: [${value.map((item) => JSON.stringify(item)).join(", ")}]` : null;
  return `${key}: ${typeof value === "string" ? scalar(value) : String(value)}`;
}

function document(fields: Field[], body: string): string {
  const header = fields.map(line).filter((entry): entry is string => entry !== null);
  return `---\n${header.join("\n")}\n---\n\n${body.trim()}\n`;
}

/** Rebuilds a definition file from what `list_extensions` parsed. Comments and
 * keys the parser ignores are lost, and skills carry no body. */
export function markdownFromDef(kind: ExtensionKind, def: unknown): string {
  switch (kind) {
    case "agents": {
      const d = def as AgentDef;
      return document(
        [
          ["name", d.name],
          ["description", d.description],
          ["tools", d.tools],
          ["disallowedTools", d.disallowedTools],
          ["model", d.model],
          ["permissionMode", d.permissionMode],
          ["isolation", d.isolation === "worktree" ? "worktree" : null],
          ["maxTurns", d.maxTurns],
          ["color", d.color],
        ],
        d.prompt,
      );
    }
    case "commands": {
      const d = def as CommandDef;
      return document(
        [
          ["description", d.description],
          ["argument-hint", d.argumentHint],
          ["allowed-tools", d.allowedTools],
          ["model", d.model],
          ["disable-model-invocation", d.disableModelInvocation || null],
        ],
        d.body,
      );
    }
    case "skills": {
      const d = def as SkillDef;
      return document([["name", d.name], ["description", d.description], ["allowed-tools", d.allowedTools]], "");
    }
    case "rules": {
      const d = def as RuleDef;
      return document(
        [["name", d.name], ["description", d.description], ["globs", d.globs], ["alwaysApply", d.alwaysApply || null]],
        d.body,
      );
    }
    case "output-styles": {
      const d = def as OutputStyleDef;
      return document([["name", d.name], ["description", d.description]], d.body);
    }
  }
}
