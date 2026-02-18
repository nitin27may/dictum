/**
 * transcription.ts — Speech-to-text via OpenAI or Azure OpenAI Whisper.
 *
 * Pure TypeScript: no Tauri imports. Works in browser, Tauri, and future mobile.
 * API key precedence:
 *   1. Settings store (user-configured, persisted)
 *   2. VITE_OPENAI_API_KEY env variable (dev/CI convenience)
 *   3. Throw — require explicit configuration
 */

import { ApiConfig } from "../types/settings";

export interface TranscriptionResult {
  text: string;
  durationMs: number;
}

export class TranscriptionError extends Error {
  constructor(
    message: string,
    public readonly statusCode?: number
  ) {
    super(message);
    this.name = "TranscriptionError";
  }
}

export async function transcribeAudio(
  wavBytes: Uint8Array,
  config: ApiConfig
): Promise<TranscriptionResult> {
  const start = Date.now();

  if (config.provider === "azure") {
    return transcribeWithAzure(wavBytes, config, start);
  }

  return transcribeWithOpenAI(wavBytes, config, start);
}

async function transcribeWithOpenAI(
  wavBytes: Uint8Array,
  config: ApiConfig,
  startMs: number
): Promise<TranscriptionResult> {
  const apiKey = resolveOpenAIKey(config);
  const model = config.openai.whisperModel || "whisper-1";
  const baseUrl = config.openai.baseUrl || "https://api.openai.com/v1";

  const formData = buildFormData(wavBytes, model);

  const response = await fetchWithRetry(
    `${baseUrl}/audio/transcriptions`,
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
      },
      body: formData,
    },
    { retries: 1, initialDelayMs: 3000 }
  );

  const json = await response.json();

  if (!response.ok) {
    throw new TranscriptionError(
      json?.error?.message ?? `OpenAI API error: ${response.status}`,
      response.status
    );
  }

  return {
    text: json.text as string,
    durationMs: Date.now() - startMs,
  };
}

async function transcribeWithAzure(
  wavBytes: Uint8Array,
  config: ApiConfig,
  startMs: number
): Promise<TranscriptionResult> {
  const { endpoint, apiKey, whisperDeployment, apiVersion } = config.azure;

  if (!endpoint) {
    throw new TranscriptionError("Azure endpoint is not configured");
  }
  if (!apiKey) {
    throw new TranscriptionError("Azure API key is not configured");
  }

  const url = `${endpoint.replace(/\/$/, "")}/openai/deployments/${whisperDeployment}/audio/transcriptions?api-version=${apiVersion}`;
  const formData = buildFormData(wavBytes, whisperDeployment ?? "whisper");

  const response = await fetchWithRetry(
    url,
    {
      method: "POST",
      headers: {
        "api-key": apiKey,
      },
      body: formData,
    },
    { retries: 1, initialDelayMs: 3000 }
  );

  const json = await response.json();

  if (!response.ok) {
    throw new TranscriptionError(
      json?.error?.message ?? `Azure API error: ${response.status}`,
      response.status
    );
  }

  return {
    text: json.text as string,
    durationMs: Date.now() - startMs,
  };
}

function buildFormData(wavBytes: Uint8Array, model: string): FormData {
  // Copy into a plain ArrayBuffer — avoids SharedArrayBuffer type incompatibility
  const plain = new Uint8Array(wavBytes).buffer as ArrayBuffer;
  const blob = new Blob([plain], { type: "audio/wav" });
  const formData = new FormData();
  formData.append("file", blob, "audio.wav");
  formData.append("model", model);
  formData.append("response_format", "json");
  return formData;
}

function resolveOpenAIKey(config: ApiConfig): string {
  // 1. Settings store
  if (config.openai.apiKey) {
    return config.openai.apiKey;
  }
  // 2. Env variable (Vite exposes VITE_* at build time)
  const envKey = import.meta.env.VITE_OPENAI_API_KEY as string | undefined;
  if (envKey) {
    return envKey;
  }
  throw new TranscriptionError(
    "OpenAI API key not configured. Set it in Settings or via VITE_OPENAI_API_KEY environment variable."
  );
}

interface RetryOptions {
  retries: number;
  initialDelayMs: number;
}

async function fetchWithRetry(
  url: string,
  options: RequestInit,
  { retries, initialDelayMs }: RetryOptions
): Promise<Response> {
  let lastError: Error | null = null;

  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const response = await fetch(url, options);
      // Don't retry on 4xx — those are config errors, not transient
      if (response.ok || response.status < 500) {
        return response;
      }
      lastError = new Error(`HTTP ${response.status}`);
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
    }

    if (attempt < retries) {
      await delay(initialDelayMs);
    }
  }

  throw lastError ?? new Error("Transcription request failed");
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
