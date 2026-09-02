const endpoint = process.argv[2];
const expectedVersion = process.argv[3];
const expectedSha256 = process.argv[4];

if (!endpoint || !expectedVersion || !/^[a-f0-9]{64}$/i.test(expectedSha256 ?? "")) {
  throw new Error("Usage: node Drive-LiveAppUpdate.mjs <cdp-websocket> <version> <sha256>");
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
  socket.addEventListener("error", () => reject(new Error("Could not connect to the Player WebView debugging endpoint")), { once: true });
});

function command(method, params = {}) {
  const id = nextId++;
  socket.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

async function evaluate(expression) {
  const result = await command("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
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
  throw new Error(`Timed out waiting for ${label}. Visible Player text: ${body}`);
}

function buttonExpression(label, click) {
  return `(() => {
    const button = [...document.querySelectorAll("button")].find((item) => item.innerText.trim().includes(${JSON.stringify(label)}));
    if (!button || button.disabled) return false;
    ${click ? "button.click();" : ""}
    return true;
  })()`;
}

try {
  await command("Runtime.enable");
  await waitFor(buttonExpression("App update", false), "the App update navigation button");
  await evaluate(buttonExpression("App update", true));
  await waitFor(
    `document.body.innerText.includes(${JSON.stringify(`Public Player ${expectedVersion}`)}) && document.body.innerText.toLowerCase().includes(${JSON.stringify(expectedSha256.toLowerCase())})`,
    "the reviewed live update version and SHA-256",
  );
  await waitFor(buttonExpression("Download and verify Player update", false), "the download action");
  await evaluate(buttonExpression("Download and verify Player update", true));
  await waitFor(
    `document.body.innerText.includes("Verified update ready") && [...document.querySelectorAll("button")].some((item) => item.innerText.trim().includes("Update and restart Player"))`,
    "the verified staged update",
    90_000,
  );
  await evaluate(`(() => {
    const checkbox = document.querySelector("input[type=checkbox]");
    if (!checkbox) return false;
    checkbox.click();
    return true;
  })()`);
  await waitFor(buttonExpression("Update and restart Player", false), "the confirmed restart action");
  await evaluate(buttonExpression("Update and restart Player", true));
  process.stdout.write(JSON.stringify({
    version: expectedVersion,
    sha256: expectedSha256.toLowerCase(),
    reviewed: true,
    downloaded: true,
    confirmed: true,
  }));
} finally {
  socket.close();
}
