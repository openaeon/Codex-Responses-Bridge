# model-toolcall-adapter-rs

[English](README.md) | [简体中文](README.zh-CN.md)

> 一个独立 Rust 适配器，让只会输出文本的模型也能接入 Codex 风格、OpenAI-compatible、Anthropic 风格的编程客户端。

`model-toolcall-adapter-rs` 对外提供 OpenAI-compatible 与 Anthropic 风格 HTTP 端点，把标准工具定义转换成稳定的文本协议，发给上游普通文本模型，再把模型输出的工具意图解析回标准工具调用响应。

它面向那些代码推理能力不错、但不稳定支持原生 function calling / tool calling 的模型和服务。

项目目标是对齐主流编程 agent 与编辑器工具：期望 OpenAI Responses 的 Codex 风格客户端、使用 Anthropic/Claude Messages 形态的客户端，以及可以配置 OpenAI-compatible `base_url` 的开发工具。

## 功能概览

- 提供 `POST /v1/chat/completions`，兼容 OpenAI Chat Completions 客户端。
- 提供 `POST /v1/responses`，并支持 retrieve、input-items、cancel、compact 等 Responses 端点。
- 提供 `POST /v1/messages`，兼容 Anthropic Messages 风格请求。
- 将 OpenAI function tools 转成模型可读的 XML/text 工具协议。
- 从纯文本模型输出中容错解析 XML、JSON 和常见 tool-call 形态。
- 支持 OpenAI-compatible 上游，例如本地 Ollama、vLLM、LM Studio、llama.cpp 风格 API。
- 内置 DeepSeek Web 上游 provider，支持本地 session 存储、PoW、SSE 解析、reasoning/text 分离。
- 内置 `/ui` 调试界面，用于测试模型、工具定义和 Responses 工具闭环。

本项目是独立仓库。构建和运行时不依赖 `../crates/aeon-claw-api`、`aeon-claw-cli` 或 FCACoreai workspace。

## 兼容目标

这个 adapter 是协议桥，不要求工具或编辑器专门适配本项目；只要客户端能说下面任一 HTTP 格式，就可以接入。

| 客户端类型 | 预期接口 | Adapter 端点 |
| --- | --- | --- |
| Codex 风格编程 agent | OpenAI Responses 风格 API | `/v1/responses` |
| OpenAI-compatible 编程工具 | Chat Completions API | `/v1/chat/completions` |
| Anthropic/Claude 风格客户端 | Messages 形态请求 | `/v1/messages` |
| 支持自定义 base URL 的编辑器/终端 agent | OpenAI-compatible `base_url` | `http://127.0.0.1:8787/v1` |
| 本地模型服务 | Ollama、vLLM、LM Studio、llama.cpp 风格 OpenAI API | 上游 `ADAPTER_UPSTREAM_BASE_URL` |

它适合作为 Codex-compatible CLI、Claude/Anthropic 风格 agent runtime、Cursor/Continue 类编辑器集成、Aider/OpenCode 类终端 agent，以及企业内部 agent 平台的本地协议桥。实际兼容性取决于客户端是否允许配置自定义 base URL，以及它使用的 wire format。

本项目不是 OpenAI、Anthropic、Cursor、Continue、Aider、Cline 或 OpenCode 的官方集成。它是一个本地协议适配层，用来帮助这些类型的工具连接只能返回纯文本的上游模型。

## 架构

```text
编程客户端 / Agent Runtime
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
上游模型
        |
        | OpenAI-compatible API
        | DeepSeek Web
        v
纯文本模型输出
```

核心模块保持小而清晰：

- `src/wire/*`：处理请求/响应 wire format 转换。
- `src/protocol/mod.rs`：渲染文本工具协议，并解析模型输出里的工具调用。
- `src/upstream.rs`：路由到 OpenAI-compatible 或 DeepSeek Web 上游。
- `src/deepseek_web.rs`：独立 DeepSeek Web 客户端实现。
- `src/responses_store.rs`：内存态 Responses 存储，用于 retrieve、input-items、cancel 和多轮续接。

## 快速开始

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

打开内置 UI：

```text
http://127.0.0.1:8787/ui
```

然后填写：

- Adapter API：`http://127.0.0.1:8787`
- Adapter API Key：`local-dev-key`
- Upstream API Base URL：你的 OpenAI-compatible 模型端点
- Model：上游真实模型名，或通过 `ADAPTER_MODEL_ALIASES` 暴露的别名

## 配置

每个 CLI 参数都有对应环境变量。

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

`ADAPTER_MODEL_ALIASES` 是逗号分隔的映射：

```text
外部模型名=上游真实模型名,另一个外部模型名=另一个上游真实模型名
```

例如：

```bash
export ADAPTER_MODEL_ALIASES=gpt-5-codex=deepseek-web/reasoner,gpt-5-mini=deepseek-web/chat
```

客户端可以请求 `model: "gpt-5-codex"`，adapter 会转发到 `deepseek-web/reasoner`，并在响应里恢复外部模型名。

## 鉴权

如果 `ADAPTER_API_KEY` 为空，adapter 端点不鉴权。

如果设置了 `ADAPTER_API_KEY`，请求需要携带以下任一 header：

```http
Authorization: Bearer local-dev-key
```

或：

```http
x-api-key: local-dev-key
```

每次请求都可以覆盖上游配置：

```http
x-upstream-base-url: https://api.example.com/v1
x-upstream-api-key: sk-...
x-upstream-provider: openai-compatible
```

DeepSeek Web 请求：

```http
x-upstream-provider: deepseek-web
x-deepseek-session: {"cookie":"...","bearer":"...","last_session_id":"..."}
```

如果省略 `x-deepseek-session`，adapter 会读取：

```text
~/.model-toolcall-adapter/deepseek_session.json
```

也可以设置 `ADAPTER_DEEPSEEK_SESSION_FILE` 指向其他路径。

## DeepSeek Web

DeepSeek Web 支持完全在当前仓库内实现。

UI 可以通过 `/deepseek-web/login` 打开 DeepSeek 登录页。登录后，把 Session JSON/Cookie 粘贴到 UI 并点击“登录/获取模型”，adapter 会保存到：

```text
~/.model-toolcall-adapter/deepseek_session.json
```

保存对象可以包含：

```json
{
  "cookie": "ds_session=...; ...",
  "bearer": "optional-token",
  "user_agent": "Mozilla/5.0 ...",
  "base_url": "https://chat.deepseek.com",
  "last_session_id": "optional-session-id"
}
```

DeepSeek Web 是非官方网页上游。如果网页服务调整私有端点、headers 或 PoW 行为，这个 provider 可能需要更新。

## 端点

| 端点 | 用途 |
| --- | --- |
| `GET /health` | 健康检查 |
| `GET /ui` | 内置调试 UI |
| `GET /v1/models` | 列出上游模型和别名模型 |
| `POST /v1/chat/completions` | OpenAI Chat Completions-compatible 请求 |
| `POST /v1/messages` | Anthropic Messages 风格请求 |
| `POST /v1/responses` | OpenAI Responses 风格创建 response |
| `GET /v1/responses/{response_id}` | 读取内存中的 response |
| `GET /v1/responses/{response_id}/input_items` | 列出保存的 response 输入项 |
| `POST /v1/responses/{response_id}/cancel` | 取消 background response |
| `POST /v1/responses/compact` | 压缩 response 上下文 |
| `POST /deepseek-web/login` | 打开 DeepSeek 登录页 |
| `POST /deepseek-web/session` | 本地保存 DeepSeek session |

为了兼容 base URL 直接指向 host 的客户端，Responses 相关路由也提供了不带 `/v1` 的版本。

## Chat Completions 示例

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

如果上游模型输出：

```xml
<tool_call id="call_1" name="get_weather">{"city":"北京"}</tool_call>
```

adapter 会在 Chat Completions 响应中返回标准 `tool_calls`。

## Responses 工具闭环

第一轮请求：

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

如果模型输出工具调用，adapter 会返回：

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

客户端执行工具后，继续请求：

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

adapter 会把上一轮输入、上一轮模型输出和新的工具结果拼入下一次上游 prompt。

## 内置工具

Responses 模式下，adapter 可以自动执行少量内置工具：

- `get_overview`
- `get_quote`

这些工具主要用于本地测试闭环行为。生产环境中的真实工具执行通常应该放在调用方 agent runtime 中。

## 开发

```bash
cargo fmt --check
cargo check
cargo test
```

本项目刻意保持为单个独立 Rust binary crate。

## 当前边界

已实现：

- 非流式 Chat Completions、Messages、Responses 兼容。
- Responses create、retrieve、input-items、cancel、compact 端点。
- `previous_response_id` 多轮续接。
- Responses 顶层 `function_call` 与 `function_call_output` 工具闭环。
- 模型别名。
- Adapter API key 鉴权。
- 按请求覆盖上游 base URL 和 API key。
- DeepSeek Web session 保存/读取、PoW、completion、文本解析。
- XML 与容错 JSON 工具调用解析。

暂未实现：

- Streaming 输出。
- DeepSeek Web 登录时自动提取浏览器 Cookie。
- 进程内存之外的持久 response 存储。
- 完整 `tool_choice` 行为。
- 生产级工具执行沙箱。

## License

MIT
