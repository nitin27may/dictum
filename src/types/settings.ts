import { z } from "zod";

export const ApiConfigSchema = z.object({
  provider: z.enum(["openai", "azure"]).default("openai"),
  openai: z
    .object({
      apiKey: z.string().optional(),
      whisperModel: z.string().default("whisper-1"),
      baseUrl: z.string().optional(), // for custom proxies
    })
    .default({}),
  azure: z
    .object({
      endpoint: z.string().optional(),
      apiKey: z.string().optional(),
      whisperDeployment: z.string().default("whisper"),
      gptDeployment: z.string().default("gpt-4o-mini"),
      apiVersion: z.string().default("2024-02-01"),
    })
    .default({}),
});

export const SettingsSchema = z.object({
  version: z.number().default(1),
  hotkey: z.string().default("Alt+Space"),
  audioDevice: z.string().optional(),
  api: ApiConfigSchema.default({
    provider: "openai",
    openai: {},
    azure: {},
  }),
  formatting: z
    .object({
      enabled: z.boolean().default(false),
      activeProfileId: z.string().optional(),
    })
    .default({}),
  general: z
    .object({
      launchAtLogin: z.boolean().default(false),
      maxRecordingSeconds: z.number().default(60),
      overlayPosition: z
        .enum(["bottom-center", "bottom-left", "bottom-right"])
        .default("bottom-center"),
    })
    .default({}),
});

export type Settings = z.infer<typeof SettingsSchema>;
export type ApiConfig = z.infer<typeof ApiConfigSchema>;

export const DEFAULT_SETTINGS: Settings = SettingsSchema.parse({});
