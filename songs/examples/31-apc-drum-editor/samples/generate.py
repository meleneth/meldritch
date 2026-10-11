#!/usr/bin/env python3
"""Original offline 909-inspired hit fixtures. No Roland samples or emulator.

Fixed-seed noise, damped oscillators, pitch sweeps and metallic partials; see
README.md. This tool writes only this example's generated WAV assets.
"""
import math
from pathlib import Path
import struct
import wave

RATE = 48000
VOICES = {"kick": .55, "snare": .32, "clap": .25, "closed-hat": .10,
          "open-hat": .50, "low-tom": .40, "high-tom": .32, "ride": .75}


def render(name, duration):
    state, previous, highpass, phase = 0x909, 0.0, 0.0, 0.0
    samples = bytearray()
    for index in range(round(duration * RATE)):
        t = index / RATE
        state = (1664525 * state + 1013904223) & 0xffffffff
        noise = state / 2147483648 - 1
        highpass = .7 * (highpass + noise - previous)
        previous = noise
        if name == "kick":
            phase += math.tau * (52 + 90 * math.exp(-t * 65)) / RATE
            value = .9 * math.sin(phase) * math.exp(-t * 11) + .15 * highpass * math.exp(-t * 280)
        elif name == "snare":
            body = math.sin(math.tau * 185 * t) + .4 * math.sin(math.tau * 330 * t)
            value = .4 * body * math.exp(-t * 24) + .65 * highpass * math.exp(-t * 19)
        elif name == "clap":
            envelope = sum(math.exp(-(t - onset) * 230) for onset in (0, .011, .022) if t >= onset)
            value = highpass * (.6 * envelope + .3 * math.exp(-t * 24))
        elif name in ("closed-hat", "open-hat", "ride"):
            metal = sum(math.sin(math.tau * frequency * t) for frequency in (3200, 4780, 6100, 7300, 8900, 10400)) / 6
            decay = {"closed-hat": 65, "open-hat": 12, "ride": 7}[name]
            value = (.6 * metal + .35 * highpass) * math.exp(-t * decay)
            if name == "ride":
                value += .18 * math.sin(math.tau * 2100 * t) * math.exp(-t * 6)
        else:
            frequency = 95 if name == "low-tom" else 165
            phase += math.tau * (frequency + 45 * math.exp(-t * 35)) / RATE
            value = .8 * math.sin(phase) * math.exp(-t * 16) + .08 * highpass * math.exp(-t * 80)
        fade = min(1.0, t * 5000, (duration - t) * 1000)
        value = math.tanh(value * 1.4) * fade
        samples.extend(struct.pack("<h", round(value * 28000)))
    return samples


if __name__ == "__main__":
    directory = Path(__file__).resolve().parent / "audio"
    directory.mkdir(exist_ok=True)
    for name, duration in VOICES.items():
        with wave.open(str(directory / f"{name}.wav"), "wb") as output:
            output.setparams((1, 2, RATE, 0, "NONE", "not compressed"))
            output.writeframes(render(name, duration))
