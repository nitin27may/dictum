export type RecordingPhase =
  | "IDLE"
  | "RECORDING"
  | "PROCESSING"
  | "SUCCESS"
  | "ERROR";

export interface RecordingState {
  phase: RecordingPhase;
  audioLevel: number; // 0-1 normalized RMS
  recordingStartedAt: number | null; // epoch ms
  durationSecs: number;
  lastTranscription: string | null;
  error: string | null;
  charCount: number;
}

export interface RecordingStore extends RecordingState {
  startRecording: () => void;
  setProcessing: () => void;
  setSuccess: (text: string) => void;
  setError: (message: string) => void;
  reset: () => void;
  setAudioLevel: (level: number) => void;
  setDuration: (secs: number) => void;
}

export const INITIAL_RECORDING_STATE: RecordingState = {
  phase: "IDLE",
  audioLevel: 0,
  recordingStartedAt: null,
  durationSecs: 0,
  lastTranscription: null,
  error: null,
  charCount: 0,
};
