# 一、真实需求判断

- 表面需求：把 DeepSeek Web 的逻辑单独抽出来，做一个 Rust 项目的模型工具调用适配器。
- 真实目标：让不支持原生工具调用的模型，也能被上层客户端按 Completions、Messages、Responses 三种标准格式调用，并返回标准工具调用结构。
- 成功标准：客户端不用理解模型是否原生支持工具，只要按标准传 `tools`，适配器就能稳定返回文本答案或工具调用。
- 最小可交付版本：独立 Rust HTTP 服务，支持非流式三端点、工具提示词注入、上游 OpenAI-compatible 普通文本模型调用、工具调用解析、标准响应输出、API key 鉴权和最快可用前端。

---

# 二、现实条件分析

- 技术栈：Rust、Axum、Reqwest、Serde JSON。
- 运行环境：本机或服务器常驻 HTTP adapter，前面接 FCACoreai、OpenAI SDK 或其他 agent runtime。
- 时间限制：先做可验证 MVP，DeepSeek Web 会话和流式状态机延后。
- 数据规模：单次请求几十个工具以内，普通 agent 对话历史。
- 并发要求：MVP 可按普通 HTTP 并发处理；会话型 DeepSeek Web 后续需要 session state。
- 安全要求：不能执行工具；只转换工具调用意图。API key 和 cookie 只放配置/环境变量。
- 维护要求：协议层、wire 层、upstream 层分离，后续可以替换模型或增加 DeepSeek Web upstream。

---

# 三、因果链推演

用户行为
→ 客户端按三种标准格式之一传入 messages/tools
→ adapter 解析成统一请求模型
→ 工具定义渲染成文本协议
→ 上游不支持工具的模型按普通文本生成
→ adapter 解析 `<tool_call>` 或 JSON 工具意图
→ 有工具意图则转换成标准工具调用响应
→ 无工具意图则转换成普通文本响应
→ 客户端继续执行工具并把结果回传
→ adapter 把工具结果再次放进 prompt 继续对话

---

# 四、技术方案选择

## 推荐方案：

独立 Rust HTTP adapter，三层边界：

- `wire`：Completions、Messages、Responses 与统一请求/响应互转。
- `protocol`：文本工具协议 prompt 与解析。
- `upstream`：调用 DeepSeek Web、本地模型、OpenAI-compatible 模型。

## 不推荐方案：

- 不建议直接把 DeepSeek Web provider 从 FCACoreai 整个复制出来，里面混有桌面端、OCR、PoW、会话缓存和 runtime 类型。
- 不建议第一版就做工具执行器，否则 adapter 会变成 agent runtime，边界变重。
- 不建议第一版就做 streaming，工具参数跨 chunk 拼接、半截 XML、Responses SSE 事件都需要单独状态机。

## 为什么这样选：

当前目标是“让不支持工具调用的模型更好处理工具调用”，核心不是执行工具，而是协议转换。先把输入、提示、解析、输出四件事做稳，DeepSeek Web 只是其中一种上游来源。

---

# 五、系统结构设计

```text
model-toolcall-adapter-rs/
├── Cargo.toml
├── README.md
├── docs/
│   └── ARCHITECTURE.zh-CN.md
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── error.rs
│   ├── types.rs
│   ├── upstream.rs
│   ├── http/
│   │   ├── mod.rs
│   │   └── routes.rs
│   ├── protocol/
│   │   └── mod.rs
│   └── wire/
│       ├── mod.rs
│       ├── chat.rs
│       ├── messages.rs
│       └── responses.rs
└── tests/
```

---

# 六、前端最小闭环

```text
用户打开 /ui
→ 输入 Adapter API 与 Adapter API Key
→ 输入外部模型 API Base URL 与 API Key
→ 点击登录并获取模型
→ 前端调用 GET /v1/models
→ 选择模型
→ 填写 OpenAI Responses 标准 tools JSON
→ 在对话页发送
→ 前端调用 POST /v1/responses
→ adapter 返回 output_text 或 function_call
→ 前端展示工具调用与模拟工具结果
```

这个前端只做调试台，不做生产用户系统。生产环境应把工具执行放在业务 runtime 或 FCACoreai 内，不在浏览器里执行真实工具。外部模型 API Key 只保存在浏览器 `sessionStorage`，刷新会保留，本标签页关闭后丢弃。

---

# 七、OpenAI Responses 对齐边界

已按 OpenAI Responses Create 接口的核心结构对齐：

- 入参：`model`
- 入参：`instructions`
- 入参：`input`
- 入参：`tools`
- 入参：`max_output_tokens`
- 出参：`output_text` 等价的 message content
- 出参：`function_call`，包含 `call_id`、`name`、`arguments`

当前暂不支持 streaming、内置 web search/code interpreter 的真实执行、response 持久化、conversation id 和 background mode。这些不属于第一版工具调用适配器的必要因果链。
