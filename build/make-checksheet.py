#!/usr/bin/env python3
"""make-checksheet.py — build the Approve/Deny sheet from build/docs/TESTPLAN.md.

The sheet is generated rather than written. P22 spent a round finding `LZMA2:19` copied by
hand into two docstrings and a CORE section, all three drifting apart from the code that
produced them; the fix it chose was to make the test read the document instead of letting a
person copy out of it. A 153-row checklist transcribed by hand is the same hazard with more
rows, so this reads the plan and emits the page, and the two cannot disagree.

    build/make-checksheet.py                    # -> build/docs/checksheet.html
    build/make-checksheet.py -o somewhere.html

The output is one self-contained file: no external fonts, scripts, styles or images, because
a published artifact is served under a CSP that blocks every one of them. Answers live in
localStorage, and the foot of the page builds the text to paste back — a viewer sandbox makes
any download the page starts itself inert, `<a download>` included.
"""

import argparse
import html
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "build" / "docs" / "TESTPLAN.md"

ROUND_RE = re.compile(r"^##\s+Round\s+(\d+)\s+—\s+(.*)$")
STEP_RE = re.compile(r"^\|\s*(\d+)\.(\d+)\s")

# `\|` is a pipe inside a code span, not a column break. Swap it out before splitting the
# row and put it back after, or every command containing a shell pipe loses a column.
PIPE = "\x00PIPE\x00"


def inline(md: str) -> str:
    """The subset of markdown the plan's cells actually use, in source order."""
    out, i, n = [], 0, len(md)
    while i < n:
        ch = md[i]
        if ch == "`":
            j = md.find("`", i + 1)
            if j == -1:
                out.append(html.escape(ch))
                i += 1
                continue
            out.append("<code>" + html.escape(md[i + 1 : j]) + "</code>")
            i = j + 1
        elif md.startswith("**", i):
            j = md.find("**", i + 2)
            if j == -1:
                out.append(html.escape("**"))
                i += 2
                continue
            out.append("<strong>" + inline(md[i + 2 : j]) + "</strong>")
            i = j + 2
        elif ch == "*":
            j = md.find("*", i + 1)
            if j == -1:
                out.append(html.escape(ch))
                i += 1
                continue
            out.append("<em>" + inline(md[i + 1 : j]) + "</em>")
            i = j + 1
        else:
            j = i
            while j < n and md[j] not in "`*":
                j += 1
            out.append(html.escape(md[i:j]))
            i = j
    return "".join(out).replace(PIPE, "|")


def parse(path: pathlib.Path):
    rounds, current = [], None
    for raw in path.read_text(encoding="utf-8").splitlines():
        m = ROUND_RE.match(raw)
        if m:
            current = {"n": int(m.group(1)), "title": m.group(2).strip(), "steps": []}
            rounds.append(current)
            continue
        if current is None or not STEP_RE.match(raw):
            continue

        cells = [c.strip() for c in raw.replace(r"\|", PIPE).strip().strip("|").split("|")]
        if len(cells) != 4:
            sys.exit(f"make-checksheet.py: row has {len(cells)} cells, wanted 4:\n  {raw}")

        ident, do, must, holds = cells
        current["steps"].append(
            {
                "id": ident.replace("†", "").replace("‡", "").strip(),
                # Recovered from the P11/P12 round, and inherited-unticked from P22. Both
                # are carried through to the sheet because a step that once failed is the
                # one most worth watching, and the walker should know which those are.
                "recovered": "†" in raw,
                "inherited": "‡" in raw,
                "do": inline(do),
                "must": inline(must),
                "holds": inline(holds),
            }
        )
    if not rounds:
        sys.exit(f"make-checksheet.py: found no rounds in {path}")
    return rounds


# ---------------------------------------------------------------------------
# The page. Palette note, since it is the one decision worth writing down: the accent is
# indium's own indigo — the 451 nm emission line the element was named after — rather than
# a hue picked to look technical.

CSS = """
*, *::before, *::after { box-sizing: border-box; }

:root {
  --ground:   #edf0f4;
  --surface:  #fbfcfd;
  --raised:   #ffffff;
  --line:     #d3d9e2;
  --line-soft:#e3e8ef;
  --ink:      #131822;
  --muted:    #5c6675;
  --faint:    #838d9c;
  --accent:   #3a31c9;
  --accent-w: #ecebfb;
  --ok:       #0e7a4f;
  --ok-w:     #e2f3ea;
  --no:       #be2e1c;
  --no-w:     #fceae7;
  --shadow:   0 1px 2px rgba(19, 24, 34, .06), 0 6px 16px -10px rgba(19, 24, 34, .22);

  --mono: ui-monospace, "JetBrains Mono", "IBM Plex Mono", "SF Mono", "Cascadia Mono",
          Menlo, Consolas, monospace;
  --sans: "Source Sans 3", "Segoe UI", Roboto, "Helvetica Neue", system-ui, sans-serif;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --ground:   #0e1116;
    --surface:  #161b23;
    --raised:   #1c222c;
    --line:     #2a323f;
    --line-soft:#222933;
    --ink:      #e6eaf0;
    --muted:    #9aa5b4;
    --faint:    #737f8e;
    --accent:   #8b84ff;
    --accent-w: #201f3d;
    --ok:       #45c98a;
    --ok-w:     #123024;
    --no:       #ff7e6b;
    --no-w:     #351b18;
    --shadow:   0 1px 2px rgba(0, 0, 0, .4), 0 6px 16px -10px rgba(0, 0, 0, .7);
  }
}

:root[data-theme="dark"] {
  --ground:   #0e1116;
  --surface:  #161b23;
  --raised:   #1c222c;
  --line:     #2a323f;
  --line-soft:#222933;
  --ink:      #e6eaf0;
  --muted:    #9aa5b4;
  --faint:    #737f8e;
  --accent:   #8b84ff;
  --accent-w: #201f3d;
  --ok:       #45c98a;
  --ok-w:     #123024;
  --no:       #ff7e6b;
  --no-w:     #351b18;
  --shadow:   0 1px 2px rgba(0, 0, 0, .4), 0 6px 16px -10px rgba(0, 0, 0, .7);
}

body {
  margin: 0;
  background: var(--ground);
  color: var(--ink);
  font-family: var(--sans);
  font-size: 15px;
  line-height: 1.55;
  -webkit-font-smoothing: antialiased;
}

code {
  font-family: var(--mono);
  font-size: .88em;
  background: var(--line-soft);
  padding: .1em .34em;
  border-radius: 3px;
  overflow-wrap: anywhere;
}

.wrap { max-width: 1360px; margin: 0 auto; padding: 0 20px 96px; }

/* ---- masthead ---- */

.masthead { padding: 40px 0 22px; border-bottom: 1px solid var(--line); }
.eyebrow {
  font-family: var(--mono);
  font-size: 11px;
  letter-spacing: .16em;
  text-transform: uppercase;
  color: var(--accent);
  margin: 0 0 10px;
}
.masthead h1 {
  margin: 0;
  font-size: 30px;
  line-height: 1.15;
  font-weight: 620;
  letter-spacing: -.015em;
  text-wrap: balance;
}
.standfirst {
  margin: 12px 0 0;
  max-width: 66ch;
  color: var(--muted);
  font-size: 15.5px;
}
.facts {
  margin: 20px 0 0;
  display: flex;
  flex-wrap: wrap;
  gap: 8px 26px;
  font-family: var(--mono);
  font-size: 12px;
  color: var(--faint);
}
.facts b { color: var(--ink); font-weight: 550; }

/* ---- the running verdict, pinned ---- */

.bar {
  position: sticky;
  top: 0;
  z-index: 30;
  background: color-mix(in srgb, var(--ground) 88%, transparent);
  backdrop-filter: blur(10px);
  border-bottom: 1px solid var(--line);
  margin-bottom: 26px;
}
.bar-in {
  max-width: 1360px;
  margin: 0 auto;
  padding: 11px 20px;
  display: flex;
  align-items: center;
  gap: 18px;
  flex-wrap: wrap;
}
.tally { display: flex; gap: 14px; font-family: var(--mono); font-size: 12.5px; }
.tally span { display: inline-flex; align-items: baseline; gap: 5px; }
.tally b { font-size: 15px; font-weight: 600; font-variant-numeric: tabular-nums; }
.t-ok b { color: var(--ok); }
.t-no b { color: var(--no); }
.t-left b { color: var(--muted); }

.meter {
  flex: 1 1 160px;
  min-width: 120px;
  height: 6px;
  border-radius: 99px;
  background: var(--line);
  overflow: hidden;
  display: flex;
}
.meter i { display: block; height: 100%; transition: width .18s ease; }
.meter .m-ok { background: var(--ok); }
.meter .m-no { background: var(--no); }

.controls { display: flex; gap: 6px; margin-left: auto; flex-wrap: wrap; }

button {
  font: inherit;
  font-family: var(--mono);
  font-size: 12px;
  color: var(--ink);
  background: var(--raised);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 5px 11px;
  cursor: pointer;
}
button:hover { border-color: var(--accent); color: var(--accent); }
button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
button[aria-pressed="true"] {
  background: var(--accent-w);
  border-color: var(--accent);
  color: var(--accent);
}

/* ---- rounds ---- */

.round { margin: 0 0 34px; }
.round-head {
  position: sticky;
  top: 47px;
  z-index: 20;
  display: flex;
  align-items: baseline;
  gap: 12px;
  padding: 9px 14px;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 8px 8px 0 0;
}
.round-n {
  font-family: var(--mono);
  font-size: 11px;
  letter-spacing: .1em;
  text-transform: uppercase;
  color: var(--accent);
}
.round-head h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  letter-spacing: -.01em;
}
.round-tally {
  margin-left: auto;
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--faint);
  font-variant-numeric: tabular-nums;
}
.round-note {
  padding: 10px 14px;
  border-inline: 1px solid var(--line);
  border-bottom: 1px solid var(--line-soft);
  background: var(--surface);
  color: var(--muted);
  font-size: 13.5px;
}

.steps { border: 1px solid var(--line); border-top: 0; border-radius: 0 0 8px 8px; overflow: hidden; }

.step {
  display: grid;
  grid-template-columns: 62px minmax(0, 1fr) minmax(0, 1.15fr) 78px 152px;
  gap: 0 16px;
  padding: 12px 14px;
  background: var(--raised);
  border-top: 1px solid var(--line-soft);
  align-items: start;
}
.step:first-child { border-top: 0; }
.step[data-verdict="approve"] { background: var(--ok-w); }
.step[data-verdict="deny"]    { background: var(--no-w); }
.hidden { display: none !important; }

.sid {
  font-family: var(--mono);
  font-size: 12px;
  font-weight: 600;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
  padding-top: 1px;
}
.marks { display: block; margin-top: 3px; font-size: 10px; color: var(--accent); letter-spacing: .05em; }

.do, .must { font-size: 13.7px; min-width: 0; overflow-wrap: anywhere; }
.must { color: var(--muted); }
.step[data-verdict] .must { color: var(--ink); }

.holds {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--faint);
  padding-top: 2px;
  overflow-wrap: anywhere;
}

.verdict { display: flex; flex-direction: column; gap: 6px; }
.verdict-row { display: flex; gap: 6px; }
.v {
  flex: 1;
  padding: 5px 0;
  font-size: 11.5px;
  letter-spacing: .04em;
  text-transform: uppercase;
}
.v-ok[aria-pressed="true"] { background: var(--ok); border-color: var(--ok); color: #fff; }
.v-no[aria-pressed="true"] { background: var(--no); border-color: var(--no); color: #fff; }
:root[data-theme="dark"] .v-ok[aria-pressed="true"],
:root[data-theme="dark"] .v-no[aria-pressed="true"] { color: #0e1116; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) .v-ok[aria-pressed="true"],
  :root:not([data-theme="light"]) .v-no[aria-pressed="true"] { color: #0e1116; }
}

/* A row earns its note by having something to say. 153 rows each carrying an empty box is
   a page nobody can scan; the box appears once the step is answered, or once the keyboard
   reaches it, and the row grows to match. */
.note { display: none; }
.step[data-verdict] .note,
.step[data-noted] .note,
.step:focus-within .note { display: block; }

.note {
  width: 100%;
  font: inherit;
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--ink);
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 5px;
  padding: 5px 7px;
  resize: vertical;
  min-height: 30px;
}
.note:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
.note::placeholder { color: var(--faint); }

/* ---- the paste-back ---- */

.copyout { margin-top: 40px; }
.copyout h2 { font-size: 19px; margin: 0 0 6px; letter-spacing: -.01em; }
.copyout p { margin: 0 0 14px; color: var(--muted); max-width: 66ch; font-size: 14px; }
.out {
  width: 100%;
  min-height: 240px;
  font-family: var(--mono);
  font-size: 12px;
  line-height: 1.5;
  color: var(--ink);
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 14px;
  resize: vertical;
  white-space: pre;
  overflow-wrap: normal;
  overflow-x: auto;
}
.foot {
  margin-top: 30px;
  padding-top: 16px;
  border-top: 1px solid var(--line);
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--faint);
}

@media (max-width: 1000px) {
  .step { grid-template-columns: 54px minmax(0, 1fr) 150px; }
  .must { grid-column: 2 / 4; padding-top: 4px; }
  .holds { grid-column: 1 / 2; grid-row: 2; font-size: 10px; }
  .round-head { top: 0; }
}

@media (prefers-reduced-motion: reduce) {
  * { transition: none !important; animation: none !important; }
}
"""


JS = """
const KEY = 'indium-pxx-checksheet-v1';
let state = {};
try { state = JSON.parse(localStorage.getItem(KEY) || '{}'); } catch (e) { state = {}; }

const save = () => { try { localStorage.setItem(KEY, JSON.stringify(state)); } catch (e) {} };
const flat = STEPS.flatMap(r => r.steps.map(s => ({ ...s, round: r.n, rtitle: r.title })));
const get  = id => state[id] || {};

let filter = 'all';

function paint() {
  let ok = 0, no = 0;
  for (const s of flat) {
    const v = get(s.id).verdict;
    if (v === 'approve') ok++; else if (v === 'deny') no++;
  }
  const total = flat.length, left = total - ok - no;

  document.getElementById('n-ok').textContent   = ok;
  document.getElementById('n-no').textContent   = no;
  document.getElementById('n-left').textContent = left;
  document.getElementById('m-ok').style.width = (ok / total * 100) + '%';
  document.getElementById('m-no').style.width = (no / total * 100) + '%';

  for (const r of STEPS) {
    let rok = 0, rno = 0;
    for (const s of r.steps) {
      const v = get(s.id).verdict;
      if (v === 'approve') rok++; else if (v === 'deny') rno++;
    }
    const el = document.getElementById('rt-' + r.n);
    el.textContent = rno > 0
      ? (rok + rno) + '/' + r.steps.length + ' · ' + rno + ' denied'
      : (rok + rno) + '/' + r.steps.length;
    el.style.color = rno > 0 ? 'var(--no)' : '';
  }

  for (const s of flat) {
    const row = document.getElementById('s-' + s.id);
    const v = get(s.id).verdict;
    if (v) row.dataset.verdict = v; else delete row.dataset.verdict;
    // Keep a note visible even when the step it belongs to has no verdict yet, or it
    // would vanish on blur and go unreported at the foot.
    if ((get(s.id).note || '').trim()) row.dataset.noted = '1'; else delete row.dataset.noted;
    row.querySelector('.v-ok').setAttribute('aria-pressed', v === 'approve');
    row.querySelector('.v-no').setAttribute('aria-pressed', v === 'deny');

    const show = filter === 'all'
      || (filter === 'left'   && !v)
      || (filter === 'denied' && v === 'deny');
    row.classList.toggle('hidden', !show);
  }

  // A round with nothing left to show goes too. Otherwise filtering to the denials leaves
  // a column of empty round headers, which reads as fourteen rounds rather than as three.
  for (const r of STEPS) {
    const sec = document.getElementById('r-' + r.n);
    sec.classList.toggle('hidden', !r.steps.some(s => {
      const v = get(s.id).verdict;
      return filter === 'all'
        || (filter === 'left'   && !v)
        || (filter === 'denied' && v === 'deny');
    }));
  }

  buildOut(ok, no, left);
}

function buildOut(ok, no, left) {
  const L = [];
  L.push('# INDIUM — PXX certification walk');
  L.push('');
  L.push('Build under test: v2.1 (indium 2.1.0-1), installed from its own package.');
  L.push('Instrument: build/docs/TESTPLAN.md — ' + flat.length + ' steps across ' + STEPS.length + ' rounds.');
  L.push('Answered ' + (ok + no) + ' of ' + flat.length + ' · approved ' + ok + ' · denied ' + no + ' · left ' + left);
  L.push('');

  const denied = flat.filter(s => get(s.id).verdict === 'deny');
  L.push('## Denials — ' + denied.length);
  L.push('');
  if (!denied.length) {
    L.push('None.');
  } else {
    for (const s of denied) {
      L.push('- [' + s.id + '] ' + s.plain);
      const n = (get(s.id).note || '').trim();
      if (n) for (const line of n.split('\\n')) L.push('      ' + line);
    }
  }
  L.push('');

  const noted = flat.filter(s => get(s.id).verdict === 'approve' && (get(s.id).note || '').trim());
  if (noted.length) {
    L.push('## Approved, with a note — ' + noted.length);
    L.push('');
    for (const s of noted) {
      L.push('- [' + s.id + '] ' + s.plain);
      for (const line of get(s.id).note.trim().split('\\n')) L.push('      ' + line);
    }
    L.push('');
  }

  const unrun = flat.filter(s => !get(s.id).verdict);
  if (unrun.length) {
    L.push('## Not answered — ' + unrun.length);
    L.push('');
    for (const s of unrun) {
      L.push('- [' + s.id + '] ' + s.plain);
      const n = (get(s.id).note || '').trim();
      if (n) for (const line of n.split('\\n')) L.push('      ' + line);
    }
    L.push('');
  }

  document.getElementById('out').value = L.join('\\n');
}

function setVerdict(id, v) {
  const cur = get(id);
  state[id] = { ...cur, verdict: cur.verdict === v ? null : v };
  save();
  paint();
}

document.addEventListener('click', e => {
  const b = e.target.closest('button[data-v]');
  if (b) { setVerdict(b.dataset.id, b.dataset.v); return; }

  const f = e.target.closest('button[data-filter]');
  if (f) {
    filter = f.dataset.filter;
    document.querySelectorAll('button[data-filter]')
      .forEach(x => x.setAttribute('aria-pressed', x.dataset.filter === filter));
    paint();
    return;
  }

  if (e.target.id === 'jump') {
    const next = flat.find(s => !get(s.id).verdict);
    if (next) {
      const row = document.getElementById('s-' + next.id);
      row.classList.remove('hidden');
      row.scrollIntoView({ block: 'center', behavior: 'smooth' });
      row.querySelector('.v-ok').focus();
    }
    return;
  }

  if (e.target.id === 'copy') {
    const out = document.getElementById('out');
    out.select();
    navigator.clipboard.writeText(out.value)
      .then(() => { e.target.textContent = 'Copied'; setTimeout(() => e.target.textContent = 'Copy', 1400); })
      .catch(() => { e.target.textContent = 'Select all, then Ctrl+C'; });
    return;
  }

  if (e.target.id === 'reset') {
    if (confirm('Clear every answer and note on this sheet? This cannot be undone.')) {
      state = {}; save(); paint();
    }
  }
});

// Typing repaints only the row being typed in and the block at the foot. A full paint on
// every keystroke would walk all 153 rows and be felt as lag in the box under the cursor.
document.addEventListener('input', e => {
  if (!e.target.classList.contains('note')) return;
  const id = e.target.dataset.id;
  state[id] = { ...get(id), note: e.target.value };
  save();

  const row = document.getElementById('s-' + id);
  if (e.target.value.trim()) row.dataset.noted = '1'; else delete row.dataset.noted;

  buildOut(
    flat.filter(s => get(s.id).verdict === 'approve').length,
    flat.filter(s => get(s.id).verdict === 'deny').length,
    flat.filter(s => !get(s.id).verdict).length
  );
});

for (const s of flat) {
  const n = get(s.id).note;
  if (n) document.querySelector('.note[data-id="' + s.id + '"]').value = n;
}
paint();
"""


def render(rounds) -> str:
    total = sum(len(r["steps"]) for r in rounds)

    # The plain text the paste-back quotes, so a denial reads as a sentence rather than as
    # an id. Tags stripped, because the block is pasted into a terminal.
    def plain(s):
        return re.sub(r"<[^>]+>", "", s["do"]).replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", '"').replace("&#x27;", "'")

    data = [
        {
            "n": r["n"],
            "title": r["title"],
            "steps": [{"id": s["id"], "plain": plain(s)} for s in r["steps"]],
        }
        for r in rounds
    ]

    body = []
    for r in rounds:
        body.append(f'<section class="round" id="r-{r["n"]}">')
        body.append(
            f'<div class="round-head"><span class="round-n">Round {r["n"]}</span>'
            f"<h2>{html.escape(r['title'])}</h2>"
            f'<span class="round-tally" id="rt-{r["n"]}">0/{len(r["steps"])}</span></div>'
        )
        body.append('<div class="steps">')
        for s in r["steps"]:
            marks = []
            if s["recovered"]:
                marks.append("†")
            if s["inherited"]:
                marks.append("‡")
            mark = f'<span class="marks">{"".join(marks)}</span>' if marks else ""
            body.append(
                f'<div class="step" id="s-{s["id"]}">'
                f'<div class="sid">{s["id"]}{mark}</div>'
                f'<div class="do">{s["do"]}</div>'
                f'<div class="must">{s["must"]}</div>'
                f'<div class="holds">{s["holds"]}</div>'
                f'<div class="verdict"><div class="verdict-row">'
                f'<button class="v v-ok" data-v="approve" data-id="{s["id"]}" aria-pressed="false">Approve</button>'
                f'<button class="v v-no" data-v="deny" data-id="{s["id"]}" aria-pressed="false">Deny</button>'
                f"</div>"
                f'<textarea class="note" data-id="{s["id"]}" rows="1" '
                f'placeholder="what happened" aria-label="Note for step {s["id"]}"></textarea>'
                f"</div></div>"
            )
        body.append("</div></section>")

    return f"""<title>INDIUM Certification Sheet</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{CSS}</style>

<div class="bar"><div class="bar-in">
  <div class="tally">
    <span class="t-ok">Approved <b id="n-ok">0</b></span>
    <span class="t-no">Denied <b id="n-no">0</b></span>
    <span class="t-left">Left <b id="n-left">{total}</b></span>
  </div>
  <div class="meter"><i class="m-ok" id="m-ok" style="width:0"></i><i class="m-no" id="m-no" style="width:0"></i></div>
  <div class="controls">
    <button data-filter="all" aria-pressed="true">All</button>
    <button data-filter="left" aria-pressed="false">Unanswered</button>
    <button data-filter="denied" aria-pressed="false">Denied</button>
    <button id="jump">Next unanswered</button>
  </div>
</div></div>

<div class="wrap">
  <header class="masthead">
    <p class="eyebrow">PXX · the round that ends the beta</p>
    <h1>The certification walk</h1>
    <p class="standfirst">
      CORE §7 says the <code>1.0</code> line stays a beta until the design work it is named for
      has been in real hands, and that the gate is a testing round against a released build
      carrying it. No such round has ever been run. This is the sheet for running it.
    </p>
    <div class="facts">
      <span>Build under test <b>v2.1 · indium 2.1.0-1</b></span>
      <span>Steps <b>{total}</b></span>
      <span>Rounds <b>{len(rounds)}</b></span>
      <span>Instrument <b>build/docs/TESTPLAN.md</b></span>
    </div>
    <p class="standfirst" style="margin-top:14px;font-size:14px">
      Install the released package first — never the working tree, which can carry a fix no user
      will ever have. A step passes only if what must happen happens <em>as written</em>; a step
      you cannot run is not a pass, so deny it and say why in the note. Answers are kept in this
      browser, so a closed tab loses nothing. Every denial becomes work in this round.
    </p>
  </header>

  {"".join(body)}

  <section class="copyout">
    <h2>Paste this back</h2>
    <p>
      Built live from your answers, denials first. Copy it into the conversation when you are
      done — or partway, if something needs fixing before you carry on.
    </p>
    <div class="controls" style="margin:0 0 10px">
      <button id="copy">Copy</button>
      <button id="reset">Clear the sheet</button>
    </div>
    <textarea class="out" id="out" readonly aria-label="Results to paste back"></textarea>
  </section>

  <p class="foot">
    Generated from build/docs/TESTPLAN.md by build/make-checksheet.py — the sheet is not
    transcribed, so it cannot drift from the plan. † recovered from the P11/P12 round ·
    ‡ inherited unticked from P22.
  </p>
</div>

<script>const STEPS = {json.dumps(data, ensure_ascii=False)};{JS}</script>
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("-o", "--out", default=str(ROOT / "build" / "docs" / "checksheet.html"))
    ap.add_argument("--plan", default=str(PLAN))
    args = ap.parse_args()

    rounds = parse(pathlib.Path(args.plan))
    out = pathlib.Path(args.out)
    out.write_text(render(rounds), encoding="utf-8")

    total = sum(len(r["steps"]) for r in rounds)
    print(f"{out}  —  {total} steps across {len(rounds)} rounds, {out.stat().st_size:,} bytes")
    for r in rounds:
        print(f"  round {r['n']:>2}  {len(r['steps']):>3}  {r['title']}")


if __name__ == "__main__":
    main()
