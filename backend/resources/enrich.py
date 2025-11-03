# sskj2_from_search_json.py
import json
import re
import time
from urllib.parse import quote

import requests
from bs4 import BeautifulSoup

INPUT_FILE = "wordlist.txt"
OUT_JSONL = "output.jsonl"
OUT_JSON = "output.json"

UA = "Mozilla/5.0 (compatible; sskj2-one-off/1.0)"
TO = 15
SLEEP = 0.2


def clean_words(path):
    words = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            w = line.strip()
            if not w:
                continue
            if len(w) < 3:
                continue
            if w[0].isupper():
                continue
            words.append(w)
    return words


def fetch_search_html(word):
    url = "https://www.fran.si/iskanje"
    params = {"View": "1", "Query": word, "FilteredDictionaryIds": "133"}
    r = requests.get(url, params=params, headers={"User-Agent": UA}, timeout=TO)
    r.raise_for_status()
    return r.text, r.url


def parse_senses_from_search(html):
    soup = BeautifulSoup(html, "html.parser")
    all_senses = []
    first_title = None
    first_href = None

    # pojdi čez vse SSKJ² kartice v rezultatih
    for entry in soup.select("div.list-group.results div.list-group-item.entry"):
        badge = entry.select_one(".badge.dictionary-name")
        if not badge or "SSKJ" not in badge.get_text():
            continue

        # naslov in href prve ujemajoče kartice si zapomnimo
        a = entry.select_one(".entry-content a[href]")
        if first_title is None and a:
            first_title = a.get_text(" ", strip=True)
            href = a["href"]
            first_href = (
                ("https://www.fran.si" + href) if href.startswith("/") else href
            )

        # zberemo vse elemente, ki nosijo razlago
        # opomba: na strani se pojavljata data-group="explanation" in data-group="explanation "
        expl_nodes = entry.select('.entry-content [data-group^="explanation"]')

        # združevanje: zaporedne razlage pripadajo istemu pomenu
        merged = []
        buf = []

        def flush():
            if not buf:
                return
            txt = " ".join(buf).strip()
            # normalizacija presledkov in odstrani končni dvopičje ali podvojeno ločilo
            txt = " ".join(txt.split())
            txt = re.sub(r"\s*[:;,.]\s*$", "", txt)
            if txt:
                merged.append(txt)
            buf.clear()

        # gremo po vseh razlagalnih koščkih v vrstnem redu
        for node in expl_nodes:
            t = node.get_text(" ", strip=True)
            t = " ".join(t.split())
            if not t:
                continue
            # če prepoznamo začetek nove oštevilčene točke zunaj razlage, flush
            # (varovalka, običajno pa je dovolj, da združimo vse zaporedne explanation koščke)
            if re.match(r"^\d+[\.)]\s*$", t):
                flush()
                continue
            buf.append(t)

            # če naslednji sibling ni več explanation, zaključimo ta pomen
            nxt = node.find_next_sibling()
            if (
                not nxt
                or not nxt.has_attr("data-group")
                or not str(nxt.get("data-group", "")).startswith("explanation")
            ):
                flush()

        # fallback, če slučajno ni nič ujetega, poskusi še numerirane odstavke v kartici
        if not merged:
            pieces = []
            current = []
            for n in entry.select(".entry-content > *"):
                txt = n.get_text(" ", strip=True)
                if not txt:
                    continue
                if re.match(r"^\d+[\.)]\s*$", txt):
                    if current:
                        pieces.append(" ".join(current))
                        current = []
                    continue
                current.append(txt)
            if current:
                pieces.append(" ".join(current))
            merged = [re.sub(r"\s+", " ", p).strip() for p in pieces if p]

        all_senses.extend(merged)

    # deduplika ob ohranitvi reda
    seen = set()
    senses = []
    for s in all_senses:
        if s not in seen:
            seen.add(s)
            senses.append(s)

    return (first_title or ""), senses, (first_href or None)


def main():
    words = clean_words(INPUT_FILE)
    all_rows = []
    with open(OUT_JSONL, "w", encoding="utf-8") as outl:
        for i, w in enumerate(words, 1):
            html, search_url = fetch_search_html(w)
            title, senses, href = parse_senses_from_search(html)
            row = {
                "word": w,
                "title": title or w,
                "senses": senses,  # zdaj vključuje homonime 1 in 2, če sta v rezultatih
                "source_url": href or search_url,
            }
            outl.write(json.dumps(row, ensure_ascii=False) + "\n")
            all_rows.append(row)
            print(f"[{i}/{len(words)}] {w}: {len(senses)} pomenov")
            time.sleep(SLEEP)

    with open(OUT_JSON, "w", encoding="utf-8") as outj:
        json.dump(all_rows, outj, ensure_ascii=False, indent=2)

    print(f"končano. zapisano v {OUT_JSONL} in {OUT_JSON}")


if __name__ == "__main__":
    main()
