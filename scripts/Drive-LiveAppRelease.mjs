const endpoint = process.argv[2];
const expectedVersion = process.argv[3];
const manifestPath = process.argv[4];
const releaseNotes = process.argv[5];
const expectedHashes = JSON.parse(process.argv[6] ?? "{}");

if (!endpoint || !expectedVersion || !manifestPath || !releaseNotes) {
  throw new Error("Usage: node Drive-LiveAppRelease.mjs <cdp-websocket> <version> <manifest> <notes> <hash-json>");
}

const socket = new WebSocket(endpoint);
const pending = new Map();
let nextId = 1;

socket.addEventListener("message", (event) => {
  const message = JSON.parse(String(event.data));
  if (!message.id || !pending.has(message.id)) return;
  const { resolve, reject } = pending.get(message.id);
  pending.delete(message.id);
  if (message.error) reject(new Error(message.error.message));
  else resolve(message.result);
});

await new Promise((resolve, reject) => {
  socket.addEventListener("open", resolve, { once: true });
  socket.addEventListener("error", () => reject(new Error("Could not connect to the Developer WebView debugging endpoint")), { once: true });
});

function command(method, params = {}) {
  const id = nextId++;
  socket.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

async function evaluate(expression) {
  const result = await command("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  }
  return result.result.value;
}

async function waitFor(expression, label, timeoutMs = 45_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await evaluate(expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  const body = await evaluate("document.body.innerText");
  throw new Error(`Timed out waiting for ${label}. Visible Developer text: ${body}`);
}

function buttonExpression(label, click) {
  return `(() => {
    const button = [...document.querySelectorAll("button")].find((item) => item.innerText.trim().includes(${JSON.stringify(label)}));
    if (!button || button.disabled) return false;
    ${click ? "button.click();" : ""}
    return true;
  })()`;
}

function setFieldExpression(labelText, value, multiline = false) {
  const elementType = multiline ? "textarea" : "input";
  const prototype = multiline ? "HTMLTextAreaElement.prototype" : "HTMLInputElement.prototype";
  return `(() => {
    const label = [...document.querySelectorAll("label")].find((item) => item.innerText.includes(${JSON.stringify(labelText)}));
    const field = label?.querySelector(${JSON.stringify(elementType)});
    if (!field) return false;
    Object.getOwnPropertyDescriptor(${prototype}, "value").set.call(field, ${JSON.stringify(value)});
    field.dispatchEvent(new Event("input", { bubbles: true }));
    field.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`;
}

try {
  await command("Runtime.enable");
  await waitFor(buttonExpression("App update", false), "the App update navigation button");
  await evaluate(buttonExpression("App update", true));
  await waitFor(setFieldExpression("Windows build manifest", manifestPath), "the build manifest field");
  await waitFor(setFieldExpression("Release notes", releaseNotes, true), "the release notes field");
  await waitFor(buttonExpression("Verify packaged Player release", false), "the release verification action");
  await evaluate(buttonExpression("Verify packaged Player release", true));

  const expectedTag = `v${expectedVersion}`;
  const hashChecks = Object.values(expectedHashes)
    .map((hash) => `document.body.innerText.toLowerCase().includes(${JSON.stringify(String(hash).toLowerCase())})`)
    .join(" && ");
  await waitFor(
    `document.body.innerText.includes(${JSON.stringify(`${expectedTag} is ready for review`)}) && ${hashChecks || "true"}`,
    "the exact reviewed release assets",
    90_000,
  );
  await evaluate(`(() => {
    const checkbox = document.querySelector(".app-release-preview input[type=checkbox]");
    if (!checkbox) return false;
    checkbox.click();
    return true;
  })()`);
  await waitFor(buttonExpression("Publish Player app update", false), "the confirmed publish action");
  await evaluate(buttonExpression("Publish Player app update", true));
  await waitFor(
    `document.body.innerText.includes("Player app release published") && document.body.innerText.includes(${JSON.stringify(expectedTag)}) && document.body.innerText.includes("3 version-tagged assets")`,
    "the completed GitHub publication",
    120_000,
  );
  process.stdout.write(JSON.stringify({ version: expectedVersion, tag: expectedTag, reviewed: true, confirmed: true, published: true }));
} finally {
  socket.close();
}
