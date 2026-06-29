# model-toolcall-adapter-rs

[English](README.md) | [简体中文](README.zh-CN.md)

> 一个本地协议适配器：把“不原生支持工具调用”的上游模型，包装成 Codex / OpenAI / Anthropic 客户端能识别的标准工具调用接口。

当前发布版本：`v0.2.0`

这个项目不执行用户工具，也不试图做一个完整 agent runtime。它只解决一个明确问题：

```text
客户端传入标准 tools
→ adapter 把工具定义转成上游模型能理解的文本协议
→ 上游模型按普通文本输出工具意图
→ adapter 把文本工具意图解析成标准 function_call / tool_calls
→ 客户端或业务 runtime 执行工具并回传结果
```

这条边界很重要。adapter 负责“让模型说出标准工具调用”，真实工具仍由你的 Codex、客户端、后端服务或业务 runtime 执行。

## 适合什么场景

适合：

- 你有一个代码/推理能力不错的模型，但它不稳定支持 OpenAI function calling。
- 你要把 DeepSeek Web、Ollama、vLLM、LM Studio、llama.cpp 等上游接到 Codex 风格客户端。
- 你的客户端已经会传 OpenAI `tools`，但上游只能返回普通文本。
- 你需要 OpenAI Responses、Chat Completions、Anthropic Messages 三种入口做协议桥接。

不适合：

- 你想让 adapter 直接执行 shell、浏览器、数据库或业务函数。
- 你需要 OpenAI 托管 `file_search` / vector store 的完整替代品。
- 你需要完全等价的 OpenAI 服务端 Structured Outputs / 加密 reasoning token。
- 你希望多个远程服务节点共享同一套 response 状态。当前状态存储是本机文件级别。

## 当前能力

| 能力 | 状态 |
| --- | --- |
| OpenAI Chat Completions | 支持 `POST /v1/chat/completions` |
| OpenAI Responses | 支持 create / retrieve / delete / input_items / input_tokens / cancel / compact |
| Conversations | 支持 create / retrieve / update / delete / items |
| Anthropic Messages | 支持 `POST /v1/messages` 形态 |
| 工具调用适配 | 将普通文本中的 XML / JSON / 容错格式解析成标准工具调用 |
| 工具执行 | 不执行，只返回标准工具调用，等待调用方回传 tool output |
| Streaming | Responses 支持真实 SSE 增量；Chat / Messages 暂未做真实增量 |
| Long task | Responses `background: true` 可 retrieve 轮询和 cancel |
| Structured output | 支持 `json_object` 和常见递归 `json_schema` 校验，不是完整 JSON Schema 引擎 |
| Reasoning | 支持 reasoning/text 分离；`reasoning.encrypted_content` 为本地 opaque 占位/透传 |
| 图片/文件 | 支持标准 input image/file；DeepSeek Web provider 会上传并转成私有引用 |
| DeepSeek Web | 支持受控浏览器登录、session 捕获、PoW、SSE 解析、搜索/思考/专家/识图模式映射 |
| Codex | 支持一键备份并写入 `~/.codex/config.toml` 和 `auth.json` |
| 本地状态 | response/conversation JSON 持久化，带 lock 文件和原子替换 |

## 架构

```text
Codex / SDK / Agent Runtime
        |
        | OpenAI Responses
        | Chat Completions
        | Anthropic Messages
        v
model-toolcall-adapter-rs
        |
        | tools -> 文本工具协议
        | 普通文本 -> 标准工具调用
        v
Upstream Provider
        |
        | OpenAI-compatible API
        | DeepSeek Web
        v
实际模型
```

目录边界：

| 路径 | 职责 |
| --- | --- |
| `src/wire/` | 三种外部 wire format 与内部统一请求模型互转 |
| `src/protocol/` | 工具协议 prompt 渲染与模型文本解析 |
| `src/providers/openai_compat.rs` | OpenAI-compatible 上游 |
| `src/providers/deepseek_web/` | DeepSeek Web 登录、session、PoW、上传、completion、SSE |
| `src/responses_store.rs` | Responses / Conversations 本地状态 |
| `src/http/routes.rs` | HTTP 路由、鉴权、Codex setup、Responses 状态机 |
| `src/ui.rs` | 本地启动向导页面 |

## 启动

### 从源码运行

```bash
git clone https://github.com/openaeon/model-toolcall-adapter-rs.git
cd model-toolcall-adapter-rs
cargo run
```

打开：

```text
http://127.0.0.1:8787/ui
```

端口占用时：

```bash
ADAPTER_BIND=127.0.0.1:8899 cargo run
```

### 使用发行包

发行包目录约定：

```text
dist/packages/
├── model-toolcall-adapter-rs-v0.2.0-windows-x64-exe.zip
├── model-toolcall-adapter-rs-v0.2.0-macos-arm64.tar.gz
├── model-toolcall-adapter-rs-v0.2.0-linux-x64-server.tar.gz
├── model-toolcall-adapter-rs-v0.2.0-linux-arm64-server.tar.gz
└── SHA256SUMS.txt
```

`SHA256SUMS.txt` 用于校验四个平台压缩包。仓库只提交压缩发行包和校验文件，不提交解压后的临时工作目录。

Windows：

```powershell
Expand-Archive .\model-toolcall-adapter-rs-v0.2.0-windows-x64-exe.zip
cd .\model-toolcall-adapter-rs-windows-x64-exe\model-toolcall-adapter-rs-windows-x64
.\model-toolcall-adapter-rs.exe
```

Windows 终端不会自动从当前目录查找程序，必须带 `.\` 前缀。

macOS：

```bash
tar -xzf model-toolcall-adapter-rs-v0.2.0-macos-arm64.tar.gz
cd model-toolcall-adapter-rs-macos-arm64
chmod +x ./model-toolcall-adapter-rs
./model-toolcall-adapter-rs
```

Linux：

```bash
tar -xzf model-toolcall-adapter-rs-v0.2.0-linux-x64-server.tar.gz
cd model-toolcall-adapter-rs-linux-x64
chmod +x ./model-toolcall-adapter-rs
./model-toolcall-adapter-rs
```

## 首次配置

首次启动会创建本地配置：

```text
~/.model-toolcall-adapter/config.json
```

其中会生成随机 `adapter_api_key`，用于保护本地接口。

打开 `/ui` 后按三步走：

1. 选择 provider：`openai-compatible` 或 `deepseek-web`。
2. 如果选择 DeepSeek Web，启动受控浏览器并登录 `https://chat.deepseek.com/`。
3. 捕获 session，查看 Base URL、Adapter Key、模型名和请求示例。

截图：

![启动向导](docs/assets/setup-wizard.png)

## 接入 Codex

启动向导第三步可以“一键配置 Codex”。它会：

- 备份 `~/.codex/config.toml` 和 `~/.codex/auth.json`。
- 写入 `model_provider = "ModelToolCallAdapter"`。
- 写入 Responses wire provider。
- 将 adapter key 写到 `OPENAI_API_KEY`。

写入后的核心配置类似：

```toml
model_provider = "ModelToolCallAdapter"

[model_providers.ModelToolCallAdapter]
name = "ModelToolCallAdapter"
base_url = "http://127.0.0.1:8787/v1"
wire_api = "responses"
requires_openai_auth = true
```

`auth.json`：

```json
{
  "OPENAI_API_KEY": "adp_xxx"
}
```

Codex 已经运行时，配置后需要重启 Codex。

## Provider

### OpenAI-compatible

适用于 Ollama、vLLM、LM Studio、llama.cpp server、兼容 OpenAI Chat Completions 的自建服务。

```bash
ADAPTER_UPSTREAM_BASE_URL=http://127.0.0.1:11434/v1 \
ADAPTER_UPSTREAM_MODEL=qwen3-coder \
cargo run
```

也可以配置模型别名：

```bash
ADAPTER_MODEL_ALIASES=gpt-5-codex=qwen3-coder,gpt-5-mini=qwen3-fast cargo run
```

客户端请求 `gpt-5-codex` 时，adapter 会转发到 `qwen3-coder`，响应里再恢复外部模型名。

### DeepSeek Web

DeepSeek Web provider 是非官方网页上游。它只读取 adapter 自己启动的受控浏览器 profile，不读取用户普通浏览器隐私数据。

支持模型名：

| 模型 | 含义 |
| --- | --- |
| `deepseek-web/reasoner` | 深度思考 |
| `deepseek-web/chat` | 普通聊天 |
| `deepseek-web/search` | 搜索开关 |
| `deepseek-web/expert` | 专家模式 |
| `deepseek-web/vision` | 识图/文件通道 |

session 默认保存到：

```text
~/.model-toolcall-adapter/deepseek_session.json
```

可用环境变量覆盖：

```bash
ADAPTER_DEEPSEEK_SESSION_FILE=/path/to/deepseek_session.json cargo run
```

DeepSeek Web 如果调整私有接口、headers、PoW 或 SSE 格式，provider 需要同步更新。

## API 示例

### Responses 工具调用

```bash
curl http://127.0.0.1:8787/v1/responses \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer adp_xxx' \
  -d '{
    "model": "deepseek-web/reasoner",
    "input": "需要外部信息时先发起工具调用",
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

adapter 可能返回：

```json
{
  "object": "response",
  "status": "completed",
  "output": [{
    "type": "function_call",
    "status": "completed",
    "call_id": "call_1",
    "name": "search_web",
    "arguments": "{\"query\":\"...\"}"
  }]
}
```

调用方执行工具后继续：

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
      "output": "工具执行结果"
    }]
  }'
```

### Chat Completions 工具调用

```bash
curl http://127.0.0.1:8787/v1/chat/completions \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer adp_xxx' \
  -d '{
    "model": "deepseek-web/chat",
    "messages": [{ "role": "user", "content": "查一下北京天气" }],
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

### Responses 长任务

流式：

```json
{
  "model": "deepseek-web/reasoner",
  "input": "分析这个项目",
  "stream": true
}
```

后台轮询：

```json
{
  "model": "deepseek-web/reasoner",
  "input": "执行一个较长分析",
  "background": true
}
```

后台任务会先返回 `status: "in_progress"`。随后用：

```bash
curl http://127.0.0.1:8787/v1/responses/resp_xxx \
  -H 'authorization: Bearer adp_xxx'
```

取消：

```bash
curl -X POST http://127.0.0.1:8787/v1/responses/resp_xxx/cancel \
  -H 'authorization: Bearer adp_xxx'
```

## 图片、文件与本地 file_search

外部请求继续使用标准格式：

Responses：

```json
{
  "model": "deepseek-web/vision",
  "input": [{
    "type": "message",
    "role": "user",
    "content": [
      { "type": "input_text", "text": "看这张图" },
      { "type": "input_image", "image_url": "data:image/png;base64,..." }
    ]
  }]
}
```

Chat Completions：

```json
{
  "messages": [{
    "role": "user",
    "content": [
      { "type": "text", "text": "总结文件" },
      { "type": "file", "file": { "filename": "note.txt", "file_data": "..." } }
    ]
  }]
}
```

DeepSeek Web provider 会在内部上传附件并轮询解析状态，再把上传得到的 id 传给 DeepSeek。专家模式不直接支持文件引用；带文件请求会先走识图/文件通道解析，再桥接到合适的模型模式。

Responses `tools:[{"type":"file_search"}]` 当前只搜索本次请求内可读的 `input_file.file_data` 文本，不读取任意本地文件，也不是持久向量库。

## 配置

| 配置 | 环境变量 | 默认值 |
| --- | --- | --- |
| 监听地址 | `ADAPTER_BIND` | `127.0.0.1:8787` |
| OpenAI-compatible 上游地址 | `ADAPTER_UPSTREAM_BASE_URL` | `http://127.0.0.1:11434/v1` |
| 上游 API key | `ADAPTER_UPSTREAM_API_KEY` | 空 |
| 默认上游模型 | `ADAPTER_UPSTREAM_MODEL` | `local-model` |
| 模型别名 | `ADAPTER_MODEL_ALIASES` | 空 |
| Adapter API key | `ADAPTER_API_KEY` | 读取/生成本地配置 |
| 最大工具数量 | `ADAPTER_MAX_TOOL_DEFINITIONS` | `64` |
| 请求超时 | `ADAPTER_REQUEST_TIMEOUT_SECS` | `120` |
| 本地配置文件 | `ADAPTER_CONFIG_FILE` | `~/.model-toolcall-adapter/config.json` |
| Response store | `ADAPTER_RESPONSE_STORE_FILE` | `~/.model-toolcall-adapter/responses_store.json` |
| Conversation store | `ADAPTER_CONVERSATION_STORE_FILE` | `~/.model-toolcall-adapter/conversations_store.json` |
| DeepSeek session | `ADAPTER_DEEPSEEK_SESSION_FILE` | `~/.model-toolcall-adapter/deepseek_session.json` |

请求级覆盖：

```http
x-upstream-provider: deepseek-web
x-upstream-base-url: https://api.example.com/v1
x-upstream-api-key: sk-...
x-deepseek-session: {"cookie":"..."}
```

## 本地文件

| 文件 | 用途 |
| --- | --- |
| `~/.model-toolcall-adapter/config.json` | provider、adapter key、模型别名 |
| `~/.model-toolcall-adapter/deepseek_session.json` | DeepSeek Web session |
| `~/.model-toolcall-adapter/responses_store.json` | Responses 状态 |
| `~/.model-toolcall-adapter/responses_store.json.lock` | Response store 文件锁 |
| `~/.model-toolcall-adapter/conversations_store.json` | Conversations 状态 |
| `~/.model-toolcall-adapter/conversations_store.json.lock` | Conversation store 文件锁 |

Response / Conversation store 仍是可读 JSON。写入时使用 lock 文件和临时文件原子替换，降低多个本地 adapter 进程或异常退出造成的状态损坏风险。

## 端点

| 端点 | 用途 |
| --- | --- |
| `GET /health` | 健康检查 |
| `GET /ui` | 启动向导 |
| `GET /v1/models` | 模型列表 |
| `POST /v1/chat/completions` | Chat Completions |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/responses` | Responses create |
| `GET /v1/responses/{id}` | Retrieve response |
| `DELETE /v1/responses/{id}` | Delete response |
| `GET /v1/responses/{id}/input_items` | Response input items |
| `POST /v1/responses/{id}/cancel` | Cancel background response |
| `POST /v1/responses/input_tokens` | 估算 input tokens |
| `POST /v1/responses/compact` | Compact response context |
| `POST /v1/conversations` | Create conversation |
| `GET /v1/conversations/{id}` | Retrieve conversation |
| `POST /v1/conversations/{id}` | Update conversation metadata |
| `DELETE /v1/conversations/{id}` | Delete conversation |
| `GET /v1/conversations/{id}/items` | List conversation items |
| `POST /v1/conversations/{id}/items` | Append conversation items |
| `GET /v1/conversations/{id}/items/{item_id}` | Retrieve conversation item |
| `DELETE /v1/conversations/{id}/items/{item_id}` | Delete conversation item |
| `GET /setup/state` | 启动向导状态 |
| `POST /setup/provider` | 保存 provider 选择 |
| `POST /setup/deepseek-browser/start` | 启动 DeepSeek 登录浏览器 |
| `POST /setup/deepseek-browser/capture` | 捕获 DeepSeek session |
| `POST /setup/codex/apply` | 写入 Codex 配置 |

为了兼容 base URL 指到 host 根路径的客户端，Responses 和 Conversations 也提供不带 `/v1` 的同名路由。

## 打包

准备 target：

```bash
rustup target add aarch64-apple-darwin
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-unknown-linux-musl
cargo install cargo-zigbuild
brew install zig
```

构建：

```bash
cargo build --release --target aarch64-apple-darwin
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-musl
cargo zigbuild --release --target x86_64-pc-windows-gnu
```

打包示例：

```bash
mkdir -p dist/packages \
  dist/work/model-toolcall-adapter-rs-macos-arm64 \
  dist/work/model-toolcall-adapter-rs-linux-x64 \
  dist/work/model-toolcall-adapter-rs-linux-arm64 \
  dist/work/model-toolcall-adapter-rs-windows-x64

cp target/aarch64-apple-darwin/release/model-toolcall-adapter-rs \
  dist/work/model-toolcall-adapter-rs-macos-arm64/model-toolcall-adapter-rs
cp target/x86_64-unknown-linux-musl/release/model-toolcall-adapter-rs \
  dist/work/model-toolcall-adapter-rs-linux-x64/model-toolcall-adapter-rs
cp target/aarch64-unknown-linux-musl/release/model-toolcall-adapter-rs \
  dist/work/model-toolcall-adapter-rs-linux-arm64/model-toolcall-adapter-rs
cp target/x86_64-pc-windows-gnu/release/model-toolcall-adapter-rs.exe \
  dist/work/model-toolcall-adapter-rs-windows-x64/model-toolcall-adapter-rs.exe

(cd dist/work && tar -czf ../packages/model-toolcall-adapter-rs-v0.2.0-macos-arm64.tar.gz model-toolcall-adapter-rs-macos-arm64)
(cd dist/work && tar -czf ../packages/model-toolcall-adapter-rs-v0.2.0-linux-x64-server.tar.gz model-toolcall-adapter-rs-linux-x64)
(cd dist/work && tar -czf ../packages/model-toolcall-adapter-rs-v0.2.0-linux-arm64-server.tar.gz model-toolcall-adapter-rs-linux-arm64)
(cd dist/work && zip -qr ../packages/model-toolcall-adapter-rs-v0.2.0-windows-x64-exe.zip model-toolcall-adapter-rs-windows-x64)
shasum -a 256 dist/packages/* > dist/packages/SHA256SUMS.txt
```

## 开发验证

```bash
cargo fmt -- --check
cargo test
cargo build
```

常见问题：

| 现象 | 处理 |
| --- | --- |
| `Address already in use` | 停掉旧进程，或用 `ADAPTER_BIND=127.0.0.1:8899 cargo run` |
| Windows 提示不是内部或外部命令 | 在 exe 所在目录使用 `.\model-toolcall-adapter-rs.exe` |
| DeepSeek 登录浏览器输出 GCM/DEPRECATED_ENDPOINT | 通常是 Chrome 后台服务日志，不等于登录失败 |
| `/v1/models` 不返回 DeepSeek 模型 | 先在 `/ui` 捕获并保存 DeepSeek session |
| Codex 不走 adapter | 重启 Codex，检查 `~/.codex/config.toml` 和 `auth.json` |

## 边界

- adapter 不执行用户业务工具。
- adapter 不读取用户普通浏览器 cookie，只读取自己启动的受控浏览器 profile。
- `reasoning.encrypted_content` 是本地 opaque 占位/透传，不是 OpenAI 服务端真实加密。
- `json_schema` 校验覆盖常见结构化输出约束，不是完整 JSON Schema 标准实现。
- DeepSeek Web provider 依赖网页私有接口，服务端变更时可能需要更新。

## License

MIT
