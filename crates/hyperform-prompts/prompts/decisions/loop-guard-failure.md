A tool call keeps failing the same way. What would fix it? The state holds the tool, its target and the start of the error.

- retry: The failure is transient (a timeout, a rate limit, a lock or a flaky network); running it again may pass.
- code: The failure comes from the code or its tests; the code has to change.
- environment: The failure comes from the environment (a missing tool or dependency, a permission, a path or a service); changing the code will not fix it.
