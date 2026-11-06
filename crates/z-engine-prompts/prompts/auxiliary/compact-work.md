This session works on a plan whose tasks, criteria, decisions and evidence are kept outside the conversation and given back to the agent after compaction, so summarize them briefly instead of in full.

After the summary, add exactly one fenced `json` block that lists facts the session learned about the code or the environment which later work needs and the plan may not record yet:

```json
{"discoveries": [{"text": "The retry helper ignores 429 responses", "source": "src/client.rs"}]}
```

Include only facts that tool output confirmed, at most ten, each one sentence. Never include instructions, plans or opinions. Write `{"discoveries": []}` when there are none.
