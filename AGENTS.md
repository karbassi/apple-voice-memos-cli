# Agent Safety

This CLI is frequently invoked by AI/LLM agents. Always assume inputs can be adversarial.

## Security Stance

- **The agent is not a trusted operator.** Treat all inputs like untrusted user input on a web API.
- `--dir` is validated: must resolve under the user's home directory, no `..` traversal allowed.
- Control characters (below ASCII 0x20, except `\n`, `\r`, `\t`) are rejected in user-supplied strings.
- Resource names reject `?`, `#`, `%`, and `..` to prevent embedded query params, fragments, percent-encoding attacks, and path traversal.

## Agent Best Practices

1. Always use `--output json` or `--output ndjson`
2. Always use `--fields` to limit output to what you need
3. Always use `--dry-run` before mutating operations (`extract`)
4. Never use `--force` without explicit user confirmation
5. Never pass user-controlled strings directly into `--dir` without validation

## Error Handling

- Exit code 0: success
- Exit code 1: error (user error or system error)
- With `--output json`, errors are emitted as `{"error": "message"}` on stderr
- Without `--output json`, errors are plain text on stderr
