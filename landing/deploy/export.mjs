import { spawn } from "node:child_process";
import {
  access,
  cp,
  mkdir,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";

const deployDir = dirname(fileURLToPath(import.meta.url));
const landingDir = join(deployDir, "..");
const publicDir = join(deployDir, "public");
const bundleDir = join(landingDir, "target", "release", "assets");
const binary = join(landingDir, "target", "release", "dainiki-landing");
const port = process.env.EXPORT_PORT ?? "3971";
const base = `http://127.0.0.1:${port}`;

async function waitForServer(timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    try {
      const response = await fetch(base);
      if (response.ok) return;
    } catch {
      // Server is still starting.
    }
    await delay(250);
  }

  throw new Error(`app did not come up on ${base}`);
}

function rewriteAssetPaths(html) {
  return html
    .replaceAll("/_topcoat/assets/", "/assets/")
    .replaceAll("/_topcoat/fonts/", "/fonts/")
    .replace(
      /<script[^>]*src="http:\/\/127\.0\.0\.1:\d+\/dev\.js"[^>]*><\/script>/g,
      "",
    );
}

await access(binary).catch(() => {
  throw new Error(`missing release binary: ${binary}. Run cargo build --release first.`);
});
await access(bundleDir).catch(() => {
  throw new Error(
    `missing release asset bundle: ${bundleDir}. Run topcoat asset bundle --release first.`,
  );
});

const app = spawn(binary, [], {
  cwd: landingDir,
  env: {
    ...process.env,
    PORT: port,
    WAITLIST_BACKEND: "file",
    WAITLIST_FILE: join(deployDir, ".export-waitlist.tsv"),
  },
  stdio: "ignore",
});

try {
  await waitForServer();

  const home = await (await fetch(base)).text();

  const fontUrls = [
    ...new Set(
      [...home.matchAll(/\/_topcoat\/fonts\/[^"' ]+\.css/g)].map(
        (match) => match[0],
      ),
    ),
  ];

  await rm(publicDir, { recursive: true, force: true });
  await mkdir(join(publicDir, "assets"), { recursive: true });
  await mkdir(join(publicDir, "fonts"), { recursive: true });

  for (const entry of await readdir(bundleDir)) {
    if (entry === "manifest.toml") continue;
    await cp(join(bundleDir, entry), join(publicDir, "assets", entry), {
      recursive: true,
    });
  }

  for (const fontUrl of fontUrls) {
    const name = fontUrl.split("/").pop();
    const css = await (await fetch(`${base}${fontUrl}`)).text();
    await writeFile(
      join(publicDir, "fonts", name),
      rewriteAssetPaths(css),
      "utf8",
    );
  }

  const homePage = rewriteAssetPaths(home);

  await writeFile(join(publicDir, "index.html"), homePage, "utf8");
  await cp(join(landingDir, "src/landing/img/og.png"), join(publicDir, "og.png"));

  await writeFile(
    join(publicDir, "_headers"),
    `# Hashed assets can be cached forever.
/assets/*
  Cache-Control: public, max-age=31536000, immutable
/fonts/*
  Cache-Control: public, max-age=31536000, immutable
/og.png
  Cache-Control: public, max-age=86400
/index.html
  Cache-Control: public, max-age=0, must-revalidate
`,
    "utf8",
  );

  if (homePage.includes("/_topcoat/")) {
    throw new Error("index.html still contains Topcoat asset paths");
  }
  if (homePage.includes("127.0.0.1")) {
    throw new Error("index.html still contains a localhost URL");
  }

  console.log(
    `exported ${publicDir}: index.html, ${fontUrls.length} font stylesheet(s), ${(await readdir(join(publicDir, "assets"))).length} asset(s)`,
  );
} finally {
  app.kill();
}
