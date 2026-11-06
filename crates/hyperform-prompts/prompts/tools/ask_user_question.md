Asks the user one to four multiple-choice questions and waits for the answers.

Ask only when the answer materially changes the outcome and you cannot find it in the code, the conversation, or the project instructions: two sound designs with different trade-offs, a destructive or irreversible step, or a product decision that belongs to the user. Otherwise choose a sensible default, state the assumption in your reply, and continue.

- Put 1-4 related questions in one call. Each has a `question` (clear and complete, ending with a question mark), a `header` (a short chip label of at most about 12 characters, such as "Database"), its `options`, and `multiSelect`.
- Give each question 2-4 distinct `options`, each with a concise `label` (1-5 words) and a `description` of what it means or what it trades off. Put the option you recommend first and explain why in its description.
- Set `multiSelect: true` when the choices are not mutually exclusive.
- The user can always answer in their own words, so do not add an "Other" option.
- Do not use it to ask for permission to proceed or for plan approval: the app handles approvals, and plans go through ExitPlanMode.
- If the user dismisses the questions, do not ask them again: proceed on your best judgment and say so, or explain what is blocked.
- Subagents cannot ask the user.
