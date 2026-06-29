# model-toolcall-adapter-rs

[English](README.md) | [简体中文](README.zh-CN.md)

> A local protocol adapter that lets upstream models without reliable native tool calling appear as standard tool-capable OpenAI / Codex / Anthropic-compatible endpoints.

Current release: `v0.2.0`

This project has one job:

```text
client sends standard tools
-> adapter renders a text tool protocol for the upstream model
-> upstream model emits plain-text tool intent
-> adapter parses that intent into standard function_call / tool_calls
-> caller executes the tool and sends tool output back
```

The adapter does not execute user tools. It converts protocols and preserves state; your client, application server, or agent runtime owns actual tool execution.

## When To Use It

Use it when:

- You have a strong coding or reasoning model that does not reliably support native function calling.
- You want to connect DeepSeek Web, Ollama, vLLM, LM Studio, llama.cpp, or another OpenAI-compatible upstream to Codex-style clients.
- Your client already sends OpenAI `tools`, but your upstream only produces plain text.
- You need one bridge for Responses, Chat Completions, and Anthropic Messages-shaped requests.

Do not use it as:

- A tool executor or full agent runtime.
- A full replacement for OpenAI-hosted vector stores or hosted tools.
- A byte-for-byte implementation of OpenAI server-side Structured Outputs or encrypted reasoning tokens.
- A distributed response-state backend shared across multiple remote nodes.

## Capabilities

| Capability | Status |
| --- | --- |
| OpenAI Chat Completions | `POST /v1/chat/completions` |
| OpenAI Responses | create / retrieve / delete / input_items / input_tokens / cancel / compact |
| Conversations | create / retrieve / update / delete / items |
| Anthropic Messages | `POST /v1/messages` shape |
| Tool-call adaptation | Parses XML, JSON, and tolerant plain-text tool intents into standard calls |
| Tool execution | Not executed by the adapter |
| Streaming | Real Responses SSE; Chat / Messages streaming is not yet incremental |
| Long tasks | Responses `background: true`, retrieve polling, cancel |
| Structured output | `json_object` and common recursive `json_schema` checks |
| Reasoning | Reasoning/text separation; local opaque `reasoning.encrypted_content` placeholder/pass-through |
| Images and files | Standard image/file request parts; DeepSeek Web uploads and references files internally |
| DeepSeek Web | Controlled-browser login, session capture, PoW, SSE parsing, search/reasoning/expert/vision mapping |
| Codex | One-click backup and write of `~/.codex/config.toml` and `auth.json` |
| Local state | JSON response/conversation store with sidecar file locks and atomic replacement |

## Architecture

```text
Codex / SDK / Agent Runtime
        |
        | OpenAI Responses
        | Chat Completions
        | Anthropic Messages
        v
model-toolcall-adapter-rs
        |
        | tools -> text protocol
        | plain text -> standard tool calls
        v
Upstream Provider
        |
        | OpenAI-compatible API
        | DeepSeek Web
        v
Actual model
```

| Path | Responsibility |
| --- | --- |
| `src/wire/` | External wire formats to/from the internal unified request |
| `src/protocol/` | Tool protocol prompt rendering and model-output parsing |
| `src/providers/openai_compat.rs` | OpenAI-compatible upstreams |
| `src/providers/deepseek_web/` | DeepSeek Web login, session, PoW, uploads, completion, SSE |
| `src/responses_store.rs` | Local Responses / Conversations state |
| `src/http/routes.rs` | HTTP routes, auth, Codex setup, Responses state machine |
| `src/ui.rs` | Local setup wizard |

## Quick Start

From source:

```bash
git clone https://github.com/openaeon/model-toolcall-adapter-rs.git
cd model-toolcall-adapter-rs
cargo run
```

Open:

```text
http://127.0.0.1:8787/ui
```

If the port is already in use:

```bash
ADAPTER_BIND=127.0.0.1:8899 cargo run
```

Release packages are expected under:

```text
dist/packages/
├── model-toolcall-adapter-rs-v0.2.0-windows-x64-exe.zip
├── model-toolcall-adapter-rs-v0.2.0-macos-arm64.tar.gz
├── model-toolcall-adapter-rs-v0.2.0-linux-x64-server.tar.gz
├── model-toolcall-adapter-rs-v0.2.0-linux-arm64-server.tar.gz
└── SHA256SUMS.txt
```

`SHA256SUMS.txt` verifies the four platform archives. The repository tracks release archives and checksums, not unpacked temporary package directories.

Windows:

```powershell
Expand-Archive .\model-toolcall-adapter-rs-v0.2.0-windows-x64-exe.zip
cd .\model-toolcall-adapter-rs-windows-x64-exe\model-toolcall-adapter-rs-windows-x64
.\model-toolcall-adapter-rs.exe
```

macOS / Linux:

```bash
tar -xzf model-toolcall-adapter-rs-v0.2.0-macos-arm64.tar.gz
cd model-toolcall-adapter-rs-macos-arm64
chmod +x ./model-toolcall-adapter-rs
./model-toolcall-adapter-rs
```

## First Run

The first run creates:

```text
~/.model-toolcall-adapter/config.json
```

It contains a random `adapter_api_key`. Open `/ui` and follow the three setup steps:

1. Select `openai-compatible` or `deepseek-web`.
2. For DeepSeek Web, start the controlled browser and log in.
3. Capture the session and copy the base URL, adapter key, model name, or apply Codex config.

![Setup wizard](docs/assets/setup-wizard.png)

## Codex

The setup wizard can write Codex configuration for you. It backs up:

- `~/.codex/config.toml`
- `~/.codex/auth.json`

Then it writes a Responses provider:

```toml
model_provider = "ModelToolCallAdapter"

[model_providers.ModelToolCallAdapter]
name = "ModelToolCallAdapter"
base_url = "http://127.0.0.1:8787/v1"
wire_api = "responses"
requires_openai_auth = true
```

And stores the adapter key:

```json
{
  "OPENAI_API_KEY": "adp_xxx"
}
```

Restart Codex after applying the config.

## Providers

OpenAI-compatible upstream:

```bash
ADAPTER_UPSTREAM_BASE_URL=http://127.0.0.1:11434/v1 \
ADAPTER_UPSTREAM_MODEL=qwen3-coder \
cargo run
```

Model aliases:

```bash
ADAPTER_MODEL_ALIASES=gpt-5-codex=qwen3-coder,gpt-5-mini=qwen3-fast cargo run
```

DeepSeek Web models:

| Model | Meaning |
| --- | --- |
| `deepseek-web/reasoner` | reasoning |
| `deepseek-web/chat` | normal chat |
| `deepseek-web/search` | search-enabled mode |
| `deepseek-web/expert` | expert mode |
| `deepseek-web/vision` | vision/file mode |

DeepSeek session is saved to:

```text
~/.model-toolcall-adapter/deepseek_session.json
```

Override it with:

```bash
ADAPTER_DEEPSEEK_SESSION_FILE=/path/to/deepseek_session.json cargo run
```

DeepSeek Web is an unofficial web upstream. If its private API, headers, PoW, or SSE format changes, the provider may need updates.

## API Examples

Responses tool call:

```bash
curl http://127.0.0.1:8787/v1/responses \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer adp_xxx' \
  -d '{
    "model": "deepseek-web/reasoner",
    "input": "Use a tool when external information is required.",
    "tools": [{
      "type": "function",
      "name": "search_web",
      "description": "Search by query",
      "parameters": {
        "type": "object",
        "properties": {
          "query": { "type": "string" }
        },
        "required": ["query"]
      }
    }]
  }'
```

Tool output continuation:

```bash
curl http://127.0.0.1:8787/v1/responses \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer adp_xxx' \
  -d '{
    "model": "deepseek-web/reasoner",
    "previous_response_id": "resp_xxx",
    "input": [{
      "type": "function_call_output",
      "call_id": "call_1",
      "output": "Tool result"
    }]
  }'
```

Chat Completions tool call:

```bash
curl http://127.0.0.1:8787/v1/chat/completions \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer adp_xxx' \
  -d '{
    "model": "deepseek-web/chat",
    "messages": [{ "role": "user", "content": "Check Beijing weather" }],
    "tools": [{
      "type": "function",
      "function": {
        "name": "get_weather",
        "parameters": {
          "type": "object",
          "properties": { "city": { "type": "string" } },
          "required": ["city"]
        }
      }
    }]
  }'
```

Responses streaming:

```json
{
  "model": "deepseek-web/reasoner",
  "input": "Analyze this project",
  "stream": true
}
```

Responses background mode:

```json
{
  "model": "deepseek-web/reasoner",
  "input": "Run a long analysis",
  "background": true
}
```

Poll:

```bash
curl http://127.0.0.1:8787/v1/responses/resp_xxx \
  -H 'authorization: Bearer adp_xxx'
```

Cancel:

```bash
curl -X POST http://127.0.0.1:8787/v1/responses/resp_xxx/cancel \
  -H 'authorization: Bearer adp_xxx'
```

## Images, Files, And Local File Search

Use standard request parts. Responses example:

```json
{
  "model": "deepseek-web/vision",
  "input": [{
    "type": "message",
    "role": "user",
    "content": [
      { "type": "input_text", "text": "Inspect this image" },
      { "type": "input_image", "image_url": "data:image/png;base64,..." }
    ]
  }]
}
```

DeepSeek Web uploads attachments internally, waits for parsing/readiness, and sends private `ref_file_ids` upstream. Expert mode does not directly support file references, so file-bearing requests are bridged through the vision/file path when needed.

Responses `tools:[{"type":"file_search"}]` searches only readable `input_file.file_data` content from the current request. It does not read arbitrary local files and is not a durable vector store.

## Configuration

| Setting | Environment variable | Default |
| --- | --- | --- |
| Bind address | `ADAPTER_BIND` | `127.0.0.1:8787` |
| OpenAI-compatible upstream | `ADAPTER_UPSTREAM_BASE_URL` | `http://127.0.0.1:11434/v1` |
| Upstream API key | `ADAPTER_UPSTREAM_API_KEY` | empty |
| Upstream model | `ADAPTER_UPSTREAM_MODEL` | `local-model` |
| Model aliases | `ADAPTER_MODEL_ALIASES` | empty |
| Adapter API key | `ADAPTER_API_KEY` | local config |
| Max tools | `ADAPTER_MAX_TOOL_DEFINITIONS` | `64` |
| Request timeout | `ADAPTER_REQUEST_TIMEOUT_SECS` | `120` |
| Config file | `ADAPTER_CONFIG_FILE` | `~/.model-toolcall-adapter/config.json` |
| Response store | `ADAPTER_RESPONSE_STORE_FILE` | `~/.model-toolcall-adapter/responses_store.json` |
| Conversation store | `ADAPTER_CONVERSATION_STORE_FILE` | `~/.model-toolcall-adapter/conversations_store.json` |
| DeepSeek session | `ADAPTER_DEEPSEEK_SESSION_FILE` | `~/.model-toolcall-adapter/deepseek_session.json` |

Per-request overrides:

```http
x-upstream-provider: deepseek-web
x-upstream-base-url: https://api.example.com/v1
x-upstream-api-key: sk-...
x-deepseek-session: {"cookie":"..."}
```

## Endpoints

| Endpoint | Purpose |
| --- | --- |
| `GET /health` | Health check |
| `GET /ui` | Setup wizard |
| `GET /v1/models` | Model list |
| `POST /v1/chat/completions` | Chat Completions |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/responses` | Responses create |
| `GET /v1/responses/{id}` | Retrieve response |
| `DELETE /v1/responses/{id}` | Delete response |
| `GET /v1/responses/{id}/input_items` | Response input items |
| `POST /v1/responses/{id}/cancel` | Cancel background response |
| `POST /v1/responses/input_tokens` | Estimate input tokens |
| `POST /v1/responses/compact` | Compact response context |
| `POST /v1/conversations` | Create conversation |
| `GET /v1/conversations/{id}` | Retrieve conversation |
| `POST /v1/conversations/{id}` | Update metadata |
| `DELETE /v1/conversations/{id}` | Delete conversation |
| `GET /v1/conversations/{id}/items` | List items |
| `POST /v1/conversations/{id}/items` | Append items |
| `GET /v1/conversations/{id}/items/{item_id}` | Retrieve item |
| `DELETE /v1/conversations/{id}/items/{item_id}` | Delete item |
| `GET /setup/state` | Setup state |
| `POST /setup/provider` | Save provider |
| `POST /setup/deepseek-browser/start` | Start DeepSeek login browser |
| `POST /setup/deepseek-browser/capture` | Capture DeepSeek session |
| `POST /setup/codex/apply` | Write Codex config |

## Packaging

```bash
rustup target add aarch64-apple-darwin
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-unknown-linux-musl
cargo install cargo-zigbuild
brew install zig
```

```bash
cargo build --release --target aarch64-apple-darwin
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-musl
cargo zigbuild --release --target x86_64-pc-windows-gnu
```

## Development

```bash
cargo fmt -- --check
cargo test
cargo build
```

## Boundaries

- The adapter does not execute user business tools.
- The adapter reads only its own controlled browser profile for DeepSeek login capture.
- `reasoning.encrypted_content` is a local opaque compatibility token, not OpenAI server-side encryption.
- `json_schema` support covers common Structured Outputs constraints, not the complete JSON Schema specification.
- DeepSeek Web depends on private web APIs and may require maintenance when the website changes.

## License

MIT
