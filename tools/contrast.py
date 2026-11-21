"""WCAG contrast of the Timelyx accent against the suite surfaces (same pairs as theme::accent_problems)."""


def lum(h):
    r, g, b = [(h >> s & 255) / 255 for s in (16, 8, 0)]
    f = lambda c: c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)


def cr(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


DARK = dict(sidebar=0x0F0F10, bg=0x141415, card=0x19191B, raised=0x232325)
LIGHT = dict(sidebar=0xF4F4F5, bg=0xFAFAFA, card=0xFFFFFF, raised=0xF4F4F5)
ACCENT_DARK, ACCENT_LIGHT = 0xE8669A, 0xB0306A
ON_DARK, ON_LIGHT = 0x141415, 0xFFFFFF

for name, accent, on, surfaces in (("dark", ACCENT_DARK, ON_DARK, DARK), ("light", ACCENT_LIGHT, ON_LIGHT, LIGHT)):
    for s, c in surfaces.items():
        print(f"{name}: accent on {s}: {cr(accent, c):.2f}")
    print(f"{name}: on_accent on accent: {cr(on, accent):.2f}")
