This chat picks up the work plan "{{goal}}" again; the plan is {{status}}. {{summary}}
Completed:{{completed?}}
Current:{{current?}}
Known issues:{{issues?}}
Reconciliation findings:{{findings?}}
Next: {{next?}}
Continue from here instead of starting over. A task whose helper was interrupted is pending with a resume pointer: continue it with Agent, `task_id` and `resume` set to the agent the pointer names (only in the chat it ran in), or start a new helper with `task_id` alone. Work from the task packet that follows, keep the plan current with TaskUpdate and PlanRecord, and tell the user briefly where the work stands. Do not mention this reminder to the user.
