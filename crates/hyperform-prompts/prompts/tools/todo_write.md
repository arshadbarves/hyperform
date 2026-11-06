Creates and updates your todo list for the current task. The list is shown to the user above the chat and survives context compaction, so it is both a progress display and your working memory.

Use it when:
- the task has three or more distinct steps, or spans several files or components;
- the user gives you a list of things to do, or asks for a todo list;
- new requirements arrive while you work (add them right away).

Skip it for a single simple change, a quick lookup, or a conversational reply.

How to use it:
- Send the complete list every time; it replaces the previous one.
- Each item has `content` in the imperative ("Add retry to the client"), `activeForm` in the present continuous ("Adding retry to the client"), and a `status` of `pending`, `in_progress`, or `completed`.
- While you work, keep exactly one item `in_progress`. Mark an item `in_progress` before you start it and `completed` as soon as it is done; do not save completions up for later.
- Mark an item completed only when it is fully done. If checks fail, the work is partial, or you are blocked, leave it open and add an item for what remains or what blocks it.
- Keep the list true to the plan: add follow-ups as you discover them, and remove items that no longer apply.
- Make items concrete and checkable: "Run the parser tests", not "Make sure everything works".
