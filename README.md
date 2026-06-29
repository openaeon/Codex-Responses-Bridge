<div align="center">

# model-toolcall-adapter-rs

**Make plain-text upstream models usable as standard tool-capable Codex / OpenAI / Anthropic endpoints.**

[English](README.md) · [简体中文](README.zh-CN.md)

![release](https://img.shields.io/badge/release-v0.2.0-1f6feb)
![rust](https://img.shields.io/badge/Rust-2021-b7410e)
![responses](https://img.shields.io/badge/OpenAI-Responses-111827)
![deepseek](https://img.shields.io/badge/DeepSeek-Web-2563eb)
![license](https://img.shields.io/badge/license-MIT-16a34a)

</div>

---

## Positioning

`model-toolcall-adapter-rs` is a local protocol bridge. It receives standard `tools`, renders them into a text protocol for plain upstream models, then parses model-emitted tool intent back into standard `function_call` / `tool_calls`.

It does not execute user tools and does not replace your agent runtime. Codex, your client, your application server, or your runtime still owns real tool execution.

```text
standard tools request
    -> text tool protocol
    -> plain upstream model
    -> textual tool intent
    -> standard function_call / tool_calls
```

<table>
  <tr>
    <td><strong>Client APIs</strong><br/>Responses, Chat Completions, Messages</td>
    <td><strong>Upstreams</strong><br/>OpenAI-compatible, local models, DeepSeek Web</td>
  </tr>
  <tr>
    <td><strong>Boundary</strong><br/>Adapts tool calls, does not execute tools</td>
    <td><strong>Release</strong><br/><code>v0.2.0</code></td>
  </tr>
</table>

<p align="center">
  <img src="docs/assets/setup-wizard.png" alt="model-toolcall-adapter-rs setup wizard" width="820">
</p>

## Quick Links

| Goal | Section |
| --- | --- |
| Run it locally | [Quick Start](#quick-start) |
| Configure Codex | [Codex](#codex) |
| Log in to DeepSeek Web | [DeepSeek Web](#deepseek-web) |
| See request examples | [API Examples](#api-examples) |
| Find environment variables | [Configuration](#configuration) |
| Download or verify packages | [Release Packages](#release-packages) |
| Build packages yourself | [Packaging](#packaging) |
| Understand limits | [Boundaries](#boundaries) |

## Fit

| Use it for | Do not use it for |
| --- | --- |
| Strong models that do not reliably support native function calling | Direct shell, browser, database, or business-tool execution inside the adapter |
| DeepSeek Web / Ollama / vLLM / LM Studio behind Codex-style clients | Full replacement for OpenAI-hosted `file_search` or vector stores |
| Clients that send OpenAI `tools` to plain-text upstreams | Byte-for-byte OpenAI server-side Structured Outputs or encrypted reasoning tokens |
| One bridge for Responses, Chat Completions, and Messages | Distributed response-state sharing across multiple remote nodes |

## Capability Map

| Area | Supported | Notes |
| --- | --- | --- |
| Wire APIs | Responses / Chat Completions / Messages | Codex, OpenAI-compatible, and Anthropic-shaped clients |
| Tool calls | XML / JSON / tolerant text parsing | Emits standard tool calls; does not execute them |
| Responses state | retrieve / delete / input_items / input_tokens / cancel / compact | Supports `previous_response_id` and Conversations |
| Streaming | Responses SSE | Chat / Messages streaming is not yet incremental |
| Long tasks | `background: true` | Retrieve polling and cancellation |
| Structured output | `json_object` / common recursive `json_schema` | Not a complete JSON Schema engine |
| Reasoning | Reasoning/text separation | `reasoning.encrypted_content` is a local opaque placeholder/pass-through |
| DeepSeek Web | Login, session, PoW, SSE, uploads | Search, reasoning, expert, vision/file mode mapping |
| Codex | One-click config | Backs up and writes `~/.codex/config.toml` and `auth.json` |
| Local state | JSON + lock + atomic replacement | Reduces corruption risk after crashes or multiple local processes |

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

### From Source

```bash
git clone https://github.com/openaeon/model-toolcall-adapter-rs.git
cd model-toolcall-adapter-rs
cargo run
```

Then open `http://127.0.0.1:8787/ui`.

If the port is already in use:

```bash
ADAPTER_BIND=127.0.0.1:8899 cargo run
```

### Release Packages

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

<details>
<summary>Windows</summary>

```powershell
Expand-Archive .\model-toolcall-adapter-rs-v0.2.0-windows-x64-exe.zip
cd .\model-toolcall-adapter-rs-windows-x64-exe\model-toolcall-adapter-rs-windows-x64
.\model-toolcall-adapter-rs.exe
```

</details>

<details>
<summary>macOS</summary>

```bash
tar -xzf model-toolcall-adapter-rs-v0.2.0-macos-arm64.tar.gz
cd model-toolcall-adapter-rs-macos-arm64
chmod +x ./model-toolcall-adapter-rs
./model-toolcall-adapter-rs
```

</details>

<details>
<summary>Linux</summary>

```bash
tar -xzf model-toolcall-adapter-rs-v0.2.0-linux-x64-server.tar.gz
cd model-toolcall-adapter-rs-linux-x64
chmod +x ./model-toolcall-adapter-rs
./model-toolcall-adapter-rs
```

</details>

## First Run Flow

The first run creates:

```text
~/.model-toolcall-adapter/config.json
```

It contains a random `adapter_api_key`. Open `/ui` and follow the three setup steps:

1. Select `openai-compatible` or `deepseek-web`.
2. For DeepSeek Web, start the controlled browser and log in.
3. Capture the session and copy the base URL, adapter key, model name, or apply Codex config.

The setup wizard shows the Adapter Base URL, adapter key, model name, and request examples so they can be copied into Codex or another client.

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
