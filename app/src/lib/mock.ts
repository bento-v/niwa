import { exitWaitMinutes } from './format';
// A stand-in for the Rust side, used only when the card is opened in a plain
// browser for design work. Sessions run fast and nothing is blocked.
//
// URL options: ?hours=12 sets total focused hours, ?scale=60 sets how many
// session seconds pass per real second, ?empty starts with an empty garden,
// ?strict starts with the list in Strict mode, ?allow makes it allow-only,
// ?noservice hides the service,
// ?session starts with a session already growing (?session=wisteria:300 to choose).

import type {
  Backend,
  BlockList,
  BlockListConfig,
  Events,
  Garden,
  GardenPlant,
  Overview,
  Session,
  Schedule,
  Settings,
  Stats,
} from './api';
import type { SpeciesId } from './plants';

const SPECIES: [SpeciesId, string, number, number][] = [
  ['sprout', 'Sprout', 0, 0],
  ['succulent', 'Succulent', 30, 0],
  ['camellia', 'Camellia', 45, 0],
  ['pine', 'Pine', 50, 0],
  ['fern', 'Fern', 25, 2],
  ['bamboo', 'Bamboo', 60, 5],
  ['hydrangea', 'Hydrangea', 45, 8],
  ['maple', 'Maple', 90, 10],
  ['sakura', 'Sakura', 60, 15],
  ['wisteria', 'Wisteria', 90, 25],
  ['ginkgo', 'Ginkgo', 120, 40],
];

const CATEGORIES = [
  {
    id: 'chat',
    label: 'Chat & social',
    apps: ['Discord', 'Telegram', 'WhatsApp', 'Signal', 'Messenger', 'Instagram', 'TikTok'],
    found: 2,
  },
  { id: 'media', label: 'Music & video', apps: ['Spotify', 'Netflix', 'Prime Video', 'Disney+', 'VLC'], found: 1 },
  {
    id: 'browsers',
    label: 'Web browsers',
    apps: ['Google Chrome', 'Microsoft Edge', 'Firefox', 'Opera', 'Brave'],
    found: 3,
  },
];
const SUGGESTED = [
  { name: 'Google Chrome', path: '', known: 'chrome' },
  { name: 'Microsoft Edge', path: '', known: 'edge' },
  { name: 'VS Code', path: '', known: 'vscode' },
  { name: 'Obsidian', path: '', known: 'obsidian' },
  { name: 'Claude', path: '', known: 'claude' },
];

export function mockBackend(): Backend {
  const params = new URLSearchParams(location.search);
  const scale = Number(params.get('scale') ?? 30);
  let focusedMinutes = Number(params.get('hours') ?? 12) * 60;
  let streakDays = 6;
  let todayMinutes = 0;

  const handlers: { [K in keyof Events]?: ((payload: Events[K]) => void)[] } = {};
  const emit = <K extends keyof Events>(event: K, payload: Events[K]) =>
    handlers[event]?.forEach((handler) => handler(payload));

  let config: BlockListConfig = {
    mode: params.has('strict') ? 'strict' : 'gentle',
    kind: params.has('allow') ? 'allowOnly' : 'block',
    categories: ['chat'],
    allowed: [],
    steam: true,
    epic: true,
    riot: true,
    apps: [
      { name: 'Discord', path: 'C:\\Apps\\Discord.exe' },
      { name: 'Spotify', path: 'C:\\Apps\\Spotify.exe' },
    ],
    exceptions: [{ name: 'Wallpaper Engine', path: 'C:\\Steam\\wallpaper_engine' }],
  };
  const list = (): BlockList => {
    const launchers = config.steam || config.epic || config.riot ? 'Games' : '';
    const apps = config.apps.length ? `${config.apps.length} apps` : '';
    return {
      name: 'Deep work',
      config,
      found: { steam: 16, epic: 5, riot: 1 },
      categories: CATEGORIES,
      suggested: SUGGESTED,
      summary:
        config.kind === 'allowOnly'
          ? `Only ${config.allowed.length} apps allowed`
          : [launchers, apps && `+${apps}`].filter(Boolean).join(' ') || 'Nothing blocked yet',
    };
  };

  const day = 86_400_000;
  let nextId = 100;
  const plant = (
    species: SpeciesId,
    minutes: number,
    daysAgo: number,
    withered = false,
  ): GardenPlant => ({
    id: nextId++,
    species,
    speciesName: SPECIES.find(([id]) => id === species)![1],
    withered,
    minutes,
    plannedMinutes: withered ? 50 : minutes,
    mode: daysAgo % 3 === 0 ? 'strict' : 'gentle',
    slips: daysAgo % 4,
    createdMs: Date.now() - daysAgo * day - 3_600_000,
  });
  // `?plants=1000` fills the garden with a long history, for checking a big one.
  const many = Number(params.get('plants') ?? 0);
  const kinds = ['pine', 'camellia', 'maple', 'bamboo', 'sprout', 'succulent', 'fern', 'sakura'] as const;
  const plants: GardenPlant[] = params.has('empty')
    ? []
    : many > 0
      ? Array.from({ length: many }, (_, i) => plant(kinds[i % kinds.length], 25 + (i % 6) * 15, Math.floor(i / 3), i % 9 === 4))
      : [
        plant('pine', 120, 0),
        plant('camellia', 60, 1),
        plant('maple', 90, 1),
        plant('bamboo', 75, 2),
        plant('pine', 12, 2, true),
        plant('sprout', 25, 3),
        plant('succulent', 45, 3),
        plant('camellia', 50, 4),
        plant('pine', 120, 5),
        plant('sprout', 25, 6),
        plant('succulent', 30, 7),
        plant('maple', 8, 8, true),
        plant('bamboo', 60, 9),
        plant('camellia', 45, 10),
        plant('pine', 50, 11),
        plant('sprout', 10, 12),
      ];

  // `?schedules` starts with two focus times.
  let schedules: Schedule[] = params.has('schedules')
    ? [
        { id: 1, days: 31, start: 9 * 60, end: 12 * 60, enabled: true },
        { id: 2, days: 96, start: 14 * 60, end: 16 * 60 + 30, enabled: false },
      ]
    : [];
  const settings: Settings = { sound: true, startWithWindows: false, checkUpdates: true };
  let session: Session | null = null;
  let timer: number | undefined;

  const end = (outcome: 'grown' | 'withered') => {
    if (!session) return;
    clearInterval(timer);
    const ended = session;
    session = null;
    if (outcome === 'grown') {
      focusedMinutes += ended.plannedMinutes;
      todayMinutes += ended.plannedMinutes;
      streakDays = Math.max(streakDays, 1);
    }
    const lasted = Math.floor((ended.totalSecs - ended.remainingSecs) / 60);
    plants.unshift(
      plant(
        ended.species,
        outcome === 'grown' ? ended.plannedMinutes : lasted,
        0,
        outcome === 'withered',
      ),
    );
    emit('session-ended', {
      outcome,
      species: ended.species,
      speciesName: ended.speciesName,
      plannedMinutes: ended.plannedMinutes,
      remainingSecs: ended.remainingSecs,
      slips: ended.slips,
      todayMinutes,
      streakDays,
    });
  };

  const api: Backend = {
    async overview(): Promise<Overview> {
      return {
        session,
        focusedMinutes,
        streakDays,
        species: SPECIES.map(([id, name, minMinutes, unlockHours]) => ({
          id,
          name,
          minMinutes,
          unlockHours,
          unlocked: focusedMinutes >= unlockHours * 60,
        })),
        list: list(),
        strictAvailable: !params.has('noservice'),
        minMinutes: 10,
        maxMinutes: 300,
      };
    },
    async garden(): Promise<Garden> {
      const withered = plants.filter((p) => p.withered).length;
      const next = SPECIES.find(([, , , hours]) => focusedMinutes < hours * 60);
      return {
        plants,
        grown: plants.length - withered,
        withered,
        nextUnlock: next ? { name: next[1], hours: next[3] } : null,
      };
    },
    async stats(): Promise<Stats> {
      // Today is the real weekday (Monday first), as the app reports it.
      const today = (new Date().getDay() + 6) % 7;
      const days = [45, 90, 60, 120, 75, 30, 50].map((minutes, i) => (i <= today ? minutes : 0));
      return {
        days,
        today,
        weekMinutes: days.reduce((sum, minutes) => sum + minutes, 0),
        changePercent: 22,
        streakDays,
        monthMinutes: 1410,
        completionPercent: 88,
        totalMinutes: focusedMinutes + 214,
        tempting: [
          { name: 'Steam games', count: 11 },
          { name: 'Discord', count: 4 },
          { name: 'Riot Client', count: 2 },
        ],
      };
    },
    async settings() {
      return { ...settings };
    },
    // A browser tab can't be resized from the page; `?size=520x760` sizes the card instead.
    async resizeCard() {},
    async setSound(on) {
      settings.sound = on;
    },
    async setStartWithWindows(on) {
      settings.startWithWindows = on;
    },
    async setCheckUpdates(on) {
      settings.checkUpdates = on;
    },
    async schedules() {
      return schedules.map((schedule) => ({ ...schedule }));
    },
    async saveSchedule(schedule) {
      if ((schedule.days & 127) === 0) throw 'Pick at least one day.';
      const minutes = schedule.end > schedule.start ? schedule.end - schedule.start : 1440 - schedule.start + schedule.end;
      if (minutes < 10) throw 'A focus time needs at least 10 minutes.';
      if (minutes > 720) throw 'A focus time can be up to 12 hours.';
      if (schedule.id === 0) schedules.push({ ...schedule, id: Math.max(0, ...schedules.map((s) => s.id)) + 1 });
      else schedules = schedules.map((s) => (s.id === schedule.id ? { ...schedule } : s));
      schedules.sort((a, b) => a.start - b.start);
      return this.schedules();
    },
    async deleteSchedule(id) {
      schedules = schedules.filter((s) => s.id !== id);
      return this.schedules();
    },
    // `?update` has a new version waiting.
    async update() {
      return params.has('update') ? { version: '0.3.0', asksAdmin: params.has('noservice') } : null;
    },
    async installUpdate() {
      await new Promise((done) => setTimeout(done, 1500));
      throw "The update couldn't be downloaded. Try again later.";
    },
    async start(minutes, species) {
      const totalSecs = minutes * 60;
      let ticks = 0;
      session = {
        species,
        speciesName: SPECIES.find(([id]) => id === species)![1],
        plannedMinutes: minutes,
        mode: config.mode,
        listName: 'Deep work',
        remainingSecs: totalSecs,
        totalSecs,
        stage: 1,
        slips: 0,
        blocking: ['Games', '+2'],
        closing: [{ label: 'Discord', secsLeft: 8 }],
        exitSecsLeft: null,
        scheduled: params.has('scheduled'),
      };
      timer = setInterval(() => {
        if (!session) return;
        ticks += 1;
        const remainingSecs = Math.max(0, session.remainingSecs - scale);
        const elapsed = totalSecs - remainingSecs;
        const exitSecsLeft =
          session.exitSecsLeft === null ? null : Math.max(0, session.exitSecsLeft - scale);
        session = {
          ...session,
          exitSecsLeft,
          remainingSecs,
          stage: remainingSecs === 0 ? 5 : Math.floor((elapsed * 4) / totalSecs) + 1,
          closing: ticks < 8 ? [{ label: 'Discord', secsLeft: 8 - ticks }] : [],
        };
        if (ticks === 8) emit('app-closed', { label: 'Discord' });
        if (ticks === 12) {
          session = { ...session, slips: session.slips + 1 };
          emit('session-slip', { label: 'Geometry Dash', slips: session.slips });
          emit('app-closed', { label: 'Geometry Dash' });
        }
        if (remainingSecs === 0) end('grown');
        else if (exitSecsLeft === 0) end('withered');
        else emit('session-tick', session);
      }, 1000);
      return session;
    },
    async requestExit() {
      if (!session) return;
      const exitSecsLeft = exitWaitMinutes(session.mode, session.plannedMinutes) * 60;
      emit('session-tick', (session = { ...session, exitSecsLeft }));
    },
    async cancelExit() {
      if (session) emit('session-tick', (session = { ...session, exitSecsLeft: null }));
    },
    async saveList(next) {
      config = next;
      return list();
    },
    async previewClosing(next) {
      if (next.kind === 'allowOnly') {
        const allowed = new Set(next.allowed.map((app) => app.name));
        return ['Discord', 'Google Chrome', 'Spotify', 'VALORANT'].filter((name) => !allowed.has(name));
      }
      return next.categories.includes('chat') ? ['Discord'] : [];
    },
    async pickApp(allow) {
      return allow
        ? { name: 'Figma', path: 'C:\\Apps\\Figma.exe' }
        : { name: 'Notepad', path: 'C:\\Windows\\notepad.exe' };
    },
    async pickException(folder) {
      return folder
        ? { name: 'PEAK', path: 'C:\\Steam\\steamapps\\common\\PEAK' }
        : { name: 'Notepad', path: 'C:\\Windows\\notepad.exe' };
    },
    async hide() {
      location.hash = '#mini';
      location.reload();
    },
    async dragMini() {},
    async openCard() {
      location.hash = '';
      location.reload();
    },
    async quit() {},
    on(event, handler) {
      (handlers[event] ??= [] as never[]).push(handler as never);
    },
  };
  if (params.has('session')) {
    // ?session=wisteria:300 picks the seed and length.
    const [seed, length] = (params.get('session') || 'pine:50').split(':');
    void api.start(Number(length) || 50, seed as SpeciesId);
  }
  return api;
}
