#!/usr/bin/env python3
"""Sons d'AlpIRC : courts sons d'événements, tous ORIGINAUX, calculés ici note par note.

Aucun enregistrement, aucun échantillon repris d'ailleurs : chaque son est une petite synthèse (cloche par partiels
inharmoniques, piano électrique par modulation de fréquence, marimba, bruit filtré tiré d'un générateur
pseudo-aléatoire à graine fixe). Le résultat est donc identique à chaque exécution.

Sortie : `assets/sounds/*.wav`, PCM 16 bits mono à 22 050 Hz, crête à -6 dB environ (des alertes, pas des
alarmes). L'appli les intègre (`crates/shell/src/assets.rs`) et les installe dans `<données>/sounds/` au démarrage
s'ils manquent (sans jamais écraser un fichier de l'utilisateur).

Régénérer, depuis la racine du dépôt (Python 3, sans bibliothèque) :

    python3 assets/sounds/src/make_sounds.py
"""

import math
import os
import struct
import wave

RATE = 22050
PEAK = 0.5  # environ -6 dB
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")

# Fréquences des notes (la à 440 Hz).
NOTES = {
    "G4": 392.00, "C5": 523.25, "D5": 587.33, "E5": 659.25, "G5": 783.99, "A5": 880.00,
    "C6": 1046.50, "D6": 1174.66, "E6": 1318.51, "F6": 1396.91, "G6": 1567.98, "B6": 1975.53,
}


def silence(seconds):
    return [0.0] * int(seconds * RATE)


def mix(dst, src, at):
    """Ajoute `src` dans `dst` à partir de `at` secondes (agrandit `dst` au besoin)."""
    start = int(at * RATE)
    end = start + len(src)
    if end > len(dst):
        dst.extend([0.0] * (end - len(dst)))
    for i, v in enumerate(src):
        dst[start + i] += v


def attack(i, ms=4.0):
    """Montée douce des premières millisecondes (pas de clic)."""
    n = ms * RATE / 1000.0
    return 1.0 if i >= n else i / n


def bell(freq, dur, decay=0.25, bright=1.0):
    """Cloche : partiels inharmoniques qui s'éteignent d'autant plus vite qu'ils sont aigus."""
    partials = [(1.0, 1.0), (2.76, 0.45 * bright), (5.40, 0.22 * bright), (8.93, 0.08 * bright)]
    out = []
    for i in range(int(dur * RATE)):
        t = i / RATE
        v = 0.0
        for ratio, amp in partials:
            v += amp * math.exp(-t / (decay / ratio ** 0.7)) * math.sin(2 * math.pi * freq * ratio * t)
        out.append(v * attack(i))
    return out


def epiano(freq, dur, decay=0.3, index=2.5):
    """Piano électrique : modulation de fréquence (rapport 1:1) dont l'indice retombe vite."""
    out = []
    for i in range(int(dur * RATE)):
        t = i / RATE
        env = math.exp(-t / decay)
        mod = index * math.exp(-t / 0.06) * math.sin(2 * math.pi * freq * t)
        out.append(env * math.sin(2 * math.pi * freq * t + mod) * attack(i))
    return out


def marimba(freq, dur, decay=0.18):
    """Lame de bois : fondamentale et 4e partiel bref."""
    out = []
    for i in range(int(dur * RATE)):
        t = i / RATE
        v = math.exp(-t / decay) * math.sin(2 * math.pi * freq * t)
        v += 0.35 * math.exp(-t / 0.025) * math.sin(2 * math.pi * freq * 3.93 * t)
        out.append(v * attack(i, 2.0))
    return out


def soft_square(freq, dur, decay=0.08):
    """Carré adouci (harmoniques impaires 1, 3, 5) : le « bip-boup » des vieux modems, en sourdine."""
    out = []
    for i in range(int(dur * RATE)):
        t = i / RATE
        env = math.exp(-t / decay) * min(1.0, (dur - t) * 60.0)
        w = 2 * math.pi * freq * t
        v = math.sin(w) + math.sin(3 * w) / 3 * 0.6 + math.sin(5 * w) / 5 * 0.3
        out.append(env * v * attack(i))
    return out


class Lcg:
    """Générateur pseudo-aléatoire à graine fixe (même son à chaque exécution)."""

    def __init__(self, seed):
        self.s = seed

    def next(self):
        self.s = (self.s * 1103515245 + 12345) & 0x7FFFFFFF
        return self.s / 0x3FFFFFFF - 1.0


def thud(dur=0.28):
    """Coup sourd : sinus qui chute de 190 à 60 Hz, et un souffle bref filtré (passe-bas à un pôle)."""
    rnd = Lcg(1987)
    out = []
    phase = 0.0
    low = 0.0
    for i in range(int(dur * RATE)):
        t = i / RATE
        f = 60 + 130 * math.exp(-t / 0.05)
        phase += 2 * math.pi * f / RATE
        body = math.exp(-t / 0.09) * math.sin(phase)
        low += 0.18 * (rnd.next() - low)
        noise = 0.5 * math.exp(-t / 0.02) * low
        out.append((body + noise) * attack(i, 1.5))
    return out


def tick(dur=0.06):
    """Petit clic feutré : sinus aigu très bref."""
    out = []
    for i in range(int(dur * RATE)):
        t = i / RATE
        out.append(math.exp(-t / 0.012) * math.sin(2 * math.pi * 1750 * t) * attack(i, 1.0))
    return out


def normalize(samples, peak=PEAK, tail_ms=12.0):
    """Ramène la crête à `peak` et éteint la fin en douceur."""
    m = max(1e-9, max(abs(v) for v in samples))
    n = len(samples)
    tail = int(tail_ms * RATE / 1000.0)
    out = []
    for i, v in enumerate(samples):
        fade = 1.0 if i < n - tail else (n - i) / tail
        out.append(v / m * peak * fade)
    return out


def write(name, samples, peak=PEAK):
    path = os.path.normpath(os.path.join(OUT, name))
    data = normalize(samples, peak)
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", max(-32767, min(32767, int(round(v * 32767))))) for v in data))
    print(f"{name}: {len(data) / RATE:.2f} s, {os.path.getsize(path)} octets")


def seq(parts):
    """Suite de (début en secondes, échantillons)."""
    out = []
    for at, s in parts:
        mix(out, s, at)
    return out


def main():
    n = NOTES
    # On vous nomme : deux notes de cloche claires, une quinte vers le haut.
    write("alpirc-highlight.wav", seq([(0.0, bell(n["G5"], 0.45, 0.22)), (0.09, bell(n["D6"], 0.40, 0.22))]))
    # Message privé : deux notes de piano électrique qui montent (do, fa).
    write("alpirc-query.wav", seq([(0.0, epiano(n["C6"], 0.16, 0.08)), (0.085, epiano(n["F6"], 0.24, 0.12))]))
    # Notify : arpège montant de marimba (do, mi, sol).
    write("alpirc-notify.wav", seq([(0.0, marimba(n["C5"], 0.3)), (0.08, marimba(n["E5"], 0.3)), (0.16, marimba(n["G5"], 0.35))]))
    # Offre DCC : « boup-bip » en sourdine.
    write("alpirc-dcc.wav", seq([(0.0, soft_square(n["D5"], 0.10)), (0.13, soft_square(n["A5"], 0.12))]), 0.4)
    # Connexion : trois notes de piano électrique qui montent (sol, ré, sol).
    write("alpirc-connect.wav", seq([(0.0, epiano(n["G4"], 0.3, 0.12)), (0.09, epiano(n["D5"], 0.3, 0.12)), (0.18, epiano(n["G5"], 0.4, 0.18))]))
    # Déconnexion : les mêmes, qui descendent.
    write("alpirc-disconnect.wav", seq([(0.0, epiano(n["G5"], 0.3, 0.12)), (0.09, epiano(n["D5"], 0.3, 0.12)), (0.18, epiano(n["G4"], 0.4, 0.18))]))
    # Expulsé : coup sourd.
    write("alpirc-kick.wav", thud())
    # Invité : sonnette « ding-dong » (mi, do).
    write("alpirc-invite.wav", seq([(0.0, bell(n["E5"], 0.5, 0.3, 0.8)), (0.28, bell(n["C5"], 0.6, 0.35, 0.8))]))
    # Message d'un canal inactif : petit clic, discret.
    write("alpirc-message.wav", tick(), 0.3)
    # Son à tout faire (`/splay`, `/sound`) : une cloche.
    write("alpirc-ding.wav", bell(n["A5"], 0.6, 0.3))


if __name__ == "__main__":
    main()
