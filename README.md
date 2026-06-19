# model-toolcall-adapter-rs

面向“不支持原生工具调用的模型”的 Rust 适配器。

它对外提供三种标准调用格式：

- `POST /v1/chat/completions`：OpenAI Chat Completions
- `POST /v1/messages`：Anthropic Messages 风格
- `POST /v1/responses`：OpenAI Responses 风格

Responses API 同时兼容以下 OpenAI 标准端点：

- `POST /v1/responses`：创建 response，支持 `input`、`instructions`、`tools`、`previous_response_id`、`background`。
- `GET /v1/responses/{response_id}`：读取内存中保存的 response。
- `GET /v1/responses/{response_id}/input_items`：按 `after`、`limit`、`order=asc|desc` 查询本次 response 的输入项。
- `POST /v1/responses/{response_id}/cancel`：取消 `background: true` 创建的 response。
- `POST /v1/responses/compact`：按 `previous_response_id` 和可选 `input` 返回压缩项列表。

为了方便非 `/v1` base URL 的客户端，服务也保留了同名的 `/responses...` 路由。当前存储是进程内内存，服务重启后历史 response 不会保留。

内部统一做一件事：把工具定义变成稳定的文本协议，发给上游普通文本模型，再把模型输出的工具意图解析回标准工具调用结构。

## 最小运行

```bash
cd model-toolcall-adapter-rs
cargo run -- \
  --bind 127.0.0.1:8787 \
  --upstream-base-url http://127.0.0.1:11434/v1 \
  --upstream-model qwen3-coder \
  --model-aliases codex-adapter=qwen3-coder \
  --adapter-api-key local-dev-key
```

也可以用环境变量：

```bash
export ADAPTER_UPSTREAM_BASE_URL=http://127.0.0.1:11434/v1
export ADAPTER_UPSTREAM_API_KEY=
export ADAPTER_UPSTREAM_MODEL=qwen3-coder
export ADAPTER_MODEL_ALIASES=codex-adapter=qwen3-coder
export ADAPTER_API_KEY=local-dev-key
cargo run
```

`ADAPTER_MODEL_ALIASES` 用来暴露外部自定义模型名，格式是：

```text
外部模型名=上游真实模型,另一个外部模型名=另一个上游真实模型
```

例如 DeepSeek Web：

```bash
cargo run -- \
  --bind 127.0.0.1:8787 \
  --upstream-model deepseek-web/reasoner \
  --model-aliases gpt-5-codex=deepseek-web/reasoner,gpt-5-mini=deepseek-web/chat \
  --adapter-api-key local-dev-key
```

外部用户可以请求 `model: "gpt-5-codex"`，adapter 会转发给 `deepseek-web/reasoner`，响应里仍显示 `gpt-5-codex`。

打开前端：

```text
http://127.0.0.1:8787/ui
```

前端包含：

- 登录区：填写 Adapter API 和 `ADAPTER_API_KEY`，调用 `/v1/models` 获取模型。
- 外部模型 API 区：填写上游 OpenAI-compatible Base URL 和 API Key。
- DeepSeek Web：选择上游类型为 `DeepSeek Web`，可粘贴 DeepSeek session JSON/Cookie；留空时后端尝试读取 `~/.FCACore/deepseek_session.json`。
- 对话区：走 `/v1/responses`。
- 工具区：按 OpenAI Responses `tools` 标准填写 function tool schema。

如果 `ADAPTER_API_KEY` 为空，`/v1/*` 不鉴权；如果设置了，则支持：

```http
Authorization: Bearer local-dev-key
```

或：

```http
x-api-key: local-dev-key
```

每次请求也支持覆盖上游模型服务：

```http
x-upstream-base-url: https://api.openai.com/v1
x-upstream-api-key: sk-...
```

这两个头只影响当前请求；没有传时使用服务启动参数或环境变量里的 `ADAPTER_UPSTREAM_BASE_URL` / `ADAPTER_UPSTREAM_API_KEY`。

DeepSeek Web 请求使用：

```http
x-upstream-provider: deepseek-web
x-deepseek-session: {"cookie":"...","bearer":"...","last_session_id":"..."}
```

`x-deepseek-session` 可省略，省略时读取 `~/.FCACore/deepseek_session.json`。如果返回 invalid token，需要重新登录 DeepSeek Web 刷新该文件。

## Chat Completions 示例

```bash
curl http://127.0.0.1:8787/v1/chat/completions \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer local-dev-key' \
  -d '{
    "model": "qwen3-coder",
    "messages": [{"role":"user","content":"查一下北京天气"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get weather by city",
        "parameters": {
          "type": "object",
          "properties": {"city": {"type":"string"}},
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

适配器会返回标准 `tool_calls`。

## Responses 工具调用闭环

外部客户端第一轮按 OpenAI Responses 标准把工具定义发给当前服务：

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
        "properties": {"city": {"type":"string"}},
        "required": ["city"]
      }
    }]
  }'
```

如果上游模型按协议输出：

```xml
<tool_call id="call_1" name="get_weather">{"city":"北京"}</tool_call>
```

当前服务会返回 OpenAI Responses 风格的工具调用：

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

如果工具是当前服务内置工具，例如 `get_overview` / `get_quote`，adapter 会自动执行工具，把结果继续喂给上游模型，并在同一个 response 里返回工具调用、工具结果和最终回答。

如果工具不是当前服务内置工具，外部客户端仍按 OpenAI Responses 标准执行工具后，把工具结果继续发给 `/v1/responses`：

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

当前服务会把上一轮输入、上一轮模型工具调用、本轮工具结果一起拼给上游模型，让没有原生工具调用能力的模型完成后续回答。

## 与 FCACoreai 的参考集成

现有仓库里可以参考三处逻辑：

- `crates/aeon-claw-api/src/providers/deepseek_web.rs`：Web 会话、Prompt 拼接、SSE delta 解析、thinking/text 分流。
- `crates/aeon-claw-api/src/providers/openai_compat.rs`：Chat Completions 与 Responses 的 wire 格式转换。
- `crates/aeon-claw-runtime/src/tool_protocol.rs`：文本工具调用协议与容错解析。

本项目不直接耦合 FCACoreai runtime。后续接入时推荐让 FCACoreai 把它当作一个 OpenAI-compatible base URL：

```json
{
  "api": {
    "provider": "openai_compat",
    "baseUrl": "http://127.0.0.1:8787/v1",
    "model": "qwen3-coder",
    "toolMode": "native"
  }
}
```

## 当前边界

- 已支持非流式三格式输入输出。
- 已支持 OpenAI Responses create/retrieve/input_items/cancel/compact 的非流式兼容端点。
- 已支持 `previous_response_id` 多轮续接：服务会把上一轮输入和模型输出拼入下一轮上下文。
- 已支持 Responses 顶层 `function_call` / `function_call_output` 工具闭环。
- 已支持后端自动执行内置工具：`get_overview` / `get_quote`。
- 已内置最快可用前端：登录拉模型、Responses 对话、工具 JSON 配置。
- 已支持 adapter API key 鉴权。
- 已支持按请求覆盖外部 OpenAI-compatible API base URL 和 key。
- 已支持 DeepSeek Web 单轮测试 provider：读取 session、创建/复用会话、PoW、completion、文本解析。
- 已支持 XML `<tool_call>` 与单个 JSON `tool_call/function_call` 解析。
- 暂不执行工具，只负责“模型文本输出 -> 标准工具调用输出”的桥接。
- 暂不支持 streaming，避免第一版把 SSE 状态机和工具参数增量拼接引入过早复杂度。

## 后续迭代

1. 增加 streaming：把文本 delta 缓冲到完整工具调用，再输出对应 SSE 事件。
2. 增强 DeepSeek Web upstream：持久化 parent_message_id、完善流式输出、增加一键刷新登录。
3. 增加工具协议方言：`aeon_tool_call`、`tool_call`、纯 JSON、Markdown fenced JSON。
4. 增加 tool_choice：支持 `auto`、`required`、指定工具。
5. 增加请求预算：限制工具数量、schema 长度、历史轮次，防止 prompt 失控。
