#!/usr/bin/env python3
"""Generate frequency dictionary for symspell from hunspell .dic files.
Output format: word count (space-separated, one per line).
"""
import re, sys, os

def parse_hunspell_dic(dic_path, freq=1000):
    """Parse hunspell .dic → list of (word, freq)."""
    words = []
    with open(dic_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    # First line is count, skip
    for line in lines[1:]:
        line = line.strip()
        if not line:
            continue
        # Hunspell format: word/stem or word/affixes
        # Also may have: word/affix1,affix2
        word = re.split(r'[/\t,]', line)[0].strip()
        # Skip non-alphabetic, too short, or garbage
        if len(word) < 2:
            continue
        if not re.match(r'^[a-zA-Z]', word):
            continue
        words.append(word.lower())
    # Deduplicate, assign pseudo-frequencies (higher = more common)
    seen = {}
    for w in words:
        seen[w] = seen.get(w, 0) + 1
    # Assign descending freq based on order
    result = []
    for i, (w, cnt) in enumerate(sorted(seen.items(), key=lambda x: -len(x[0]))):
        # Longer words get slightly lower freq (common words are short)
        f = max(100, freq - i // 10)
        result.append(f"{w} {f}")
    return result

def main():
    dicts = [
        ("/usr/share/hunspell/id_ID.dic", "Indonesian"),
        ("/usr/share/hunspell/en_US.dic", "English"),
    ]
    out_path = sys.argv[1] if len(sys.argv) > 1 else "data/frequency_dict.txt"
    os.makedirs(os.path.dirname(out_path) if os.path.dirname(out_path) else ".", exist_ok=True)
    
    all_words = []
    for dic_path, lang in dicts:
        if os.path.exists(dic_path):
            words = parse_hunspell_dic(dic_path)
            print(f"{lang}: {len(words)} words from {dic_path}", file=sys.stderr)
            all_words.extend(words)
    
    # Deduplicate again across languages
    seen = set()
    unique = []
    for line in all_words:
        word = line.split()[0]
        if word not in seen:
            seen.add(word)
            unique.append(line)
    
    with open(out_path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(unique))
    print(f"Total: {len(unique)} unique words → {out_path}", file=sys.stderr)

if __name__ == "__main__":
    main()
