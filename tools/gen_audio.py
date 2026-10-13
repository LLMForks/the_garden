#!/usr/bin/env python3
"""Synthesizes all sound effects and the background music for The Garden.

Everything is generated from scratch with numpy (no samples), deterministic.

    python3 tools/gen_audio.py
"""
import os
import wave

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "assets", "audio")
os.makedirs(OUT, exist_ok=True)
SR = 22050
rng = np.random.RandomState(1234)


def write(name, x, sr=SR):
    x = np.asarray(x, dtype=float)
    peak = np.max(np.abs(x)) or 1.0
    x = x / peak * 0.9
    data = (x * 32767).astype("<i2")
    with wave.open(os.path.join(OUT, name), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(data.tobytes())


def t(dur, sr=SR):
    return np.arange(int(dur * sr)) / sr


def env(n, a=0.005, d=0.1, sr=SR):
    tt = np.arange(n) / sr
    e = np.minimum(1, tt / max(a, 1e-4)) * np.exp(-np.maximum(0, tt - a) / d)
    return e


def lowpass(x, k):
    """simple one-pole lowpass, k in (0,1]: smaller = darker"""
    y = np.zeros_like(x)
    acc = 0.0
    for i in range(len(x)):
        acc += k * (x[i] - acc)
        y[i] = acc
    return y


def noise(dur):
    return rng.uniform(-1, 1, int(dur * SR))


def sweep(f0, f1, dur, shape="sine"):
    tt = t(dur)
    f = f0 * (f1 / f0) ** (tt / dur)
    ph = 2 * np.pi * np.cumsum(f) / SR
    if shape == "tri":
        return 2 / np.pi * np.arcsin(np.sin(ph))
    if shape == "square":
        return np.sign(np.sin(ph)) * 0.5
    return np.sin(ph)


def bell(freq, dur, decay=0.25, sr=SR):
    tt = t(dur, sr)
    x = (np.sin(2 * np.pi * freq * tt) + 0.35 * np.sin(2 * np.pi * freq * 2.01 * tt) * np.exp(-tt / (decay * 0.4))
         + 0.15 * np.sin(2 * np.pi * freq * 3.98 * tt) * np.exp(-tt / (decay * 0.2)))
    return x * env(len(tt), 0.003, decay, sr)


def pluck(freq, dur, sr=SR, bright=0.5):
    """Karplus-Strong string."""
    n = int(dur * sr)
    p = max(2, int(sr / freq))
    buf = np.random.RandomState(int(freq * 10)).uniform(-1, 1, p)
    buf = lowpass(buf, bright)
    out = np.zeros(n)
    for i in range(n):
        out[i] = buf[i % p]
        nxt = buf[(i + 1) % p]
        buf[i % p] = 0.996 * 0.5 * (buf[i % p] + nxt)
    return out * env(n, 0.002, dur * 0.6, sr)


def mix(*parts):
    n = max(len(p) for p in parts)
    out = np.zeros(n)
    for p in parts:
        out[:len(p)] += p
    return out


def delay(x, secs, n):
    out = np.zeros(len(x) + int(secs * SR * n))
    out[:len(x)] += x
    for k in range(1, n + 1):
        o = int(secs * SR * k)
        out[o:o + len(x)] += x * (0.35 ** k)
    return out


def sfx():
    # footsteps (2 variants, soft grassy thump)
    for i in range(2):
        n = noise(0.07)
        x = lowpass(n, 0.12 + 0.05 * i) * env(len(n), 0.002, 0.018)
        x += 0.5 * sweep(160 - i * 20, 70, 0.07) * env(len(n), 0.001, 0.02)
        write("step%d.wav" % i, x)
    # hoe: crunchy dirt
    n = noise(0.22)
    x = lowpass(n, 0.25) * env(len(n), 0.002, 0.06)
    x += 0.8 * sweep(180, 60, 0.22) * env(len(n), 0.001, 0.05)
    x += 0.3 * lowpass(noise(0.22), 0.6) * env(len(n), 0.05, 0.04)
    write("hoe.wav", x)
    # water: bubbly hiss
    d = 0.55
    n = lowpass(noise(d), 0.35) * env(int(d * SR), 0.04, 0.25) * 0.6
    bub = np.zeros(int(d * SR))
    for k in range(9):
        st = int(rng.uniform(0, d - 0.08) * SR)
        b = sweep(rng.uniform(500, 900), rng.uniform(1200, 1800), 0.04) * env(int(0.04 * SR), 0.001, 0.015)
        bub[st:st + len(b)] += b * 0.5
    write("water.wav", n + bub)
    # plant: soft pop
    x = sweep(300, 700, 0.09) * env(int(0.09 * SR), 0.002, 0.03)
    x = mix(x, lowpass(noise(0.05), 0.3) * env(int(0.05 * SR), 0.001, 0.01) * 0.6)
    write("plant.wav", x)
    # harvest: pop + sparkle
    pop = sweep(350, 1000, 0.08) * env(int(0.08 * SR), 0.001, 0.03)
    sp = np.zeros(int(0.45 * SR))
    for k, f in enumerate([1318.5, 1760, 2093]):
        b = bell(f, 0.3, 0.12) * 0.35
        o = int((0.05 + 0.06 * k) * SR)
        sp[o:o + len(b)] += b[:len(sp) - o]
    write("harvest.wav", mix(pop, sp))
    # pickup / coin
    a = bell(1318.5, 0.25, 0.08)
    b = bell(1975.5, 0.35, 0.12)
    x = np.zeros(int(0.42 * SR))
    x[:len(a)] += a
    x[int(0.07 * SR):int(0.07 * SR) + len(b)] += b[:len(x) - int(0.07 * SR)]
    write("coin.wav", x)
    write("pickup.wav", bell(987.8, 0.18, 0.06) + 0.5 * bell(1480, 0.18, 0.05))
    # ui click / open / close
    write("click.wav", sweep(1800, 1200, 0.03, "tri") * env(int(0.03 * SR), 0.001, 0.008))
    o = sweep(400, 900, 0.16, "tri") * env(int(0.16 * SR), 0.01, 0.06) * 0.6
    o = mix(o, lowpass(noise(0.16), 0.2) * env(int(0.16 * SR), 0.03, 0.05) * 0.4)
    write("open.wav", o)
    write("close.wav", sweep(800, 380, 0.13, "tri") * env(int(0.13 * SR), 0.005, 0.05) * 0.6)
    write("select.wav", bell(1046.5, 0.08, 0.03) * 0.6)
    # deny
    x = mix(sweep(220, 180, 0.12, "square") * env(int(0.12 * SR), 0.002, 0.05),
            np.concatenate([np.zeros(int(0.1 * SR)), sweep(180, 150, 0.14, "square") * env(int(0.14 * SR), 0.002, 0.06)]))
    write("deny.wav", lowpass(x, 0.3))
    # cook: sizzle then ding
    d = 1.1
    s = lowpass(noise(d), 0.7) * (0.6 + 0.4 * np.sin(2 * np.pi * 13 * t(d)) ** 2) * env(int(d * SR), 0.05, 0.4) * 0.5
    ding = bell(1568, 0.8, 0.35)
    x = np.zeros(int(1.6 * SR))
    x[:len(s)] += s
    o = int(0.75 * SR)
    x[o:o + len(ding)] += ding[:len(x) - o]
    write("cook.wav", x)
    # level up jingle
    notes = [523.25, 659.25, 783.99, 1046.5, 1318.5]
    x = np.zeros(int(1.6 * SR))
    for k, f in enumerate(notes):
        b = bell(f, 0.9, 0.3) * (0.8 if k < 4 else 1.0)
        o = int(0.09 * k * SR)
        x[o:o + len(b)] += b[:len(x) - o]
        pl = pluck(f / 2, 0.6) * 0.4
        x[o:o + len(pl)] += pl[:len(x) - o]
    write("levelup.wav", delay(x, 0.12, 2))
    # unlock / quest complete (shorter)
    x = np.zeros(int(0.9 * SR))
    for k, f in enumerate([783.99, 987.77, 1174.66]):
        b = bell(f, 0.6, 0.2)
        o = int(0.07 * k * SR)
        x[o:o + len(b)] += b[:len(x) - o]
    write("quest.wav", x)
    # meow (cat) and yip (fox)
    tt = t(0.32)
    f = 520 + 380 * np.sin(np.pi * tt / 0.32) ** 1.5
    ph = 2 * np.pi * np.cumsum(f) / SR
    m = (np.sin(ph) + 0.4 * np.sin(2 * ph) + 0.2 * np.sin(3 * ph)) * env(len(tt), 0.03, 0.12)
    write("meow.wav", lowpass(m, 0.5))
    tt = t(0.16)
    f = 900 + 500 * np.sin(np.pi * tt / 0.16)
    ph = 2 * np.pi * np.cumsum(f) / SR
    write("yip.wav", lowpass((np.sin(ph) + 0.5 * np.sin(2 * ph)) * env(len(tt), 0.005, 0.05), 0.6))
    # purr / pet
    tt = t(0.6)
    p = lowpass(noise(0.6), 0.08) * (0.5 + 0.5 * np.sin(2 * np.pi * 24 * tt)) * env(len(tt), 0.05, 0.3)
    write("purr.wav", p)
    # splash (refill)
    d = 0.5
    s = lowpass(noise(d), 0.5) * env(int(d * SR), 0.005, 0.12)
    s = mix(s, sweep(300, 900, 0.2) * env(int(0.2 * SR), 0.01, 0.06) * 0.3)
    write("splash.wav", s)
    # place decor: wooden knock
    x = mix(sweep(420, 260, 0.08, "tri") * env(int(0.08 * SR), 0.001, 0.025),
            lowpass(noise(0.05), 0.4) * env(int(0.05 * SR), 0.001, 0.01) * 0.6)
    x = np.concatenate([x, np.zeros(int(0.03 * SR)), x * 0.6])
    write("place.wav", x)
    # boat horn (soft) for NPC arrival
    tt = t(0.9)
    h = (np.sin(2 * np.pi * 220 * tt) + 0.5 * np.sin(2 * np.pi * 330 * tt) + 0.3 * np.sin(2 * np.pi * 440 * tt))
    h *= np.minimum(1, tt / 0.1) * np.exp(-np.maximum(0, tt - 0.5) / 0.15)
    write("horn.wav", lowpass(h, 0.2))
    # bird chirps (3 variants)
    for i in range(3):
        x = np.zeros(int(0.5 * SR))
        o = 0
        for k in range(2 + i):
            c = sweep(rng.uniform(2500, 3400), rng.uniform(3600, 4800), 0.06) * env(int(0.06 * SR), 0.003, 0.02)
            x[o:o + len(c)] += c[:len(x) - o]
            o += int(rng.uniform(0.08, 0.12) * SR)
        write("bird%d.wav" % i, x * 0.6)
    # night crickets loop (soft)
    d = 4.0
    tt = t(d)
    c = np.sin(2 * np.pi * 4200 * tt) * (np.sin(2 * np.pi * 28 * tt) > 0.3) * (np.sin(2 * np.pi * 0.9 * tt) > 0.0)
    c = c * 0.15 + lowpass(noise(d), 0.05) * 0.2
    write("night.wav", c)
    # waves ambience loop
    d = 6.0
    tt = t(d)
    w = lowpass(noise(d), 0.06) * (0.55 + 0.45 * np.sin(2 * np.pi * tt / d * 2) ** 2)
    write("waves.wav", w)


# ---------------------------------------------------------------- music
MSR = 32000


def note_freq(n):
    return 440.0 * 2 ** ((n - 69) / 12)


def music():
    bpm = 78
    beat = 60 / bpm
    bars = 16
    total = bars * 4 * beat
    n = int(total * MSR)
    out = np.zeros(n + MSR * 4)
    tt_all = np.arange(len(out)) / MSR
    # chord progression (midi roots + intervals): Fmaj7 - Em7 - Dm7 - Cmaj7(add9) -> repeat with variation
    prog = [(53, [0, 4, 7, 11]), (52, [0, 3, 7, 10]), (50, [0, 3, 7, 10]), (48, [0, 4, 7, 14]),
            (53, [0, 4, 7, 11]), (55, [0, 4, 7, 9]), (57, [0, 3, 7, 10]), (55, [0, 4, 7, 11])]
    r = np.random.RandomState(42)

    def add(sig, start):
        o = int(start * MSR)
        out[o:o + len(sig)] += sig[:len(out) - o]

    def soft_pad(freq, dur):
        tt = np.arange(int(dur * MSR)) / MSR
        x = np.zeros_like(tt)
        for k, a in ((1, 1.0), (2, 0.25), (3, 0.08)):
            x += a * np.sin(2 * np.pi * freq * k * tt + 0.3 * np.sin(2 * np.pi * 0.4 * tt))
        e = np.minimum(1, tt / 0.6) * np.minimum(1, (dur - tt) / 0.8)
        return x * np.clip(e, 0, 1)

    def kalimba(freq, dur=1.6):
        tt = np.arange(int(dur * MSR)) / MSR
        x = np.sin(2 * np.pi * freq * tt) + 0.25 * np.sin(2 * np.pi * freq * 5.4 * tt) * np.exp(-tt / 0.03)
        x += 0.12 * np.sin(2 * np.pi * freq * 2 * tt) * np.exp(-tt / 0.2)
        return x * np.exp(-tt / 0.55) * np.minimum(1, tt / 0.002)

    pent = [0, 2, 4, 7, 9]
    for bar in range(bars):
        root, iv = prog[bar % len(prog)]
        start = bar * 4 * beat
        # pad
        for i in iv:
            add(soft_pad(note_freq(root + i), 4 * beat + 0.6) * 0.07, start)
        # bass on 1 and 3
        for b in (0, 2.5):
            tt = np.arange(int(1.4 * MSR)) / MSR
            bs = np.sin(2 * np.pi * note_freq(root - 12) * tt) * np.exp(-tt / 0.6) * np.minimum(1, tt / 0.01)
            add(bs * 0.22, start + b * beat)
        # arpeggio (gentle 8ths with swing-less feel)
        arp = [iv[0], iv[1], iv[2], iv[3], iv[2], iv[1], iv[2], iv[3]]
        for k, a in enumerate(arp):
            if bar < 2 and k % 2:
                continue
            add(kalimba(note_freq(root + 12 + a), 1.2) * 0.08, start + k * beat / 2)
        # melody from pentatonic over C, sparse
        if bar >= 2:
            pos = 0.0
            while pos < 4:
                if r.rand() < 0.62:
                    deg = pent[r.randint(0, 5)]
                    octv = 72 + (12 if r.rand() < 0.25 else 0)
                    add(kalimba(note_freq(octv + deg), 1.8) * 0.16, start + pos * beat)
                pos += r.choice([0.5, 1.0, 1.0, 1.5])
        # soft shaker on off-beats
        for k in range(8):
            if k % 2 == 1:
                tt = np.arange(int(0.05 * MSR)) / MSR
                sh = r.uniform(-1, 1, len(tt)) * np.exp(-tt / 0.012)
                add(sh * 0.025, start + k * beat / 2)
    # fold the tail back to the start so the loop is seamless
    tail = out[n:]
    out = out[:n]
    out[:len(tail)] += tail
    # gentle lowpass for warmth
    acc = 0.0
    y = np.zeros_like(out)
    k = 0.35
    for i in range(len(out)):
        acc += k * (out[i] - acc)
        y[i] = acc
    write("music.wav", y, MSR)


if __name__ == "__main__":
    sfx()
    music()
    print("audio generated")
