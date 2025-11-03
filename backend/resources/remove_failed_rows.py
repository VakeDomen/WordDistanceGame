#!/usr/bin/env python3
import json
import logging
from typing import Set

INPUT_OK = "cleaned_senses_output.jsonl"
INPUT_ERR = "errors_cleaned_senses_output.jsonl"
OUTPUT_CLEANED = "cleaned_senses_output_no_errors.jsonl"


def load_failed_words(err_path: str) -> Set[str]:
    failed = set()
    with open(err_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
                word = str(row.get("word") or "").strip().lower()
                if word:
                    failed.add(word)
            except json.JSONDecodeError:
                continue
    return failed


def main():
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(message)s",
        datefmt="%H:%M:%S",
    )

    failed_words = load_failed_words(INPUT_ERR)
    logging.info(f"Najdenih neuspešnih zapisov: {len(failed_words)}")

    kept = 0
    removed = 0

    with (
        open(INPUT_OK, "r", encoding="utf-8") as f_in,
        open(OUTPUT_CLEANED, "w", encoding="utf-8") as f_out,
    ):
        for line in f_in:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            word = str(row.get("word") or "").strip().lower()
            if word and word not in failed_words:
                f_out.write(json.dumps(row, ensure_ascii=False) + "\n")
                kept += 1
            else:
                removed += 1

    logging.info(f"Ohranjeno {kept} zapisov, odstranjeno {removed}.")
    logging.info(f"Novi izhod: {OUTPUT_CLEANED}")


if __name__ == "__main__":
    main()
