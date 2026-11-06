Helper {{agent}} on task {{task}}: {{event}}. The task is now {{status}}.
Its report: {{summary?}}
Newly ready tasks:{{ready?}}
Tasks that may run in parallel now:{{parallel?}}
Held until the plan change is accepted:{{held?}}
Read the report, and resolve what it leaves open. Dispatch ready tasks with Agent and `task_id`; to continue an interrupted task, add `resume` with the agent id of the helper its resume pointer names. Start several at once only when they are all in the parallel list, and leave held tasks alone. Do not mention this reminder to the user.
