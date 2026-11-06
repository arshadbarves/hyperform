Records something in this chat's work plan so it reaches every later agent working on the plan, including after compaction. Each record goes to the tasks named in `taskIds`.

- `kind: "decision"`: a design decision. `text` is the choice, `title` names the question, `alternatives` lists the options you rejected and `reason` says why. Record decisions another agent could otherwise undo.
- `kind: "discovery"`: a fact you learned about the code or environment that the plan does not record yet. `source` says where it comes from (a path, command or URL).
- `kind: "experiment"`: a hypothesis you test (`text`) and how (`method`). Conclude it later with `experimentId`, `outcome` (`confirmed`, `refuted` or `inconclusive`), `result` and `conclusion`.
- `kind: "evidence"`: a note supporting a criterion of a task (`taskIds` with one task and `criterionId`). Check results are linked automatically when you run Verify; do not record them by hand.
- `kind: "requirement"` or `"invariant"`: proposes a requirement (what the work must deliver) or an invariant (what must stay true). The user accepts it before it binds.
- `files` lists the paths it concerns.
- Once this turn has read web content (WebFetch, WebSearch, or the report of a helper that did), or when `source` is a URL, what you record waits for the user: entries stay unconfirmed and requirements and invariants proposed until the user accepts them, and evidence for a criterion becomes an unconfirmed note instead of counting toward it.
