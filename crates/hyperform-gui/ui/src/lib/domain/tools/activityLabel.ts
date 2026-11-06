import type { JsonValue } from "../../protocol/serde_json/JsonValue";
import { baseName, firstLine, str } from "./toolInput";
import { toolMeta } from "./toolMeta";

const MAX = 52;

/** Imperative verbs the model starts Bash descriptions with, as ongoing actions. */
const PROGRESSIVE: Record<string, string> = {
  add: "Adding",
  build: "Building",
  check: "Checking",
  clean: "Cleaning",
  commit: "Committing",
  compile: "Compiling",
  copy: "Copying",
  create: "Creating",
  delete: "Deleting",
  fetch: "Fetching",
  find: "Finding",
  format: "Formatting",
  generate: "Generating",
  get: "Getting",
  install: "Installing",
  lint: "Linting",
  list: "Listing",
  make: "Making",
  move: "Moving",
  print: "Printing",
  pull: "Pulling",
  push: "Pushing",
  read: "Reading",
  remove: "Removing",
  rename: "Renaming",
  run: "Running",
  search: "Searching",
  show: "Showing",
  start: "Starting",
  stop: "Stopping",
  test: "Testing",
  update: "Updating",
  verify: "Verifying",
};

function clip(text: string): string {
  return text.length > MAX ? `${text.slice(0, MAX - 1)}…` : text;
}

function phrase(verb: string, subject: string, sep = " "): string {
  return subject ? clip(`${verb}${sep}${subject}`) : verb;
}

function ongoing(description: string): string {
  const [first = "", ...rest] = description.split(" ");
  const verb = PROGRESSIVE[first.toLowerCase()];
  return clip(verb ? [verb, ...rest].join(" ") : description);
}

function host(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

function filePath(input: JsonValue): string {
  return baseName(str(input, "file_path", "notebook_path", "path"));
}

/** What a tool call is doing, in the words a person would use: "Reading auth.rs", "Running the tests". */
export function activityLabel(name: string, input: JsonValue): string {
  const meta = toolMeta(name);
  switch (meta.family) {
    case "read":
      return phrase("Reading", filePath(input));
    case "edit":
      return phrase("Editing", filePath(input));
    case "write":
      return phrase("Writing", filePath(input));
    case "bash": {
      const description = firstLine(str(input, "description"));
      return description ? ongoing(description) : phrase("Running", firstLine(str(input, "command")));
    }
    case "search":
      if (name === "Glob") return phrase("Finding", str(input, "pattern"));
      return phrase("Searching for", str(input, "pattern", "query"));
    case "web":
      if (name === "WebSearch") return phrase("Searching the web for", str(input, "query"));
      return phrase("Reading", host(str(input, "url")));
    case "agent":
      return phrase("Delegating", str(input, "description"), ": ");
    case "todo":
      return "Updating the plan";
    case "question":
      return "Asking you a question";
    case "plan":
      return "Proposing a plan";
    case "job":
      return /kill/i.test(name) ? "Stopping a background job" : "Checking a background job";
    case "mcp":
      return clip(`Using ${meta.server} · ${meta.label.replaceAll("_", " ")}`);
    default:
      return genericLabel(name, input);
  }
}

function genericLabel(name: string, input: JsonValue): string {
  switch (name) {
    case "Verify":
      return "Checking the changes";
    case "LSP":
      return "Asking the language server";
    case "Skill":
      return phrase("Using the skill", str(input, "skill", "name"));
    case "ApplyAgentChanges":
      return "Applying an agent's changes";
    case "ListMcpResources":
    case "ReadMcpResource":
      return "Reading MCP resources";
    case "LoadMcpTools":
      return "Loading MCP tools";
    default:
      return phrase("Running", name || "a tool");
  }
}
