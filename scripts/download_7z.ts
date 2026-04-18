#!/usr/bin/env -S deno run --allow-net --allow-read --allow-write --allow-run --allow-env

/**
 * Download 7-zip DLLs required by bit7z
 *
 * Downloads and extracts 7z.dll to the output directory.
 * This script handles the bootstrap problem by first downloading 7zr.exe
 * (standalone console 7-zip), then using it to extract the full package.
 *
 * Supports:
 * - Progress bar with download speed
 * - HTTP_PROXY, HTTPS_PROXY, ALL_PROXY environment variables
 * - HTTP and SOCKS5 proxies
 */

import { ensureDir } from "@std/fs/ensure-dir";
import { exists } from "@std/fs/exists";
import { join } from "@std/path";

const VERSION = "2600"; // 7-zip version (26.00)

interface DownloadSource {
  name: string;
  sevenZrUrl: string;
  extraUrl: string;
  minSize?: number; // Minimum expected file size to validate download
}

interface ProxyConfig {
  type: "http" | "socks5" | "none";
  url?: string;
}

function getSources(): DownloadSource[] {
  // Use the main installer (exe) which contains 7z.dll
  // The -extra.7z package only contains 7za.dll (standalone version)
  return [
    {
      name: "GitHub (official)",
      sevenZrUrl: `https://github.com/ip7z/7zip/releases/download/26.00/7zr.exe`,
      extraUrl: `https://github.com/ip7z/7zip/releases/download/26.00/7z${VERSION}-x64.exe`,
      minSize: 100 * 1024, // At least 100KB
    },
    {
      name: "7-zip.org",
      sevenZrUrl: "https://www.7-zip.org/a/7zr.exe",
      extraUrl: `https://www.7-zip.org/a/7z${VERSION}-x64.exe`,
      minSize: 100 * 1024,
    },
  ];
}

function getProxyConfig(url: string): ProxyConfig {
  const isHttps = url.startsWith("https://");

  // Check environment variables in order of priority
  const proxyUrl = Deno.env.get(isHttps ? "HTTPS_PROXY" : "HTTP_PROXY") ||
    Deno.env.get(isHttps ? "https_proxy" : "http_proxy") ||
    Deno.env.get("ALL_PROXY") ||
    Deno.env.get("all_proxy");

  if (!proxyUrl) {
    return { type: "none" };
  }

  const lowerProxy = proxyUrl.toLowerCase();
  if (lowerProxy.startsWith("socks5://") || lowerProxy.startsWith("socks://")) {
    return { type: "socks5", url: proxyUrl };
  }

  return { type: "http", url: proxyUrl };
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

function formatSpeed(bytesPerSecond: number): string {
  if (bytesPerSecond < 1024) return `${bytesPerSecond.toFixed(0)} B/s`;
  if (bytesPerSecond < 1024 * 1024) return `${(bytesPerSecond / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSecond / 1024 / 1024).toFixed(2)} MB/s`;
}

function renderProgressBar(
  current: number,
  total: number,
  speed: number,
  width = 40,
): string {
  const percentage = total > 0 ? Math.min(100, (current / total) * 100) : 0;
  const filled = Math.round((percentage / 100) * width);
  const empty = width - filled;

  const bar = "\u2588".repeat(filled) + "\u2591".repeat(empty);
  const percentStr = percentage.toFixed(1).padStart(5);
  const currentStr = formatBytes(current);
  const totalStr = total > 0 ? formatBytes(total) : "???";
  const speedStr = formatSpeed(speed);

  return `\r[${bar}] ${percentStr}% ${currentStr}/${totalStr} @ ${speedStr}`;
}

function createHttpClient(proxyConfig: ProxyConfig): Deno.HttpClient | undefined {
  if (proxyConfig.type === "none" || !proxyConfig.url) {
    return undefined;
  }

  if (proxyConfig.type === "http") {
    return Deno.createHttpClient({
      proxy: { url: proxyConfig.url },
    });
  }

  // For SOCKS5, Deno 2.x doesn't have native support yet
  // We'll fall back to using curl if available
  return undefined;
}

async function downloadWithCurl(
  url: string,
  destPath: string,
  proxyUrl: string,
): Promise<void> {
  const args = [
    "-L", // Follow redirects
    "--progress-bar",
    "-o",
    destPath,
    "--proxy",
    proxyUrl,
    "-A",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Deno/2.0",
    url,
  ];

  const cmd = new Deno.Command("curl", {
    args,
    stdout: "inherit",
    stderr: "inherit",
  });

  const { code } = await cmd.output();
  if (code !== 0) {
    throw new Error(`curl failed with exit code ${code}`);
  }
}

async function downloadFile(
  url: string,
  destPath: string,
  description: string,
  maxRetries = 3,
): Promise<void> {
  const proxyConfig = getProxyConfig(url);

  console.log(`Downloading ${description}...`);
  console.log(`  URL: ${url}`);
  if (proxyConfig.type !== "none") {
    console.log(`  Proxy: ${proxyConfig.url} (${proxyConfig.type})`);
  }

  // Use curl for SOCKS5 proxy since Deno doesn't support it natively
  if (proxyConfig.type === "socks5" && proxyConfig.url) {
    try {
      await downloadWithCurl(url, destPath, proxyConfig.url);
      const stat = await Deno.stat(destPath);
      console.log(`\n  Saved: ${destPath} (${formatBytes(stat.size)})`);
      return;
    } catch (error) {
      throw new Error(`SOCKS5 download failed: ${error instanceof Error ? error.message : error}`);
    }
  }

  let lastError: Error | null = null;
  const httpClient = createHttpClient(proxyConfig);

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      const fetchOptions: RequestInit & { client?: Deno.HttpClient } = {
        headers: {
          "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Deno/2.0",
        },
      };

      if (httpClient) {
        fetchOptions.client = httpClient;
      }

      const response = await fetch(url, fetchOptions);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status} ${response.statusText}`);
      }

      const contentLength = parseInt(response.headers.get("content-length") || "0", 10);
      const reader = response.body?.getReader();

      if (!reader) {
        throw new Error("No response body");
      }

      const chunks: Uint8Array[] = [];
      let downloaded = 0;
      const startTime = Date.now();
      let lastUpdate = startTime;

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        chunks.push(value);
        downloaded += value.length;

        const now = Date.now();
        const elapsed = (now - startTime) / 1000;
        const speed = elapsed > 0 ? downloaded / elapsed : 0;

        // Update progress bar (throttle to every 100ms)
        if (now - lastUpdate >= 100 || done) {
          const progressBar = renderProgressBar(downloaded, contentLength, speed);
          await Deno.stdout.write(new TextEncoder().encode(progressBar));
          lastUpdate = now;
        }
      }

      // Clear progress line and show completion
      await Deno.stdout.write(new TextEncoder().encode("\r" + " ".repeat(80) + "\r"));

      // Combine chunks and write to file
      const totalLength = chunks.reduce((acc, chunk) => acc + chunk.length, 0);
      const data = new Uint8Array(totalLength);
      let offset = 0;
      for (const chunk of chunks) {
        data.set(chunk, offset);
        offset += chunk.length;
      }

      await Deno.writeFile(destPath, data);

      const elapsed = (Date.now() - startTime) / 1000;
      const avgSpeed = elapsed > 0 ? downloaded / elapsed : 0;
      console.log(
        `  Saved: ${destPath} (${formatBytes(downloaded)}) in ${elapsed.toFixed(1)}s @ ${
          formatSpeed(avgSpeed)
        }`,
      );

      httpClient?.close();
      return;
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      console.log(`\n  Attempt ${attempt}/${maxRetries} failed: ${lastError.message}`);
      if (attempt < maxRetries) {
        const delay = attempt * 2000;
        console.log(`  Retrying in ${delay / 1000}s...`);
        await new Promise((r) => setTimeout(r, delay));
      }
    }
  }

  httpClient?.close();
  throw lastError ?? new Error("Download failed");
}

async function tryDownloadFromSources(
  sources: DownloadSource[],
  getUrl: (s: DownloadSource) => string,
  destPath: string,
  description: string,
): Promise<string> {
  for (const source of sources) {
    const url = getUrl(source);
    console.log(`\nTrying ${source.name}...`);
    try {
      await downloadFile(url, destPath, description, 2);

      // Validate file size
      const stat = await Deno.stat(destPath);
      if (source.minSize && stat.size < source.minSize) {
        throw new Error(
          `Downloaded file too small (${formatBytes(stat.size)}), expected at least ${
            formatBytes(source.minSize)
          }`,
        );
      }

      return source.name;
    } catch (error) {
      console.log(`  ${source.name} failed: ${error instanceof Error ? error.message : error}`);
      // Clean up potentially corrupted file
      try {
        await Deno.remove(destPath);
      } catch {
        // Ignore cleanup errors
      }
    }
  }
  throw new Error(`All download sources failed for ${description}`);
}

async function extract7z(
  sevenZrPath: string,
  archivePath: string,
  outputDir: string,
): Promise<void> {
  console.log(`Extracting ${archivePath}...`);

  const cmd = new Deno.Command(sevenZrPath, {
    args: ["x", "-y", `-o${outputDir}`, archivePath],
    stdout: "piped",
    stderr: "piped",
  });

  const { code, stdout, stderr } = await cmd.output();

  if (code !== 0) {
    const errorText = new TextDecoder().decode(stderr);
    throw new Error(`Extraction failed (exit code ${code}): ${errorText}`);
  }

  const outputText = new TextDecoder().decode(stdout);
  console.log(outputText);
}

async function findDll(dir: string, name: string): Promise<string | null> {
  for await (const entry of Deno.readDir(dir)) {
    const fullPath = join(dir, entry.name);
    if (entry.isFile && entry.name.toLowerCase() === name.toLowerCase()) {
      return fullPath;
    }
    if (entry.isDirectory) {
      const found = await findDll(fullPath, name);
      if (found) return found;
    }
  }
  return null;
}

async function main(): Promise<void> {
  const projectRoot = new URL("..", import.meta.url).pathname.replace(/^\/([A-Z]:)/i, "$1");
  const outputDir = join(projectRoot, "output");
  const tempDir = join(outputDir, "temp");
  const sources = getSources();

  console.log("=== 7-zip Download Script ===\n");
  console.log(`Project root: ${projectRoot}`);
  console.log(`Output directory: ${outputDir}`);

  // Show proxy configuration
  const testProxy = getProxyConfig("https://example.com");
  if (testProxy.type !== "none") {
    console.log(`Proxy: ${testProxy.url} (${testProxy.type})`);
  }
  console.log();

  // Create directories
  await ensureDir(outputDir);
  await ensureDir(tempDir);

  const sevenZrPath = join(tempDir, "7zr.exe");
  const installerPath = join(tempDir, `7z${VERSION}-x64.exe`);
  const extractDir = join(tempDir, "extracted");

  // Step 1: Download 7zr.exe (standalone console version)
  if (!(await exists(sevenZrPath))) {
    await tryDownloadFromSources(
      sources,
      (s) => s.sevenZrUrl,
      sevenZrPath,
      "7zr.exe (standalone console)",
    );
  } else {
    console.log("7zr.exe already exists, skipping download.");
  }

  // Step 2: Download 7-zip installer (contains 7z.dll)
  if (!(await exists(installerPath))) {
    await tryDownloadFromSources(
      sources,
      (s) => s.extraUrl,
      installerPath,
      "7-zip installer (x64)",
    );
  } else {
    console.log("7-zip installer already exists, skipping download.");
  }

  // Step 3: Extract the installer
  console.log("\nExtracting 7-zip installer...");
  await ensureDir(extractDir);
  await extract7z(sevenZrPath, installerPath, extractDir);

  // Step 4: Find and copy DLLs to output directory
  console.log("\nLooking for DLLs...");

  // 7z.dll is the main DLL needed by bit7z
  const dllsToCopy = ["7z.dll", "7z64.dll", "7za.dll", "7zxa.dll"];
  const copiedDlls: string[] = [];

  for (const dllName of dllsToCopy) {
    const dllPath = await findDll(extractDir, dllName);
    if (dllPath) {
      const destPath = join(outputDir, dllName);
      await Deno.copyFile(dllPath, destPath);
      console.log(`  Copied: ${dllName}`);
      copiedDlls.push(dllName);
    }
  }

  // Also copy 7z.dll as the main DLL if only 7z64.dll exists
  const mainDll = join(outputDir, "7z.dll");
  if (!(await exists(mainDll))) {
    const dll64 = join(outputDir, "7z64.dll");
    if (await exists(dll64)) {
      await Deno.copyFile(dll64, mainDll);
      console.log("  Copied 7z64.dll as 7z.dll");
      copiedDlls.push("7z.dll (from 7z64.dll)");
    }
  }

  if (copiedDlls.length === 0) {
    console.error("\nError: No DLLs found in the extracted package!");
    console.error("Please check the 7z-extra package structure.");
    Deno.exit(1);
  }

  // Cleanup temp directory
  console.log("\nCleaning up temporary files...");
  await Deno.remove(tempDir, { recursive: true });

  console.log("\n=== Done! ===");
  console.log(`\nDLLs are available at: ${outputDir}`);
  console.log("\nTo use with bit7z, set the DLL path when loading the library:");
  console.log(`  let lib = Library::new("${join(outputDir, "7z.dll").replace(/\\/g, "/")}")?;`);
}

if (import.meta.main) {
  try {
    await main();
  } catch (error) {
    console.error(`\nError: ${error instanceof Error ? error.message : error}`);
    Deno.exit(1);
  }
}
