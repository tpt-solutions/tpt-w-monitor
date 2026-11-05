// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Wire types. These mirror the DTOs in src-tauri/src/dto.rs; keep them in sync.

export type PriorityDto = 'idle' | 'below_normal' | 'normal' | 'above_normal' | 'high';

export type ActionDto =
  | { kind: 'kill' }
  | { kind: 'suspend' }
  | { kind: 'resume' }
  | { kind: 'set_priority'; priority: PriorityDto }
  | { kind: 'set_affinity'; mask: number };

export interface SystemDto {
  cpuTotalPercent: number;
  cpuPerCorePercent: number[];
  memoryTotalBytes: number;
  memoryUsedBytes: number;
  netUpBytesPerSec: number | null;
  netDownBytesPerSec: number | null;
}

export interface ProcessDto {
  pid: number;
  name: string;
  cpuPercent: number;
  workingSetBytes: number;
  privateBytes: number;
  commitBytes: number;
  gpuEnginePercent: number | null;
  vramBytes: number | null;
  diskReadBytesPerSec: number | null;
  diskWriteBytesPerSec: number | null;
  netUpBytesPerSec: number | null;
  netDownBytesPerSec: number | null;
  hasVisibleWindow: boolean;
}

export interface SuggestionDto {
  pid: number;
  processName: string;
  rationale: string;
  recommendedAction: ActionDto;
  confidence: number;
  impactCpuPercent: number;
}

export interface TemperatureDto {
  label: string;
  celsius: number;
}

export interface FanDto {
  label: string;
  rpm: number;
}

export interface SensorsDto {
  temperatures: TemperatureDto[] | null;
  fans: FanDto[] | null;
}

export interface FrameDto {
  system: SystemDto;
  processes: ProcessDto[];
  suggestions: SuggestionDto[];
  sensors: SensorsDto;
}

export interface ViewDto {
  dashboardVisible: boolean;
  dashboardFocused: boolean;
  openOverlays: number;
}

/** An open desktop overlay, as the backend saves it. */
export interface OverlayDto {
  id: string;
  kind: string;
  x: number;
  y: number;
  width: number;
  height: number;
  opacity: number;
  clickThrough: boolean;
}

/** Suggestion settings: dismissed executables, and protected system processes. */
export interface SuggestionSettingsDto {
  dismissed: string[];
  protected: string[];
}
