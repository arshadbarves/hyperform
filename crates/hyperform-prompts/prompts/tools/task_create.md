Creates a task in this chat's work plan. A plan keeps the tasks of a larger piece of work with their acceptance criteria, scope, decisions and evidence, so the work survives compaction and new chats. When no plan is attached, the first TaskCreate starts one; in plan mode it is a draft the user approves with your ExitPlanMode plan.

Use tasks for work with several parts that need ordering, verification or a record of decisions. Keep using TodoWrite for the steps of the task you are working on: while a task is in progress, your todo list is saved as its steps. A single simple change needs no task.

- `subject` is a short imperative title ("Add retry to the client"); `description` says what done looks like and anything another agent would need; `activeForm` is the present continuous shown while it runs ("Adding retry to the client").
- `blockedBy` lists task ids that must be done first; `parentId` makes it a subtask.
- `acceptance` lists criteria as `{"text": "...", "method": "..."}`. A `test`, `build`, `lint` or `typecheck` criterion is met only by a passing Verify run of that kind on the current code; an `inspection` criterion you confirm with TaskUpdate `metCriteria`; a `user` criterion only the user confirms. Prefer check criteria where a check exists.
- `scope` lists the files or globs the task changes; `requirements` lists the ids of plan requirements it serves; `kind` is `research`, `design`, `implement`, `verify`, `integrate` or `experiment`; `priority` is `low`, `normal` or `high`.
- Once this turn has read web content (WebFetch, WebSearch, or the report of a helper that did), a task you add to an approved plan is proposed: it waits for the user to accept it in the Plan tab, and nobody can start it before then. While the plan is a draft or being replanned, the user's approval covers it.
- The result gives the task id. A rejected change comes back with the exact reason: fix the input rather than repeating the call.
