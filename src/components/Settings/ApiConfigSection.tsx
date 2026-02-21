import { useState } from "react";
import { useSettingsStore } from "../../store/settingsStore";
import { transcribeAudio } from "../../services/transcription";
import { ApiConfig } from "../../types/settings";

export function ApiConfigSection() {
  const { settings, save } = useSettingsStore();
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);

  const api = settings.api;
  const provider = api.provider;

  const handleProviderChange = (p: "openai" | "azure") => {
    save({ api: { ...api, provider: p } });
    setTestResult(null);
  };

  const handleOpenAIKeyChange = (key: string) => {
    save({ api: { ...api, openai: { ...api.openai, apiKey: key } } });
    setTestResult(null);
  };

  const handleAzureFieldChange = (field: keyof ApiConfig["azure"], value: string) => {
    save({ api: { ...api, azure: { ...api.azure, [field]: value } } });
    setTestResult(null);
  };

  const smartKeywords = api.smartKeywords ?? { enabled: false };

  const handleSmartKeywordsToggle = (enabled: boolean) => {
    save({ api: { ...api, smartKeywords: { ...smartKeywords, enabled } } });
  };

  const handleOpenAIGptModelChange = (model: string) => {
    save({ api: { ...api, openai: { ...api.openai, gptModel: model } } });
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      // Generate ~0.5s of silence as test audio (16kHz, 16-bit WAV)
      const sampleCount = 8000;
      const samples = new Float32Array(sampleCount).fill(0.001);
      const wavBytes = encodeTestWav(samples);
      await transcribeAudio(wavBytes, api);
      setTestResult({ ok: true, msg: "API connection successful" });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setTestResult({ ok: false, msg });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="space-y-5">
      {/* Provider toggle */}
      <div>
        <label className="block text-xs font-medium text-zinc-400 uppercase tracking-wider mb-2">
          Provider
        </label>
        <div className="flex gap-2">
          {(["openai", "azure"] as const).map((p) => (
            <button
              key={p}
              type="button"
              onClick={() => handleProviderChange(p)}
              className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${
                provider === p
                  ? "bg-teal-600 text-white"
                  : "bg-zinc-800 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700"
              }`}
            >
              {p === "openai" ? "OpenAI" : "Azure OpenAI"}
            </button>
          ))}
        </div>
      </div>

      {/* OpenAI config */}
      {provider === "openai" && (
        <div className="space-y-4">
          <Field label="API Key">
            <input
              type="password"
              value={api.openai.apiKey ?? ""}
              onChange={(e) => handleOpenAIKeyChange(e.target.value)}
              placeholder="sk-... or set VITE_OPENAI_API_KEY env variable"
              className="input-field"
            />
            <p className="text-xs text-zinc-500 mt-1">
              Alternatively, set <code className="text-zinc-400">VITE_OPENAI_API_KEY</code> in a{" "}
              <code className="text-zinc-400">.env.local</code> file for dev.
            </p>
          </Field>
          <Field label="Whisper Model">
            <input
              type="text"
              value={api.openai.whisperModel}
              onChange={(e) =>
                save({ api: { ...api, openai: { ...api.openai, whisperModel: e.target.value } } })
              }
              className="input-field"
            />
          </Field>
        </div>
      )}

      {/* Azure config */}
      {provider === "azure" && (
        <div className="space-y-4">
          <Field label="Endpoint">
            <input
              type="url"
              value={api.azure.endpoint ?? ""}
              onChange={(e) => handleAzureFieldChange("endpoint", e.target.value)}
              placeholder="https://your-resource.openai.azure.com"
              className="input-field"
            />
          </Field>
          <Field label="API Key">
            <input
              type="password"
              value={api.azure.apiKey ?? ""}
              onChange={(e) => handleAzureFieldChange("apiKey", e.target.value)}
              className="input-field"
            />
          </Field>
          <Field label="Whisper Deployment">
            <input
              type="text"
              value={api.azure.whisperDeployment}
              onChange={(e) => handleAzureFieldChange("whisperDeployment", e.target.value)}
              className="input-field"
            />
          </Field>
          <Field label="API Version">
            <input
              type="text"
              value={api.azure.apiVersion}
              onChange={(e) => handleAzureFieldChange("apiVersion", e.target.value)}
              className="input-field"
            />
          </Field>
        </div>
      )}

      {/* Smart Keywords */}
      <div className="border-t border-zinc-800 pt-5">
        <div className="flex items-center justify-between mb-2">
          <div>
            <label className="block text-xs font-medium text-zinc-400 uppercase tracking-wider">
              Smart Keywords
            </label>
            <p className="text-xs text-zinc-500 mt-0.5">
              Say "rephrase" at the end to have GPT improve the text
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={smartKeywords.enabled}
            onClick={() => handleSmartKeywordsToggle(!smartKeywords.enabled)}
            className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
              smartKeywords.enabled ? "bg-teal-600" : "bg-zinc-700"
            }`}
          >
            <span
              className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
                smartKeywords.enabled ? "translate-x-[18px]" : "translate-x-[3px]"
              }`}
            />
          </button>
        </div>

        {smartKeywords.enabled && (
          <div className="mt-3 space-y-3">
            {provider === "openai" && (
              <Field label="GPT Model">
                <input
                  type="text"
                  value={api.openai.gptModel ?? "gpt-4o-mini"}
                  onChange={(e) => handleOpenAIGptModelChange(e.target.value)}
                  placeholder="gpt-4o-mini"
                  className="input-field"
                />
              </Field>
            )}
            {provider === "azure" && (
              <Field label="GPT Deployment">
                <input
                  type="text"
                  value={api.azure.gptDeployment}
                  onChange={(e) => handleAzureFieldChange("gptDeployment", e.target.value)}
                  placeholder="gpt-4o-mini"
                  className="input-field"
                />
              </Field>
            )}
          </div>
        )}
      </div>

      {/* Test button */}
      <div className="flex items-center gap-4 pt-1">
        <button
          type="button"
          onClick={handleTest}
          disabled={testing}
          className="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 text-zinc-200 text-sm rounded-md transition-colors disabled:opacity-50"
        >
          {testing ? "Testing..." : "Test Connection"}
        </button>
        {testResult && (
          <span
            className={`text-sm ${testResult.ok ? "text-emerald-400" : "text-red-400"}`}
          >
            {testResult.msg}
          </span>
        )}
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="block text-xs font-medium text-zinc-400 uppercase tracking-wider mb-1.5">
        {label}
      </label>
      {children}
    </div>
  );
}

// Minimal WAV encoder for test purposes
function encodeTestWav(samples: Float32Array): Uint8Array {
  const sampleRate = 16000;
  const numChannels = 1;
  const bitsPerSample = 16;
  const byteRate = (sampleRate * numChannels * bitsPerSample) / 8;
  const blockAlign = (numChannels * bitsPerSample) / 8;
  const dataSize = samples.length * 2;
  const buffer = new ArrayBuffer(44 + dataSize);
  const view = new DataView(buffer);

  const writeStr = (offset: number, str: string) => {
    for (let i = 0; i < str.length; i++) view.setUint8(offset + i, str.charCodeAt(i));
  };

  writeStr(0, "RIFF");
  view.setUint32(4, 36 + dataSize, true);
  writeStr(8, "WAVE");
  writeStr(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, numChannels, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, byteRate, true);
  view.setUint16(32, blockAlign, true);
  view.setUint16(34, bitsPerSample, true);
  writeStr(36, "data");
  view.setUint32(40, dataSize, true);

  let offset = 44;
  for (const s of samples) {
    view.setInt16(offset, Math.max(-32768, Math.min(32767, s * 32767)), true);
    offset += 2;
  }

  return new Uint8Array(buffer);
}
