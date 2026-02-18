import { create } from "zustand";
import { INITIAL_RECORDING_STATE, RecordingStore } from "../types/recording";

export const useRecordingStore = create<RecordingStore>((set) => ({
  ...INITIAL_RECORDING_STATE,

  startRecording: () =>
    set({
      phase: "RECORDING",
      recordingStartedAt: Date.now(),
      durationSecs: 0,
      audioLevel: 0,
      error: null,
      lastTranscription: null,
      charCount: 0,
    }),

  setProcessing: () =>
    set({
      phase: "PROCESSING",
      audioLevel: 0,
    }),

  setSuccess: (text: string) =>
    set({
      phase: "SUCCESS",
      lastTranscription: text,
      charCount: text.length,
      audioLevel: 0,
    }),

  setError: (message: string) =>
    set({
      phase: "ERROR",
      error: message,
      audioLevel: 0,
    }),

  reset: () => set(INITIAL_RECORDING_STATE),

  setAudioLevel: (level: number) =>
    set({
      audioLevel: Math.min(1, Math.max(0, level)),
    }),

  setDuration: (secs: number) => set({ durationSecs: secs }),
}));
