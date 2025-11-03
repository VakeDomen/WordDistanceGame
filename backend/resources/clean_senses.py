# clean_senses_with_ollama.py
import http.client
import json
import logging
import re
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Any, Dict, List, Optional, Tuple

INPUT_PATH = "output.jsonl"
OUTPUT_PATH = "cleaned_senses_output.jsonl"
ERRORS_PATH = "errors_cleaned_senses_output.jsonl"

# tvoj strežnik
OLLAMA_HOST = "hivecore.famnit.upr.si"
OLLAMA_PORT = 6666
MODEL_NAME = "hf.co/unsloth/Qwen3-4b-Instruct-2507-GGUF:UD-Q4_K_XL"

# paralelizacija
MAX_WORKERS = 20

REQUEST_DELAY_SEC = 0.02  # kratek premor v niti po uspešnem klicu
MAX_RETRIES = 4
HTTP_TIMEOUT_SEC = 45

SYSTEM_PROMPT = (
    "Deluješ kot leksikograf. Dobiš slovensko besedo, naslov in seznam pomenov iz SSKJ², "
    "kjer so razlage včasih poškodovane, združene ali vsebujejo primere rabe. "
    "Naloga: vrni čist seznam pomenov v slovenščini kot polne povedi in paralelni seznam angleških prevodov. "
    "Navodila: "
    "1) Ohrani vse dejanske pomene. Odstrani šum, primere rabe, bibliografske opombe in gole enobesedne drobce. "
    "2) Združi koščke, ki tvorijo en pomen, in razdruži po pomoti zlepljene pomene. "
    "3) Ohrani približen vrstni red. "
    "4) Dedupliciraj. "
    "5) Če je izvorni seznam že pravilen, ga vrni nespremenjen. "
    "6) Vrni STROGO JSON objekt s polji: "
    '{"senses": [<slovenske razlage>], "senses_en": [<angleške razlage>]}. '
    "Brez dodatnega besedila.\n\n"
    "Primeri:\n"
    "Vhod:\n"
    '{"word": "bombaž", "title": "bombáž", "senses": ["semenska vlakna bombaževca, ki se uporabljajo kot tekstilna surovina", "bombaževca", "tkanina iz te surovine", "razstrelivo z veliko rušilno močjo"]}\n'
    "Izhod:\n"
    '{"senses": ["Semenska vlakna bombaževca, ki se uporabljajo kot tekstilna surovina.", "Tkanina izdelana iz bombažnih vlaken.", "Vrsta razstreliva z veliko rušilno močjo."], '
    '"senses_en": ["Seed fibers of the cotton plant used as a textile raw material.", "Fabric made from cotton fibers.", "A type of explosive with great destructive power."]}\n\n'
    "Vhod:\n"
    '{"word": "konjiček", "title": "konjíček", "senses": ["manjšalnica od konj", "z avtomobilom, motornim kolesom", "najljubše delo v prostem času", "majhna riba s cevastim gobcem in konju podobno glavo, Hippocampus"]}\n'
    "Izhod:\n"
    '{"senses": ["Manjšalnica od ‘konj’, torej majhen konj.", "Najljubše delo ali dejavnost v prostem času.", "Majhna morska riba s cevastim gobcem in konju podobno glavo (morski konjiček)."], '
    '"senses_en": ["Diminutive of ‘horse’, i.e., a small horse.", "A favorite hobby or pastime.", "A small marine fish with a tubular snout and horse-like head (seahorse)."]}\n\n'
    "Vhod:\n"
    '{"word": "prepir", "title": "prepír", "senses": ["medsebojno izražanje nesoglasja z izjavami, mnenjem drugega, navadno glasno, ostro", "nehajte se prepirati", "da se ne bomo več prepirali", "spori", "stanje, ki ga povzroči tako izražanje nesoglasja", "se želi, hoče prepirati"]}\n'
    "Izhod:\n"
    '{"senses": ["Medsebojno izražanje nesoglasja z izjavami, navadno glasno in ostro.", "Stanje ali razmerje, ki ga povzroči tako izražanje nesoglasja.", "Spor kot rezultat ali oblika nesoglasja."], '
    '"senses_en": ["Mutual expression of disagreement through statements, usually loud and sharp.", "A state or situation caused by such disagreement.", "A quarrel as a result or form of disagreement."]}\n\n"'
    "Vhod:\n"
    '{"word": "instrument", "title": "instrumènt", "senses": ["listina :", "zelo natančna priprava, ki se uporablja pri specializiranem strokovnem, znanstvenem delu", "priprava za proizvajanje tonov; glasbilo", "kar se rabi ali je namenjeno za dosego določenega cilja; sredstvo, pripomoček"]}\n'
    "Izhod:\n"
    '{"senses": ["V pravnem in diplomatskem jeziku: listina, uradni akt ali mednarodni dokument.", "Zelo natančna priprava, ki se uporablja pri specializiranem strokovnem ali znanstvenem delu.", "Priprava za proizvajanje tonov, torej glasbilo.", "Kar se uporablja kot sredstvo ali pripomoček za dosego določenega cilja."], '
    '"senses_en": ["In legal and diplomatic language: a deed, official act, or international document.", "A very precise device used in specialized professional or scientific work.", "A device for producing tones, i.e., a musical instrument.", "Something used as a means or tool to achieve a particular goal."]}\n\n"'
    "Vhod:\n"
    '{"word": "redukcija", "title": "redúkcija", "senses": ["glagolnik od reducirati", "reducirane oblike"]}\n'
    "Izhod:\n"
    '{"senses": ["Dejanje ali proces zmanjševanja, poenostavljanja ali pretvarjanja česa na bolj preprosto obliko.", "Skupina ali nabor oblik, ki nastanejo kot rezultat zmanjšanja ali poenostavitve (npr. jezikovne oblike)."], '
    '"senses_en": ["The act or process of reducing, simplifying, or converting something to a simpler form.", "A set of forms that result from reduction or simplification (e.g., linguistic forms)."]}\n\n"'
    "Vhod:\n"
    '{"word": "obris", "title": "obrís", "senses": ["nejasno, neostro vidna meja, rob česa", "velikost", "silhueta", "črta, risba, ki prikazuje zunanji rob, robove česa", "kar kaže, nakazuje kaj brez podrobnosti", "je postajala določnejša"]}\n'
    "Izhod:\n"
    '{"senses": ["Zunanja meja ali rob česa, ki je viden nejasno ali neostro.", "Silhueta ali zunanji okvir predmeta ali telesa.", "Risba ali črta, ki prikazuje zunanji rob predmeta.", "Znak ali nakazovanje česa brez podrobnosti, oris."], '
    '"senses_en": ["The outer boundary or edge of something seen indistinctly.", "A silhouette or outer outline of an object or body.", "A drawing or line showing the outer edge of an object.", "An indication or sketch of something without details; an outline."]}\n\n"'
    "Vhod:\n"
    '{"word": "paša", "title": "páša", "senses": ["visok vojaški ali civilni oblastnik", "glagolnik od pasti pasem", "kar se (po)pase", "je povzročil izredno zanimanje", "kar si kdo z zanimanjem in zadovoljstvom ogleduje"]}\n'
    "Izhod:\n"
    '{"senses": ["Visok turški vojaški ali civilni dostojanstvenik.", "Dejanje paše živine; proces, ko se živina pase.", "Količina ali hrana, ki jo živina popase; pašišče.", "Kar si kdo z zanimanjem in zadovoljstvom ogleduje; prava ‘paša za oči’."] , '
    '"senses_en": ["A high-ranking Turkish military or civil official (pasha).", "The act of grazing; the process of animals feeding on pasture.", "The feed or amount grazed; a pasture.", "Something pleasing to look at; a treat for the eyes."]}"'
)


# -------------- helpers --------------
def _strip_code_fences(s: str) -> str:
    s = s.strip()
    m = re.search(r"```(?:json)?\s*(.*?)```", s, re.DOTALL | re.IGNORECASE)
    return m.group(1).strip() if m else s


def _scan_last_balanced_fragment(s: str, open_ch: str, close_ch: str) -> Optional[str]:
    """
    Scan from the end and return the last balanced {...} or [...] fragment, if any.
    Works for plain JSON without string-escape awareness, which is fine for our LLM outputs.
    """
    depth = 0
    end = -1
    # find last closing bracket
    for i in range(len(s) - 1, -1, -1):
        if s[i] == close_ch:
            end = i
            break
    if end == -1:
        return None
    # walk backwards to find the matching opening bracket
    for i in range(end, -1, -1):
        ch = s[i]
        if ch == close_ch:
            depth += 1
        elif ch == open_ch:
            depth -= 1
            if depth == 0:
                return s[i : end + 1]
    return None


def extract_json_from_text(s: str) -> Optional[str]:
    """
    Try to pull a JSON object or array out of a model message.
    Handles code fences and trailing chatter.
    """
    s = _strip_code_fences(s)

    # 1) already clean JSON
    t = s.strip()
    if (t.startswith("{") and t.endswith("}")) or (
        t.startswith("[") and t.endswith("]")
    ):
        return t

    # 2) try last {...}
    obj = _scan_last_balanced_fragment(t, "{", "}")
    if obj:
        return obj

    # 3) try last [...]
    arr = _scan_last_balanced_fragment(t, "[", "]")
    if arr:
        return arr

    return None


def call_ollama_chat(messages: List[Dict[str, str]]) -> Optional[Any]:
    body = {
        "model": MODEL_NAME,
        "messages": messages,
        "stream": False,
        "options": {
            "temperature": 0.7,
            "min_p": 0.0,
            "top_p": 0.8,
            "top_k": 20,
            "presence_penalty": 1.0,
        },
        "format": {
            "type": "json_schema",
            "json_schema": {
                "name": "cleaned_senses_schema",
                "schema": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["senses", "senses_en"],
                    "properties": {
                        "senses": {
                            "type": "array",
                            "items": {"type": "string"},
                            "minItems": 1,
                        },
                        "senses_en": {
                            "type": "array",
                            "items": {"type": "string"},
                            "minItems": 0,
                        },
                    },
                },
            },
        },
    }
    payload = json.dumps(body).encode("utf-8")

    for attempt in range(MAX_RETRIES + 1):
        conn = http.client.HTTPConnection(
            OLLAMA_HOST, OLLAMA_PORT, timeout=HTTP_TIMEOUT_SEC
        )
        try:
            conn.request(
                "POST",
                "/api/chat",
                body=payload,
                headers={"Content-Type": "application/json"},
            )
            resp = conn.getresponse()
            data = resp.read()
            if resp.status != 200:
                logging.warning(f"HTTP {resp.status} {resp.reason}")
                raise RuntimeError(f"http_status_{resp.status}")
            out = json.loads(data.decode("utf-8"))
            content = out.get("message", {}).get("content", "")
            if not content:
                raise RuntimeError("empty_content")
            # poskusi prebrati JSON ali seznam
            snippet = extract_json_from_text(content) or content.strip()
            try:
                return json.loads(snippet)
            except json.JSONDecodeError:
                # včasih vrne že python-liken seznam brez narekovajev; ni kaj
                raise
        except Exception as e:
            if attempt < MAX_RETRIES:
                time.sleep(0.6 * (attempt + 1))
                continue
            logging.debug(f"model response raw: {e}")
            return None
        finally:
            conn.close()


def normalize_list(xs: Any) -> List[str]:
    out: List[str] = []
    if isinstance(xs, list):
        for x in xs:
            if isinstance(x, str):
                s = " ".join(x.split()).strip()
                s = re.sub(r"\s*[:;]\s*$", "", s)
                if s:
                    out.append(s)
    return dedupe(out)


def dedupe(items: List[str]) -> List[str]:
    seen = set()
    res = []
    for s in items:
        key = re.sub(r"\W+", " ", s).strip().lower()
        if key not in seen:
            seen.add(key)
            res.append(s)
    return res


def normalize_model_response(resp: Any) -> Dict[str, List[str]]:
    """
    Sprejme različne oblike in vrne {"senses": [...], "senses_en": [...]}
    Podprte oblike:
      - {"senses": [...], "senses_en": [...]}
      - [{"slovenian": "...", "english": "..."}, ...]
      - ["...", "..."]
    """
    senses: List[str] = []
    senses_en: List[str] = []

    if isinstance(resp, dict):
        if "senses" in resp:
            raw = resp.get("senses")
            if isinstance(raw, list) and raw and isinstance(raw[0], dict):
                # seznam slovarjev
                for item in raw:
                    if isinstance(item, dict):
                        if "slovenian" in item:
                            senses.append(str(item.get("slovenian", "")).strip())
                        elif "si" in item:
                            senses.append(str(item.get("si", "")).strip())
                # prevodi iz senses_en ali iz itemov
                if isinstance(resp.get("senses_en"), list):
                    senses_en = [
                        str(x).strip() for x in resp["senses_en"] if isinstance(x, str)
                    ]
                else:
                    for item in raw:
                        if isinstance(item, dict):
                            if "english" in item:
                                senses_en.append(str(item.get("english", "")).strip())
                            elif "en" in item:
                                senses_en.append(str(item.get("en", "")).strip())
            else:
                senses = normalize_list(raw)
                senses_en = normalize_list(resp.get("senses_en"))
        else:
            # neznan objekt, poskusi prebrati standardna polja
            senses = normalize_list(resp.get("slovenian") or resp.get("si") or [])
            senses_en = normalize_list(resp.get("english") or resp.get("en") or [])

    elif isinstance(resp, list):
        # lahko je seznam stringov ali seznam slovarjev
        if resp and isinstance(resp[0], dict):
            for item in resp:
                si = item.get("slovenian") or item.get("si")
                en = item.get("english") or item.get("en")
                if isinstance(si, str):
                    senses.append(si.strip())
                if isinstance(en, str):
                    senses_en.append(en.strip())
        else:
            senses = normalize_list(resp)

    # normalizacija in ujemanje dolžin
    senses = normalize_list(senses)
    senses_en = normalize_list(senses_en)
    if senses_en and len(senses_en) != len(senses):
        senses_en = []
    return {"senses": senses, "senses_en": senses_en}


def build_user_prompt(word: str, title: str, senses: List[str]) -> str:
    data = {"word": word, "title": title, "senses": senses}
    return json.dumps(data, ensure_ascii=False)


def clean_entry(entry: Dict[str, Any]) -> Dict[str, Any]:
    word = entry.get("word") or ""
    title = entry.get("title") or ""
    senses_in = normalize_list(entry.get("senses") or [])

    if not senses_in:
        entry["senses_en"] = []
        entry["_clean_status"] = "skipped_empty_input"
        return entry

    messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": build_user_prompt(word, title, senses_in)},
    ]

    resp = call_ollama_chat(messages)

    if resp is None:
        entry["_clean_status"] = "model_error_no_json"
        entry["senses"] = senses_in
        entry["senses_en"] = []
        entry["_clean_error_detail"] = "no_or_invalid_json_response"
        return entry

    try:
        norm = normalize_model_response(resp)
    except Exception as e:
        entry["_clean_status"] = "normalize_error"
        entry["senses"] = senses_in
        entry["senses_en"] = []
        entry["_clean_error_detail"] = "normalize_exception"
        return entry

    cleaned = norm.get("senses", [])
    senses_en = norm.get("senses_en", [])

    if not cleaned:
        entry["_clean_status"] = "model_empty_senses"
        entry["senses"] = senses_in
        entry["senses_en"] = []
        return entry

    entry["senses"] = cleaned
    entry["senses_en"] = senses_en
    entry["_clean_status"] = "ok"
    return entry


def write_jsonl(path: str, obj: Dict[str, Any]) -> None:
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(obj, ensure_ascii=False) + "\n")


# -------------- parallel runner --------------


def process_line(
    idx: int, line: str
) -> Tuple[int, Optional[Dict[str, Any]], Optional[Dict[str, Any]], Optional[str]]:
    line = line.strip()
    if not line:
        return idx, None, None, None
    try:
        row = json.loads(line)
    except json.JSONDecodeError:
        err = {"_clean_status": "parse_error_input_line", "raw_line": line}
        return idx, None, err, "parse_error"

    start = time.time()
    res = clean_entry(dict(row))
    elapsed = time.time() - start

    status = res.get("_clean_status")
    if status == "ok":
        out = dict(res)
        out.pop("_clean_status", None)
        out.pop("_clean_error_detail", None)
        logging.info(
            f"[{idx + 1}] ✅ {row.get('word', '')} ({elapsed:.1f}s, {len(out.get('senses', []))} pomenov)"
        )
        time.sleep(REQUEST_DELAY_SEC)
        return idx, out, None, None
    else:
        # vrni fallback v glavni izhod in zapiši napako ločeno
        fallback = dict(row)
        fallback["senses_en"] = res.get("senses_en", [])
        logging.warning(
            f"[{idx + 1}] ⚠️ {row.get('word', '')} status={status} ({elapsed:.1f}s) – fallback"
        )
        time.sleep(REQUEST_DELAY_SEC)
        return idx, fallback, res, status


def main():
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(message)s",
        datefmt="%H:%M:%S",
    )

    logging.info(f"Začetek čiščenja. Vhod: {INPUT_PATH}")
    logging.info(f"Obstoječi izhod: {OUTPUT_PATH}")

    # preberi že uspešno očiščene besede iz obstoječega izhoda
    existing_words = set()
    try:
        with open(OUTPUT_PATH, "r", encoding="utf-8") as f_exist:
            for line in f_exist:
                line = line.strip()
                if not line:
                    continue
                try:
                    row = json.loads(line)
                    w = str(row.get("word") or "").strip().lower()
                    if w:
                        existing_words.add(w)
                except json.JSONDecodeError:
                    continue
    except FileNotFoundError:
        pass  # prvi zagon, izhod še ne obstaja

    # počisti errors izhod na nov zagon
    open(ERRORS_PATH, "w", encoding="utf-8").close()

    # preberi vhod in pripravi samo manjkajoče
    with open(INPUT_PATH, "r", encoding="utf-8") as f_in:
        raw_lines = [l for l in f_in if l.strip()]

    total_in = len(raw_lines)

    # filtriraj: vzemi vrstice, katerih 'word' še NI v existing_words
    lines_to_process = []
    seen_this_run = set()  # da ne podvojimo v tem zagonu, če se word ponovi v vhodu
    for i, line in enumerate(raw_lines):
        try:
            obj = json.loads(line)
            word = str(obj.get("word") or "").strip().lower()
        except json.JSONDecodeError:
            # če ni JSON, ga vseeno predamo v obdelavo, da konča v errors
            lines_to_process.append((i, line))
            continue

        if not word:
            lines_to_process.append((i, line))
            continue

        if word in existing_words or word in seen_this_run:
            continue

        seen_this_run.add(word)
        lines_to_process.append((i, line))

    total_existing = len(existing_words)
    total_todo = len(lines_to_process)

    logging.info(f"Skupaj vrstic v vhodu: {total_in}")
    logging.info(f"Že obdelane besede v izhodu: {total_existing}")
    logging.info(f"Za obdelavo zdaj: {total_todo}")
    logging.info(f"Vzporedne niti: {MAX_WORKERS}")

    count_in = 0
    count_ok = 0
    count_err = 0

    # odpremo izhodne datoteke za sprotni zapis, ne brišemo glavnega izhoda
    with (
        open(OUTPUT_PATH, "a", encoding="utf-8") as out_f,
        open(ERRORS_PATH, "a", encoding="utf-8") as err_f,
    ):
        # pošljemo samo manjkajoče
        with ThreadPoolExecutor(max_workers=MAX_WORKERS) as ex:
            futures = {
                ex.submit(process_line, idx, line): idx
                for idx, line in lines_to_process
            }
            for fut in as_completed(futures):
                try:
                    idx, ok_row, err_row, status = fut.result()
                except Exception:
                    logging.exception("Nepričakovana napaka pri obdelavi future")
                    continue

                # nič za zapis
                if ok_row is None and err_row is None:
                    continue

                count_in += 1

                if ok_row:
                    out_f.write(json.dumps(ok_row, ensure_ascii=False) + "\n")
                    out_f.flush()
                    count_ok += 1
                    # dodaj obdelano besedo v existing_words, da se izogne morebitnim kasnejšim duplikatom
                    w = str(ok_row.get("word") or "").strip().lower()
                    if w:
                        existing_words.add(w)

                if err_row:
                    err_f.write(json.dumps(err_row, ensure_ascii=False) + "\n")
                    err_f.flush()
                    count_err += 1

    logging.info(
        f"Končano. Novi poskusi: {count_in}. Uspešno očiščeno: {count_ok}. Napake ali fallback: {count_err}."
    )
    logging.info(f"Izhod (append): {OUTPUT_PATH}")
    logging.info(f"Napake: {ERRORS_PATH}")


if __name__ == "__main__":
    main()
