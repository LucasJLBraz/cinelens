#!/usr/bin/env python3
"""Measures the app's speed on a phone over adb, and prints the Perfil, as a release check asks.

    scripts/device-speed.py SERIAL

The phone may hold the owner's real account, so every tap goes through `tocar`: it finds an
allowed label on the screen and refuses anything else. Nothing here gives a nota or a mark.
Times run from the end of the tap in the system log to the app's frames in SurfaceFlinger.
"""
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

APP = "io.github.lucasjlbraz.cinelens"
# What only navigates. Add here only labels that change no data.
PERMITIDOS = {"Início", "Explorar", "Perfil", "Tudo", "Animes"}
serial = sys.argv[1]


def adb(*args):
    return subprocess.run(["adb", "-s", serial, *args], capture_output=True, text=True, stdin=subprocess.DEVNULL, check=True).stdout


def shell(*args):
    return adb("shell", *args)


def arvore():
    shell("uiautomator", "dump", "/sdcard/cinelens-ui.xml")
    xml = shell("cat", "/sdcard/cinelens-ui.xml")
    shell("rm", "/sdcard/cinelens-ui.xml")
    return ET.fromstring(xml[xml.index("<"):]).iter("node")


def primeira_capa():
    """The center of the first poster on screen, whose tap only opens the obra's page."""
    for no in arvore():
        if no.get("class") == "android.widget.Image" and no.get("content-desc"):
            x1, y1, x2, y2 = map(int, re.findall(r"\d+", no.get("bounds")))
            if x2 > x1 and y2 > y1:
                return (x1 + x2) // 2, (y1 + y2) // 2
    sys.exit("nenhuma capa na tela: parei sem tocar em nada")


def nos():
    """What is on screen: (text, clickable, bounds), hidden elements left out."""
    for no in arvore():
        x1, y1, x2, y2 = map(int, re.findall(r"\d+", no.get("bounds")))
        if no.get("text") and x2 > x1 and y2 > y1:
            yield no.get("text"), no.get("clickable") == "true", (x1, y1, x2, y2)


def centro(rotulo):
    """The center of the element showing exactly `rotulo`, a button first, or None."""
    # A WebView just opened takes seconds to show up in the dump.
    for _ in range(8):
        achados = sorted((n for n in nos() if n[0] == rotulo), key=lambda n: not n[1])
        if achados:
            break
        time.sleep(2)
    else:
        return None
    x1, y1, x2, y2 = achados[0][2]
    return (x1 + x2) // 2, (y1 + y2) // 2


def tocar(rotulo):
    if rotulo not in PERMITIDOS:
        sys.exit(f"recusado: {rotulo} não é um toque só de navegação")
    ponto = centro(rotulo)
    if not ponto:
        sys.exit(f"{rotulo} não está na tela: parei sem tocar em nada")
    return ponto


def quadros():
    camadas = [l.strip() for l in shell("dumpsys", "SurfaceFlinger", "--list").splitlines() if APP in l and "$_" in l]
    # Quoted: the remote shell would expand the layer's `$_`.
    linhas = shell("dumpsys", "SurfaceFlinger", "--latency", f"'{camadas[-1]}'").splitlines()[1:]
    campos = [l.split() for l in linhas]
    return [int(c[1]) / 1e9 for c in campos if len(c) == 3 and 0 < int(c[1]) < 1 << 62]


def medir(x, y):
    """(first frame, last frame) in ms after a tap at a point `tocar` allowed."""
    shell("logcat", "-c")
    shell("input", "tap", str(x), str(y))
    time.sleep(1.5)
    toques = re.findall(r"^\s*(\d+\.\d+).*ViewPostIme pointer 1", shell("logcat", "-d", "-v", "monotonic"), re.M)
    if not toques:
        sys.exit("o toque não apareceu no log do sistema")
    fim = float(toques[-1])
    depois = [q for q in quadros() if q > fim]
    if not depois:
        sys.exit("nenhum quadro depois do toque: a medida falhou")
    return round((depois[0] - fim) * 1000), round((depois[-1] - fim) * 1000)


def voltar():
    shell("input", "keyevent", "KEYCODE_BACK")
    time.sleep(0.8)


def main():
    shell("am", "force-stop", APP)
    shell("monkey", "-p", APP, "-c", "android.intent.category.LAUNCHER", "1")
    time.sleep(3)

    abas = {nome: tocar(nome) for nome in ["Início", "Explorar", "Perfil"]}
    print("abas", [medir(*abas[nome]) for nome in ["Início", "Perfil", "Explorar"] * 3])

    # Explorar is open: its filters are on screen.
    animes, tudo = tocar("Animes"), tocar("Tudo")
    print("filtro para Animes e de volta para Tudo", [medir(*p) for p in [animes, tudo] * 3])

    medir(*animes)
    obra = primeira_capa()
    paginas = []
    for _ in range(5):
        paginas.append(medir(*obra))
        voltar()
    print("página do primeiro anime", paginas)

    # To compare before and after an update, without a screenshot.
    medir(*abas["Perfil"])
    print("Perfil:", " | ".join(n[0] for n in nos()))


main()
