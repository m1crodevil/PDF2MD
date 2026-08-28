#!/usr/bin/env python3
"""Generate a deterministic XeLaTeX source report from a runtime manifest."""
from pathlib import Path
import json
import sys


def tex(value: object) -> str:
    return str(value).replace("\\", "\\textbackslash{}").replace("&", "\\&").replace("%", "\\%").replace("_", "\\_")


def main() -> int:
    if len(sys.argv) not in (3, 4):
        print(f"usage: {sys.argv[0]} <manifest.json> <report.tex> [--pdf]", file=sys.stderr)
        return 2
    manifest_path, output = map(Path, sys.argv[1:3])
    data = json.loads(manifest_path.read_text(encoding="utf-8"))
    required = {"schema_version", "mode", "input", "output_dir", "pages_total", "ok", "skipped", "failed", "content_integrity"}
    missing = sorted(required - data.keys())
    if missing:
        raise SystemExit(f"missing manifest fields: {', '.join(missing)}")
    rows = "\n".join(f"\\textbf{{{tex(k)}}} & {tex(v)} \\\\\\" for k, v in data.items())
    source = """\\documentclass[11pt]{article}
\\usepackage{fontspec}
\\usepackage[margin=1in]{geometry}
\\begin{document}
\\section*{PDF2MD Run Report}
This report is generated from the runtime manifest; it is not a quality certificate.
\\begin{tabular}{ll}
%s
\\end{tabular}
\\end{document}
""" % rows
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(source, encoding="utf-8")
    if len(sys.argv) == 4:
        import shutil, subprocess
        if not shutil.which("xelatex"):
            raise SystemExit("xelatex not found; generated .tex only")
        subprocess.run(["xelatex", "-interaction=nonstopmode", "-halt-on-error", output.name], cwd=output.parent, check=True, stdout=subprocess.DEVNULL)
    print(f"report_tex: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
