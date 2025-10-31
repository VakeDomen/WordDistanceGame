# clean_slovenian_words.py

# Input and output file names
input_file = "wordlist.txt"
output_file = "cleaned_wordlist.txt"

# Open input and output
with open(input_file, "r", encoding="utf-8") as infile, open(output_file, "w", encoding="utf-8") as outfile:
    for line in infile:
        word = line.strip()
        # skip empty lines
        if not word:
            continue
        # remove words shorter than 3 characters
        if len(word) < 3:
            continue
        # remove words that start with a capital letter (likely names)
        if word[0].isupper():
            continue
        outfile.write(word + "\n")

print("Cleaning complete. Saved to", output_file)
