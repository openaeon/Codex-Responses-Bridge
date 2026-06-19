# model-toolcall-adapter-rs

> A standalone Rust adapter that lets text-only models work with Codex-style, OpenAI-compatible, and Anthropic-style coding clients.

`model-toolcall-adapter-rs` exposes OpenAI-compatible and Anthropic-style HTTP endpoints, converts standard tool definitions into a stable text protocol, sends that prompt to an upstream model, then parses the model's textual tool intent back into standard tool-call responses.

It is designed for models and providers that are useful at coding and reasoning but do not reliably support native function calling.

The goal is practical compatibility with mainstream programming agents and editor tools: Codex-style clients that expect OpenAI Responses, Claude/Anthropic-style clients that speak Messages-shaped payloads, and developer tools that can point at an OpenAI-compatible `base_url`.

## What It Does

- Serves `POST /v1/chat/completions` for OpenAI Chat Completions clients.
- Serves `POST /v1/responses` plus retrieve, input-items, cancel, and compact endpoints for OpenAI Responses-style clients.
- Serves `POST /v1/messages` for Anthropic Messages-style payloads.
- Converts OpenAI function tools into a model-readable XML/text protocol.
- Parses XML, JSON, and tolerant tool-call formats from plain model output.
- Supports OpenAI-compatible upstreams such as local Ollama/vLLM/LM Studio-style APIs.
- Includes a DeepSeek Web upstream provider with local session storage, PoW handling, SSE parsing, and reasoning/text separation.
- Includes a compact browser UI at `/ui` for debugging models, tools, and Responses flows.

The project is now standalone. It does not depend on `../crates/aeon-claw-api`, `aeon-claw-cli`, or the FCACoreai workspace at build time or runtime.

## Compatibility Targets

The adapter is protocol-oriented. It does not require a tool or editor to know about this project specifically; it only needs to speak one of the supported HTTP formats.

| Client family | Expected interface | Adapter endpoint |
| --- | --- | --- |
| Codex-style coding agents | OpenAI Responses-style API | `/v1/responses` |
| OpenAI-compatible coding tools | Chat Completions API | `/v1/chat/completions` |
| Anthropic/Claude-style clients | Messages-shaped payloads | `/v1/messages` |
| Editor and terminal agents with custom base URL support | OpenAI-compatible `base_url` | `http://127.0.0.1:8787/v1` |
| Local model stacks | Ollama, vLLM, LM Studio, llama.cpp-style OpenAI APIs | upstream `ADAPTER_UPSTREAM_BASE_URL` |

This makes it suitable as a bridge for coding environments such as Codex-compatible CLIs, Claude/Anthropic-style agent runtimes, Cursor/Continue-like editor integrations, Aider/OpenCode-style terminal agents, and custom internal agent platforms. Compatibility depends on the client allowing a custom base URL and on which wire format it uses.

The adapter is not an official OpenAI, Anthropic, Cursor, Continue, Aider, Cline, or OpenCode integration. It is a local protocol bridge that helps those classes of tools talk to upstream models that otherwise only return plain text.

## Architecture

```text
Coding Client / Agent Runtime
        |
        | Codex-style Responses
        | OpenAI Chat Completions
        | Anthropic-style Messages
        v
model-toolcall-adapter-rs
        |
        | tool schema -> text tool protocol
        | model text -> standard tool calls
        v
Upstream model
        |
        | OpenAI-compatible API
        | DeepSeek Web
        v
Plain-text model completion
```

The core modules are intentionally small:

- `src/wire/*` converts request and response wire formats.
- `src/protocol/mod.rs` renders the text tool protocol and parses tool calls.
- `src/upstream.rs` routes to OpenAI-compatible and DeepSeek Web upstreams.
- `src/deepseek_web.rs` implements the standalone DeepSeek Web client.
- `src/responses_store.rs` keeps in-memory Responses state for retrieve, input-items, cancel, and continuation flows.

## Quick Start

```bash
git clone https://github.com/openaeon/model-toolcall-adapter-rs.git
cd model-toolcall-adapter-rs
cargo run -- \
  --bind 127.0.0.1:8787 \
  --upstream-base-url http://127.0.0.1:11434/v1 \
  --upstream-model qwen3-coder \
  --model-aliases codex-adapter=qwen3-coder \
  --adapter-api-key local-dev-key
```

Open the built-in UI:

```text
http://127.0.0.1:8787/ui
```

Then use:

- Adapter API: `http://127.0.0.1:8787`
- Adapter API Key: `local-dev-key`
- Upstream API Base URL: your OpenAI-compatible model endpoint
- Model: either the upstream model name or an alias configured with `ADAPTER_MODEL_ALIASES`

## Configuration

Every CLI flag also has an environment variable.

```bash
export ADAPTER_BIND=127.0.0.1:8787
export ADAPTER_UPSTREAM_BASE_URL=http://127.0.0.1:11434/v1
export ADAPTER_UPSTREAM_API_KEY=
export ADAPTER_UPSTREAM_MODEL=qwen3-coder
export ADAPTER_MODEL_ALIASES=codex-adapter=qwen3-coder
export ADAPTER_API_KEY=local-dev-key
export ADAPTER_DEEPSEEK_SESSION_FILE=~/.model-toolcall-adapter/deepseek_session.json
cargo run
```

`ADAPTER_MODEL_ALIASES` is a comma-separated mapping:

```text
external-name=upstream-name,another-name=another-upstream-name
```

For example:

```bash
export ADAPTER_MODEL_ALIASES=gpt-5-codex=deepseek-web/reasoner,gpt-5-mini=deepseek-web/chat
```

Clients may request `model: "gpt-5-codex"`, while the adapter sends the request to `deepseek-web/reasoner` and restores the external model name in the response.

## Authentication

If `ADAPTER_API_KEY` is empty, adapter endpoints do not require authentication.

If it is set, pass either:

```http
Authorization: Bearer local-dev-key
```

or:

```http
x-api-key: local-dev-key
```

Per-request upstream overrides are supported:

```http
x-upstream-base-url: https://api.example.com/v1
x-upstream-api-key: sk-...
x-upstream-provider: openai-compatible
```

For DeepSeek Web:

```http
x-upstream-provider: deepseek-web
x-deepseek-session: {"cookie":"...","bearer":"...","last_session_id":"..."}
```

If `x-deepseek-session` is omitted, the adapter reads:

```text
~/.model-toolcall-adapter/deepseek_session.json
```

Set `ADAPTER_DEEPSEEK_SESSION_FILE` to use a different path.

## DeepSeek Web

DeepSeek Web support is fully local to this repository.

The UI can open the DeepSeek login page from `/deepseek-web/login`. After login, paste a Session JSON/Cookie into the UI and click "login/fetch models"; the adapter saves it to:

```text
~/.model-toolcall-adapter/deepseek_session.json
```

The saved object can contain:

```json
{
  "cookie": "ds_session=...; ...",
  "bearer": "optional-token",
  "user_agent": "Mozilla/5.0 ...",
  "base_url": "https://chat.deepseek.com",
  "last_session_id": "optional-session-id"
}
```

DeepSeek Web is an unofficial web upstream. If the web service changes its private endpoints, headers, or proof-of-work behavior, this provider may need updates.

## Endpoints

| Endpoint | Purpose |
| --- | --- |
| `GET /health` | Health check |
| `GET /ui` | Built-in debug UI |
| `GET /v1/models` | List upstream and alias models |
| `POST /v1/chat/completions` | OpenAI Chat Completions-compatible request |
| `POST /v1/messages` | Anthropic Messages-style request |
| `POST /v1/responses` | OpenAI Responses-style create |
| `GET /v1/responses/{response_id}` | Retrieve in-memory response |
| `GET /v1/responses/{response_id}/input_items` | List stored response input items |
| `POST /v1/responses/{response_id}/cancel` | Cancel background response |
| `POST /v1/responses/compact` | Compact response context |
| `POST /deepseek-web/login` | Open DeepSeek login page |
| `POST /deepseek-web/session` | Save DeepSeek session locally |

The same Responses routes are also available without `/v1` for clients that expect a base URL ending at the host.

## Chat Completions Example

```bash
curl http://127.0.0.1:8787/v1/chat/completions \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer local-dev-key' \
  -d '{
    "model": "qwen3-coder",
    "messages": [
      { "role": "user", "content": "查一下北京天气" }
    ],
    "tools": [{
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get weather by city",
        "parameters": {
          "type": "object",
          "properties": {
            "city": { "type": "string" }
          },
          "required": ["city"]
        }
      }
    }]
  }'
```

If the upstream model emits:

```xml
<tool_call id="call_1" name="get_weather">{"city":"北京"}</tool_call>
```

the adapter returns standard `tool_calls` in the Chat Completions response.

## Responses Tool Loop

First request:

```bash
curl http://127.0.0.1:8787/v1/responses \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer local-dev-key' \
  -d '{
    "model": "qwen3-coder",
    "input": "查一下北京天气",
    "tools": [{
      "type": "function",
      "name": "get_weather",
      "description": "Get weather by city",
      "parameters": {
        "type": "object",
        "properties": {
          "city": { "type": "string" }
        },
        "required": ["city"]
      }
    }]
  }'
```

If the model emits a tool call, the adapter returns:

```json
{
  "object": "response",
  "status": "completed",
  "output": [{
    "type": "function_call",
    "status": "completed",
    "call_id": "call_1",
    "name": "get_weather",
    "arguments": "{\"city\":\"北京\"}"
  }]
}
```

After your client executes the tool, continue with:

```bash
curl http://127.0.0.1:8787/v1/responses \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer local-dev-key' \
  -d '{
    "model": "qwen3-coder",
    "previous_response_id": "resp_xxx",
    "input": [{
      "type": "function_call_output",
      "call_id": "call_1",
      "output": "北京今天晴，气温 12-20 摄氏度"
    }]
  }'
```

The adapter stitches prior input, prior model output, and the new tool result into the next upstream prompt.

## Built-in Tools

The adapter can automatically execute a small set of internal tools in Responses mode:

- `get_overview`
- `get_quote`

These are intended for local testing of closed-loop behavior. Production tool execution should usually live in the calling agent runtime.

## Development

```bash
cargo fmt --check
cargo check
cargo test
```

The project is intentionally kept as a single standalone Rust binary crate.

## Current Boundaries

Implemented:

- Non-streaming Chat Completions, Messages, and Responses compatibility.
- Responses create, retrieve, input-items, cancel, and compact endpoints.
- `previous_response_id` continuation.
- Top-level Responses `function_call` and `function_call_output` loops.
- Model aliases.
- Adapter API-key authentication.
- Per-request upstream base URL and API-key overrides.
- DeepSeek Web session save/read, PoW, completion, and text parsing.
- XML and tolerant JSON tool-call parsing.

Not yet implemented:

- Streaming output.
- Full browser cookie extraction for DeepSeek Web login.
- Durable response storage beyond process memory.
- Full `tool_choice` behavior.
- Production-grade tool execution sandboxing.

## License

MIT
