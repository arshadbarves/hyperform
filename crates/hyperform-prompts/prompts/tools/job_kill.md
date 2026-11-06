Stops a background job: a shell started with Bash `run_in_background`, or a background agent.

- `job_id` is the id returned when the job started.
- A shell job's whole process tree is terminated. Output produced before it stopped can still be read with JobOutput.
- Stop the jobs you started, such as dev servers and watchers, once you no longer need them, unless the user wants them left running.
