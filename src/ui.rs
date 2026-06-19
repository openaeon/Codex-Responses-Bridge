pub const INDEX_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Model Tool Call Adapter</title>
  <style>
    :root {
      color-scheme: dark;
      --bg: #0d1117;
      --panel: #151b23;
      --panel2: #0f1620;
      --line: #30363d;
      --text: #e6edf3;
      --muted: #8b949e;
      --accent: #2f81f7;
      --ok: #3fb950;
      --bad: #f85149;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      background: var(--bg);
      color: var(--text);
      font: 14px/1.45 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    main {
      min-height: 100vh;
      display: grid;
      grid-template-columns: 360px minmax(0, 1fr);
    }
    aside {
      border-right: 1px solid var(--line);
      background: var(--panel);
      padding: 18px;
      overflow: auto;
    }
    section {
      padding: 18px;
      display: grid;
      grid-template-rows: auto minmax(0, 1fr) auto;
      gap: 12px;
      min-width: 0;
    }
    h1 { font-size: 18px; margin: 0 0 4px; }
    h2 { font-size: 13px; margin: 18px 0 8px; color: var(--muted); text-transform: uppercase; }
    label { display: block; margin: 10px 0 5px; color: var(--muted); font-size: 12px; }
    input, select, textarea {
      width: 100%;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: var(--panel2);
      color: var(--text);
      padding: 9px 10px;
      outline: none;
      font: inherit;
    }
    textarea {
      min-height: 130px;
      resize: vertical;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 12px;
    }
    button {
      border: 1px solid var(--line);
      border-radius: 6px;
      background: var(--accent);
      color: white;
      padding: 9px 12px;
      cursor: pointer;
      font-weight: 650;
    }
    button.secondary { background: transparent; color: var(--text); }
    button:disabled { opacity: .55; cursor: not-allowed; }
    .row { display: flex; gap: 8px; align-items: center; }
    .row > * { flex: 1; }
    .status { margin-top: 10px; min-height: 20px; color: var(--muted); font-size: 12px; }
    .status.ok { color: var(--ok); }
    .status.bad { color: var(--bad); }
    .chat {
      overflow: auto;
      border: 1px solid var(--line);
      border-radius: 8px;
      background: var(--panel);
      padding: 12px;
      min-height: 0;
    }
    .msg {
      border-bottom: 1px solid rgba(48,54,61,.65);
      padding: 10px 0;
      white-space: pre-wrap;
      word-break: break-word;
    }
    .msg:last-child { border-bottom: 0; }
    .role { color: var(--muted); font-size: 12px; margin-bottom: 4px; }
    .tool {
      margin-top: 8px;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 8px;
      background: #101820;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 12px;
    }
    .composer {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 8px;
    }
    .composer textarea { min-height: 72px; }
    @media (max-width: 820px) {
      main { grid-template-columns: 1fr; }
      aside { border-right: 0; border-bottom: 1px solid var(--line); }
      section { min-height: 65vh; }
    }
  </style>
</head>
<body>
  <main>
    <aside>
      <h1>Tool Call Adapter</h1>
      <div class="status">OpenAI Responses 标准工具调用调试台</div>

      <h2>登录 / 模型</h2>
      <label>Adapter API</label>
      <input id="adapterUrl" value="http://127.0.0.1:8787" />
      <label>Adapter API Key</label>
      <input id="adapterKey" type="password" placeholder="本地测试通常填 local-dev-key；若 ADAPTER_API_KEY 为空可不填" />
      <label>上游类型</label>
      <select id="providerSelect">
        <option value="openai-compatible">OpenAI-compatible</option>
        <option value="deepseek-web">DeepSeek Web</option>
      </select>
      <label>外部模型 API Base URL</label>
      <input id="upstreamUrl" value="http://127.0.0.1:11434/v1" placeholder="https://api.openai.com/v1" />
      <label>外部模型 API Key</label>
      <input id="upstreamKey" type="password" placeholder="上游模型服务 key，可为空" />
      <label>DeepSeek Web Session JSON / Cookie</label>
      <textarea id="deepseekSession" placeholder="可留空，后端会尝试读取 ~/.model-toolcall-adapter/deepseek_session.json；如果提示 invalid token，请重新登录 DeepSeek Web 后粘贴并保存新的 Session"></textarea>
      <div class="row" style="margin-top:10px">
        <button id="loginBtn">登录并获取模型</button>
        <button id="deepseekLoginBtn" class="secondary">刷新 DeepSeek 登录</button>
        <button id="clearBtn" class="secondary">清空</button>
      </div>
      <div id="loginStatus" class="status"></div>

      <label>模型</label>
      <select id="modelSelect"></select>

      <h2>Responses 参数</h2>
      <label>Instructions</label>
      <textarea id="instructions">你是一个会按需调用工具的助手。需要查询港股行情、公司概况或价格时，必须先调用工具，不要直接编造数据。</textarea>
      <label>Tools JSON</label>
      <textarea id="toolsJson">[
  {
    "type": "function",
    "name": "get_overview",
    "description": "Get HK stock overview by symbol",
    "parameters": {
      "type": "object",
      "properties": {
        "symbol": { "type": "string", "description": "Ticker like 0700.HK" },
        "market": { "type": "string", "enum": ["hk"], "default": "hk" },
        "modules": {
          "type": "array",
          "items": { "type": "string" },
          "default": ["price"]
        },
        "base_url": {
          "type": "string",
          "default": "http://47.238.165.205:8009/overview"
        }
      },
      "required": ["symbol"]
    }
  }
]</textarea>
      <label>Max Output Tokens</label>
      <input id="maxTokens" type="number" value="1024" min="1" max="32000" />
    </aside>

    <section>
      <div>
        <h1>对话</h1>
        <div class="status">发送后走 <code>/v1/responses</code>，返回文本或标准 <code>function_call</code>。</div>
      </div>
      <div id="chat" class="chat"></div>
      <div class="composer">
        <textarea id="input" placeholder="例如：查一下北京天气"></textarea>
        <button id="sendBtn">发送</button>
      </div>
    </section>
  </main>

  <script>
    const $ = (id) => document.getElementById(id);
    const state = {
      input: []
    };

    function load() {
      $("adapterUrl").value = localStorage.getItem("adapterUrl") || $("adapterUrl").value;
      $("adapterKey").value = localStorage.getItem("adapterKey") || "";
      $("providerSelect").value = localStorage.getItem("provider") || "openai-compatible";
      $("upstreamUrl").value = localStorage.getItem("upstreamUrl") || $("upstreamUrl").value;
      $("upstreamKey").value = sessionStorage.getItem("upstreamKey") || "";
      $("deepseekSession").value = sessionStorage.getItem("deepseekSession") || "";
      $("modelSelect").innerHTML = `<option value="${localStorage.getItem("model") || "local-model"}">${localStorage.getItem("model") || "local-model"}</option>`;
      updateProviderMode();
      render();
    }

    function headers() {
      const h = { "content-type": "application/json" };
      const key = $("adapterKey").value.trim();
      if (key) h.authorization = `Bearer ${key}`;
      const provider = $("providerSelect").value;
      const upstreamUrl = $("upstreamUrl").value.trim();
      const upstreamKey = $("upstreamKey").value.trim();
      const deepseekSession = $("deepseekSession").value.trim();
      if (provider) h["x-upstream-provider"] = provider;
      if (upstreamUrl) h["x-upstream-base-url"] = upstreamUrl;
      if (upstreamKey) h["x-upstream-api-key"] = upstreamKey;
      if (deepseekSession) h["x-deepseek-session"] = deepseekSession;
      return h;
    }

    function adapterUrl(path) {
      return `${$("adapterUrl").value.replace(/\/$/, "")}${path}`;
    }

    function setStatus(id, text, kind = "") {
      const el = $(id);
      el.textContent = text;
      el.className = `status ${kind}`;
    }

    async function login() {
      localStorage.setItem("adapterUrl", $("adapterUrl").value.trim());
      localStorage.setItem("adapterKey", $("adapterKey").value);
      localStorage.setItem("provider", $("providerSelect").value);
      localStorage.setItem("upstreamUrl", $("upstreamUrl").value.trim());
      sessionStorage.setItem("upstreamKey", $("upstreamKey").value);
      sessionStorage.setItem("deepseekSession", $("deepseekSession").value);
      setStatus("loginStatus", "正在获取模型...");
      try {
        if ($("providerSelect").value === "deepseek-web" && $("deepseekSession").value.trim()) {
          const saveRes = await fetch(adapterUrl("/deepseek-web/session"), {
            method: "POST",
            headers: headers(),
            body: JSON.stringify({ session: $("deepseekSession").value.trim() })
          });
          const saveBody = await saveRes.json();
          if (!saveRes.ok) throw new Error(saveBody?.error?.message || saveRes.statusText);
        }
        const res = await fetch(adapterUrl("/v1/models"), { headers: headers() });
        const body = await res.json();
        if (!res.ok) throw new Error(body?.error?.message || res.statusText);
        const models = Array.isArray(body.data) ? body.data.map((m) => m.id).filter(Boolean) : [];
        if (!models.length) throw new Error("模型列表为空");
        $("modelSelect").innerHTML = models.map((id) => `<option value="${escapeHtml(id)}">${escapeHtml(id)}</option>`).join("");
        const savedModel = localStorage.getItem("model");
        $("modelSelect").value = models.includes(savedModel) ? savedModel : models[0];
        localStorage.setItem("model", $("modelSelect").value);
        setStatus("loginStatus", `已获取 ${models.length} 个模型`, "ok");
      } catch (err) {
        setStatus("loginStatus", err.message || String(err), "bad");
      }
    }

    async function refreshDeepSeekLogin() {
      localStorage.setItem("adapterUrl", $("adapterUrl").value.trim());
      localStorage.setItem("adapterKey", $("adapterKey").value);
      setStatus("loginStatus", "正在打开 DeepSeek 登录窗口...");
      $("deepseekLoginBtn").disabled = true;
      try {
        const res = await fetch(adapterUrl("/deepseek-web/login"), {
          method: "POST",
          headers: headers()
        });
        const body = await res.json();
        if (!res.ok) throw new Error(body?.error?.message || res.statusText);
        $("deepseekSession").value = "";
        sessionStorage.removeItem("deepseekSession");
        setStatus("loginStatus", `DeepSeek 登录页已打开。登录后粘贴 Session 并点“登录并获取模型”：${body.session_file || ""}`, "ok");
      } catch (err) {
        setStatus("loginStatus", err.message || String(err), "bad");
      } finally {
        $("deepseekLoginBtn").disabled = false;
      }
    }

    async function send() {
      const text = $("input").value.trim();
      if (!text) return;
      const tools = parseTools();
      const model = $("modelSelect").value || "local-model";
      localStorage.setItem("model", model);
      state.input.push({ role: "user", content: [{ type: "input_text", text }] });
      $("input").value = "";
      render();

      $("sendBtn").disabled = true;
      try {
        const payload = {
          model,
          instructions: $("instructions").value,
          input: state.input,
          tools,
          max_output_tokens: Number($("maxTokens").value || 1024),
          stream: false
        };
        const res = await fetch(adapterUrl("/v1/responses"), {
          method: "POST",
          headers: headers(),
          body: JSON.stringify(payload)
        });
        const body = await res.json();
        if (!res.ok) throw new Error(body?.error?.message || res.statusText);
        await applyResponse(body);
      } catch (err) {
        state.input.push({ role: "assistant", content: [{ type: "output_text", text: `Error: ${err.message || err}` }] });
      } finally {
        $("sendBtn").disabled = false;
        render();
      }
    }

    function parseTools() {
      const raw = $("toolsJson").value.trim();
      if (!raw) return [];
      try {
        const parsed = JSON.parse(raw);
        return Array.isArray(parsed) ? parsed : [parsed];
      } catch (err) {
        throw new Error(`Tools JSON 格式错误：${err.message}`);
      }
    }

    async function applyResponse(body) {
      const serverHandledCalls = new Set(
        (body.output || [])
          .filter((item) => item.type === "function_call_output")
          .map((item) => item.call_id)
      );
      for (const item of body.output || []) {
        if (item.type === "function_call") {
          state.input.push({
            type: "function_call",
            call_id: item.call_id,
            name: item.name,
            arguments: item.arguments || "{}"
          });
          if (!serverHandledCalls.has(item.call_id)) {
            const toolResult = await executeToolCall(item);
            state.input.push({
              type: "function_call_output",
              call_id: item.call_id,
              output: toolResult
            });
          }
        } else if (item.type === "function_call_output") {
          state.input.push({
            type: "function_call_output",
            call_id: item.call_id,
            output: item.output || ""
          });
        } else if (item.type === "message") {
          state.input.push({
            role: "assistant",
            content: item.content || []
          });
        }
      }
    }

    async function executeToolCall(item) {
      const args = parseToolArgs(item.arguments || "{}");
      const name = (item.name || "").trim();
      if (name === "get_overview" || name === "get_quote") {
        const res = await fetch(adapterUrl("/tools/market/overview"), {
          method: "POST",
          headers: headers(),
          body: JSON.stringify(args)
        });
        const body = await res.json();
        if (!res.ok) throw new Error(body?.error?.message || res.statusText);
        return JSON.stringify(body, null, 2);
      }
      return "前端演示模式：这里应由你的业务 runtime 执行工具后写入真实结果。";
    }

    function parseToolArgs(raw) {
      try {
        return JSON.parse(raw || "{}");
      } catch (err) {
        throw new Error(`工具参数 JSON 格式错误：${err.message}`);
      }
    }

    function render() {
      const chat = $("chat");
      chat.innerHTML = state.input.map(renderItem).join("");
      chat.scrollTop = chat.scrollHeight;
    }

    function renderItem(item) {
      if (item.type === "function_call") {
        return `<div class="msg"><div class="role">tool call · ${escapeHtml(item.name)}</div><div class="tool">${escapeHtml(item.arguments)}</div></div>`;
      }
      if (item.type === "function_call_output") {
        return `<div class="msg"><div class="role">tool result · ${escapeHtml(item.call_id)}</div><div class="tool">${escapeHtml(item.output)}</div></div>`;
      }
      const text = (item.content || []).map((part) => part.text || part.input_text || part.output_text || "").join("\n");
      return `<div class="msg"><div class="role">${escapeHtml(item.role || "message")}</div>${escapeHtml(text)}</div>`;
    }

    function escapeHtml(value) {
      return String(value).replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch]));
    }

    $("loginBtn").addEventListener("click", login);
    $("deepseekLoginBtn").addEventListener("click", refreshDeepSeekLogin);
    $("clearBtn").addEventListener("click", () => { state.input = []; render(); });
    $("sendBtn").addEventListener("click", send);
    $("providerSelect").addEventListener("change", updateProviderMode);
    $("input").addEventListener("keydown", (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") send();
    });
    load();

    function updateProviderMode() {
      const deepseek = $("providerSelect").value === "deepseek-web";
      $("upstreamUrl").disabled = deepseek;
      $("upstreamUrl").closest("label");
      $("upstreamKey").placeholder = deepseek ? "可填 DeepSeek Cookie；优先使用下方 Session JSON" : "上游模型服务 key，可为空";
      $("deepseekSession").disabled = !deepseek;
      $("deepseekLoginBtn").disabled = !deepseek;
      if (deepseek && (!$("modelSelect").value || $("modelSelect").value === "local-model")) {
        $("modelSelect").innerHTML = `<option value="deepseek-web/reasoner">deepseek-web/reasoner</option><option value="deepseek-web/chat">deepseek-web/chat</option>`;
        $("modelSelect").value = "deepseek-web/reasoner";
      }
    }
  </script>
</body>
</html>
"#;
