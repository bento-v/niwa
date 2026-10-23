// The chime when a plant is fully grown: two soft bell tones, made on the
// spot so there is no sound file to ship.

/** Plays one bell-like tone: a pure note with a quieter overtone, fading out. */
function bell(audio: AudioContext, frequency: number, at: number, length: number) {
  const level = audio.createGain();
  level.gain.setValueAtTime(0, at);
  level.gain.linearRampToValueAtTime(0.14, at + 0.02);
  level.gain.exponentialRampToValueAtTime(0.0001, at + length);
  level.connect(audio.destination);

  for (const [multiple, share] of [
    [1, 1],
    [2.76, 0.18],
  ]) {
    const tone = audio.createOscillator();
    const toneLevel = audio.createGain();
    tone.type = 'sine';
    tone.frequency.value = frequency * multiple;
    toneLevel.gain.value = share;
    tone.connect(toneLevel).connect(level);
    tone.start(at);
    tone.stop(at + length);
  }
}

export function playChime() {
  try {
    const audio = new AudioContext();
    const now = audio.currentTime;
    bell(audio, 659.25, now, 1.8); // E5
    bell(audio, 987.77, now + 0.22, 2.2); // B5
    setTimeout(() => audio.close(), 3000);
  } catch {
    // No sound device: the notification and the card still say it's done.
  }
}
