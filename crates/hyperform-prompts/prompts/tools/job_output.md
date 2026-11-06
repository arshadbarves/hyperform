Reads the output of a background job: a shell started with Bash `run_in_background`, or an agent started with Agent `run_in_background`.

- `job_id` is the id returned when the job started.
- Returns only the output produced since your last JobOutput call for that job, with the job's status (running, completed, failed, or killed) and, once it has finished, its exit code. For an agent job, the output is the agent's report.
- `filter` is a regular expression; only matching lines are returned. Lines that do not match are consumed all the same, so they will not appear in a later call.
- `wait_ms` waits up to that many milliseconds for the job to finish before returning its new output. Use it instead of polling with `sleep`; omit it to return at once.
- You are notified when a job finishes, so there is no need to poll jobs you are not actively watching.
