// The card's link to the Rust side. Shapes mirror `src-tauri/src/views.rs`.

import type { SpeciesId } from './plants';

export interface SpeciesInfo {
  id: SpeciesId;
  name: string;
  minMinutes: number;
  unlockHours: number;
  unlocked: boolean;
}

export interface Closing {
  label: string;
  secsLeft: number;
}

export interface Session {
  species: SpeciesId;
  speciesName: string;
  plannedMinutes: number;
  mode: 'gentle' | 'strict';
  listName: string;
  remainingSecs: number;
  totalSecs: number;
  stage: number;
  slips: number;
  blocking: string[];
  closing: Closing[];
  /** Strict only: seconds until a requested early end takes effect. */
  exitSecsLeft: number | null;
  /** Started by a schedule. */
  scheduled: boolean;
}

/** A focus time: a start and end on some days, when a session starts by itself. */
export interface Schedule {
  /** 0 for one not saved yet. */
  id: number;
  /** One bit per day, Monday first: 1 is Monday, 64 is Sunday. */
  days: number;
  /** Minutes after midnight. An end at or before the start is the next day. */
  start: number;
  end: number;
  enabled: boolean;
}

/** Niwa just closed an app. */
export interface Closed {
  label: string;
}

export interface Slip {
  label: string;
  slips: number;
}

export interface Ended {
  outcome: 'grown' | 'withered';
  species: SpeciesId;
  speciesName: string;
  plannedMinutes: number;
  remainingSecs: number;
  slips: number;
  todayMinutes: number;
  streakDays: number;
}

export interface AppEntry {
  name: string;
  path: string;
  productName?: string | null;
  originalFilename?: string | null;
  /** An app Niwa knows by name (a catalog id), recognised wherever it's installed. */
  known?: string | null;
}

export interface ExceptionEntry {
  name: string;
  path: string;
}

export interface BlockListConfig {
  mode: 'gentle' | 'strict';
  /** Close what the list names, or close every app it doesn't allow. */
  kind: 'block' | 'allowOnly';
  steam: boolean;
  epic: boolean;
  riot: boolean;
  /** Ready-made categories to block, by id. */
  categories: string[];
  apps: AppEntry[];
  exceptions: ExceptionEntry[];
  /** Allow-only: the apps that may stay open. */
  allowed: AppEntry[];
}

export interface Category {
  id: string;
  label: string;
  apps: string[];
  /** How many of its apps are installed. */
  found: number;
}

export interface BlockList {
  name: string;
  config: BlockListConfig;
  found: { steam: number; epic: number; riot: number };
  categories: Category[];
  /** Installed work apps, offered when an allow-only list is empty. */
  suggested: AppEntry[];
  summary: string;
}

export interface Overview {
  session: Session | null;
  focusedMinutes: number;
  streakDays: number;
  species: SpeciesInfo[];
  list: BlockList;
  /** Whether the Strict service is installed and answering. */
  strictAvailable: boolean;
  minMinutes: number;
  maxMinutes: number;
}

export interface GardenPlant {
  id: number;
  species: SpeciesId;
  speciesName: string;
  withered: boolean;
  /** The session length for a grown plant; how long it lasted for a withered one. */
  minutes: number;
  plannedMinutes: number;
  mode: 'gentle' | 'strict';
  slips: number;
  createdMs: number;
}

export interface Garden {
  plants: GardenPlant[];
  grown: number;
  withered: number;
  nextUnlock: { name: string; hours: number } | null;
}

export interface Stats {
  /** Minutes focused each day, Monday first. */
  days: number[];
  /** Which of `days` is today. */
  today: number;
  weekMinutes: number;
  changePercent: number | null;
  streakDays: number;
  monthMinutes: number;
  completionPercent: number | null;
  /** Every minute spent blocking, across all sessions. */
  totalMinutes: number;
  tempting: { name: string; count: number }[];
}

export type ResizeDirection =
  | 'North'
  | 'South'
  | 'East'
  | 'West'
  | 'NorthEast'
  | 'NorthWest'
  | 'SouthEast'
  | 'SouthWest';

export interface Settings {
  sound: boolean;
  startWithWindows: boolean;
  checkUpdates: boolean;
}

/** A newer version of Niwa, ready to install. */
export interface Update {
  version: string;
  /** Windows will ask for admin: there's no Strict service to install it. */
  asksAdmin: boolean;
}

export interface Events {
  'session-tick': Session;
  'session-slip': Slip;
  'app-closed': Closed;
  'session-ended': Ended;
  'close-requested': null;
  'update-ready': Update;
}

export interface Backend {
  overview(): Promise<Overview>;
  garden(): Promise<Garden>;
  stats(): Promise<Stats>;
  settings(): Promise<Settings>;
  /** Starts resizing the card from one of its edges or corners, following the mouse. */
  resizeCard(direction: ResizeDirection): Promise<void>;
  setSound(on: boolean): Promise<void>;
  setStartWithWindows(on: boolean): Promise<void>;
  setCheckUpdates(on: boolean): Promise<void>;
  schedules(): Promise<Schedule[]>;
  /** Adds (id 0) or changes a focus time; rejects with why it can't be saved. Returns them all. */
  saveSchedule(schedule: Schedule): Promise<Schedule[]>;
  deleteSchedule(id: number): Promise<Schedule[]>;
  /** The update found by the daily look, if there is one. */
  update(): Promise<Update | null>;
  /** Niwa exits and the installer takes over; rejects with why it couldn't. */
  installUpdate(): Promise<void>;
  /** Rejects with a message to show the user if the session can't start. */
  start(minutes: number, species: SpeciesId): Promise<Session>;
  /** Start the wait that ends the session early (`exitWaitMinutes`). */
  requestExit(): Promise<void>;
  cancelExit(): Promise<void>;
  saveList(config: BlockListConfig): Promise<BlockList>;
  /** The open apps a list would close if a session started now. Only looks. */
  previewClosing(config: BlockListConfig): Promise<string[]>;
  pickApp(allow: boolean): Promise<AppEntry | null>;
  pickException(folder: boolean): Promise<ExceptionEntry | null>;
  /** Minimises the card into the mini timer. */
  hide(): Promise<void>;
  /** From the mini timer: opens the full card. */
  openCard(): Promise<void>;
  /** From the mini timer: lets the user drag it. The app remembers where it ends up. */
  dragMini(): Promise<void>;
  quit(): Promise<void>;
  on<K extends keyof Events>(event: K, handler: (payload: Events[K]) => void): void;
}

async function tauriBackend(): Promise<Backend> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  return {
    overview: () => invoke('get_overview'),
    garden: () => invoke('get_garden'),
    stats: () => invoke('get_stats'),
    settings: () => invoke('get_settings'),
    resizeCard: async (direction) => {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().startResizeDragging(direction);
    },
    setSound: (on) => invoke('set_sound', { on }),
    setStartWithWindows: (on) => invoke('set_start_with_windows', { on }),
    setCheckUpdates: (on) => invoke('set_check_updates', { on }),
    schedules: () => invoke('get_schedules'),
    saveSchedule: (schedule) => invoke('save_schedule', { schedule }),
    deleteSchedule: (id) => invoke('delete_schedule', { id }),
    update: () => invoke('get_update'),
    installUpdate: () => invoke('install_update'),
    start: (minutes, species) => invoke('start_session', { minutes, species }),
    requestExit: () => invoke('request_exit'),
    cancelExit: () => invoke('cancel_exit'),
    saveList: (config) => invoke('save_block_list', { config }),
    previewClosing: (config) => invoke('preview_closing', { config }),
    pickApp: (allow) => invoke('pick_app', { allow }),
    pickException: (folder) => invoke('pick_exception', { folder }),
    hide: () => invoke('hide_window'),
    openCard: () => invoke('open_card'),
    dragMini: async () => {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      // Windows moves the window from here; the app records where it ends up.
      await invoke('mini_drag_started');
      await getCurrentWindow().startDragging();
    },
    quit: () => invoke('quit_app'),
    on: (event, handler) => {
      void listen(event, (e) => handler(e.payload as never));
    },
  };
}

export const inTauri = '__TAURI_INTERNALS__' in window;

/** Which window this page is in: the card ("main") or the mini timer ("mini"). */
export const windowLabel: string = inTauri
  ? (await import('@tauri-apps/api/window')).getCurrentWindow().label
  : location.hash === '#mini'
    ? 'mini'
    : 'main';

/** In a plain browser (design work, no Rust side) a stand-in backend is used. */
export const backend: Backend = inTauri
  ? await tauriBackend()
  : (await import('./mock')).mockBackend();
