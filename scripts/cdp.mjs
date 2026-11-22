// Minimal Chrome DevTools Protocol client for the running Manka window.
// Usage:
//   node scripts/cdp.mjs text
//   node scripts/cdp.mjs eval "document.title"
//   node scripts/cdp.mjs shot shot.png
//   node scripts/cdp.mjs click "Стратегии"
//
// Requires the app built with additionalBrowserArgs --remote-debugging-port=9222.

import { writeFileSync } from "node:fs";

const HOST = "http://127.0.0.1:9222";

async function target() {
  const list = await (await fetch(`${HOST}/json`)).json();
  const page = list.find((t) => t.type === "page");
  if (!page) throw new Error("no page target; is Manka running with remote debugging?");
  return page;
}

async function connect() {
  const page = await target();
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  let seq = 0;
  const pending = new Map();
  const events = [];
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && pending.has(msg.id)) {
      pending.get(msg.id)(msg);
      pending.delete(msg.id);
    } else {
      events.push(msg);
    }
  };
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = reject;
  });
  const send = (method, params = {}) =>
    new Promise((resolve) => {
      const id = ++seq;
      pending.set(id, resolve);
      ws.send(JSON.stringify({ id, method, params }));
    });
  const evaluate = async (expression) => {
    const r = await send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
      userGesture: true,
    });
    if (r.result?.exceptionDetails) {
      throw new Error(
        r.result.exceptionDetails.exception?.description ??
          r.result.exceptionDetails.text ??
          "evaluation failed",
      );
    }
    return r.result?.result?.value;
  };
  return { ws, send, evaluate, events };
}

const [cmd, ...rest] = process.argv.slice(2);
const { ws, send, evaluate, events } = await connect();

try {
  switch (cmd) {
    case "text": {
      console.log(await evaluate("document.body.innerText"));
      break;
    }
    case "eval": {
      console.log(JSON.stringify(await evaluate(rest.join(" ")), null, 2));
      break;
    }
    case "shot": {
      const file = rest[0] ?? "shot.png";
      const r = await send("Page.captureScreenshot", { format: "png" });
      writeFileSync(file, Buffer.from(r.result.data, "base64"));
      console.log(`saved ${file}`);
      break;
    }
    case "click": {
      const label = rest.join(" ");
      const result = await evaluate(`(() => {
        const nodes = [...document.querySelectorAll('button, .chip, a, .nav button')];
        const el = nodes.find(n => (n.innerText || '').trim().includes(${JSON.stringify(label)}));
        if (!el) return 'not found: ' + ${JSON.stringify(label)};
        el.click();
        return 'clicked: ' + (el.innerText || '').trim().slice(0, 80);
      })()`);
      console.log(result);
      break;
    }
    case "errors": {
      const errs = events.filter(
        (e) =>
          e.method === "Runtime.exceptionThrown" ||
          (e.method === "Runtime.consoleAPICalled" && e.params.type === "error") ||
          (e.method === "Log.entryAdded" && e.params.entry.level === "error"),
      );
      console.log(JSON.stringify(errs, null, 2));
      break;
    }
    case "watch": {
      await send("Runtime.enable");
      await send("Log.enable");
      console.log("listening for 30s…");
      await new Promise((r) => setTimeout(r, 30000));
      const errs = events.filter(
        (e) =>
          e.method === "Runtime.exceptionThrown" ||
          (e.method === "Runtime.consoleAPICalled" && e.params.type === "error") ||
          (e.method === "Log.entryAdded" && e.params.entry.level === "error"),
      );
      console.log(JSON.stringify(errs, null, 2));
      break;
    }
    default:
      console.log("commands: text | eval <js> | shot <file> | click <label> | errors | watch");
  }
} finally {
  ws.close();
}
