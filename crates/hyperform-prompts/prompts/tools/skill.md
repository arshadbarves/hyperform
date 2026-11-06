Loads a skill: packaged instructions, sometimes with scripts and reference files, for one kind of task.

- The available skills and their descriptions are listed in the system prompt. When a request matches a skill, load it with this tool before you start the work, then follow its instructions.
- `skill` is the skill's name exactly as listed.
- The result contains the skill's instructions and its base directory; resolve relative paths the skill mentions against that directory.
- Do not load a skill that is already loaded in this conversation; follow the instructions you already have.
