# NOVA

*[Italiano](README.md) · **English***

**An expert sitting next to you, inside your PC.**

NOVA is not a chat that gives advice: it opens programs, fills in forms,
writes files, runs commands. And it does so **without taking your seat** — it
works in a window of its own, acting on the accessibility tree instead of the
mouse and keyboard, so you can keep working while it does its part.

[![ci](https://github.com/CastermustOfficial/NOVA/actions/workflows/ci.yml/badge.svg)](https://github.com/CastermustOfficial/NOVA/actions/workflows/ci.yml)
[![licence: MIT](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)

> **Status: alpha.** It works on the machine of the person building it. If you
> try it, expect rough edges — and open an issue, which is the most useful way
> to help.

> **A note on the language.** NOVA is written in Italian: the code, the
> comments, the system prompt. That is not an oversight — the prompt is source
> code, and translating it would mean maintaining N copies of a text that
> changes with every new function, then watching them drift apart. NOVA
> answers in your language: you pick it during installation, and the model is
> told which language to speak. This document is the English translation of
> [`README.md`](README.md).

## What it can do

A list of adjectives says nothing. These are the numbers, counted from the
code: **60 tools** for the model running on your PC, **31** for an agentic
brain working on its own, **38 file formats** it can open and show.

### It acts on the system, and doesn't take your seat

Files, applications, windows, PowerShell, clipboard, volume, notifications.
The difference that matters is not *what* it touches but **how**: NOVA acts on
the accessibility tree, not on mouse and keyboard. It can fill in a form in a
background window while you type in another one, and no window jumps to the
front to steal your focus.

That is a rule written into its prompt, not a side effect: *work behind, not
in front*.

### It uses the browser the way you would, but in blocks

NOVA drives Chrome by talking to it over CDP. It does not simulate keystrokes:
it pastes. Filling five fields of an online spreadsheet costs **one** call
instead of five, and reading a whole table costs one.

| Operation | Measured |
|---|---|
| `web_incolla` (paste) — five values into three fields | 35 ms |
| `web_tabella` (table) — a whole 5x4 table read at once | 33 ms |
| `web_cerca` (search) — searching without opening the browser | ~0.9 s |

The last row is the one that changes the assistant's character: **before
opening a page, NOVA searches**. A browser that opens is a window appearing on
your screen; a search that goes through a faceless browser is not.

### It remembers, and what it learns stays yours

A graph memory made of `.md` files — openable in Obsidian, versionable in git,
readable without NOVA. It learns durable facts after each exchange, and
**procedures**: how it solved a request, so it doesn't have to work it out
again next time. Procedures are found again even when the request is worded
differently or contains a typo, because the comparison goes through character
tri-grams rather than string equality.

On the disk of the person writing this, right now: 138 notes and 28 learned
procedures.

### It keeps credentials without showing them to the model

An archive encrypted with DPAPI. NOVA can fill in a login without the password
ever passing through the model: a reference goes into the prompt, the value
goes into the field. It is the only way in which "the assistant knows my
passwords" can be an acceptable sentence.

### It does by itself what it has to repeat

Automations it writes, learned procedures, scheduled tasks ("every day at 8"),
sentries that speak only when a value changes. And a **log of irreversible
actions**: what cannot be undone gets written down. The log never records the
value of a credential.

### It sees

It reads the screen when needed — but it tries to read the system first. A
screenshot is an accessory, not the normal way to know what is on a window:
the accessibility tree is more precise, faster, and does not depend on what is
visible.

---

## What can NOVA do? Some use cases

Every entry carries a marker, because "can do" is a phrase that stretches far
too easily:

- **works** — it works with the tools that exist today;
- **builds it** — NOVA puts it together on the spot, with a script or an
  automation that then sticks around;
- **missing** — it isn't there, and below you'll find what is missing. A list
  that names only what works is a list you don't trust the second time.

### Paperwork and deadlines

- **works** — Filling in a long online form using data from your *dossier*:
  refunds, enrolments, school forms, warranties, cancellations.
- **works** — Watching a deadline and warning you *before*: road tax,
  insurance, MOT, passport, domain renewal.
- **builds it** — Gathering the documents scattered around for one procedure
  into a single folder, renamed consistently.
- **missing** — Anything that goes through national digital identity (SPID,
  CIE, and their equivalents). This is not a technical limit to be worked
  around: strong authentication has to be done by the person, and rightly so.

### Household money

- **builds it** — Bank statements in PDF turning into a spreadsheet: "where
  did the money go this month".
- **works** — Watching a price and warning you **only when it drops**.
- **builds it** — Invoices and receipts: collected, renamed by date and
  supplier, added up.
- **works** — Comparing two offers — electricity, gas, phone — by reading the
  pages and putting them in a table.

### Documents and letters

- **works** — Writing a formal letter with real data: a cancellation, a
  complaint, a refund request, an appeal against a fine.
- **works** — Proofreading a document with the suggestions inside the text. On
  a `.docx` without losing the layout.
- **builds it** — Merging several PDFs, extracting pages, converting them.
- **missing** — Digital signature inside the harness.
- **missing** — Presentations: no tool produces `.pptx`.

### Spreadsheets and data

- **builds it** — Cleaning up a messy sheet: duplicates, columns out of place,
  dates written three different ways.
- **builds it** — From PDF to table, for price lists and statements.
- **missing** — From a **scanned** PDF to a table: without optical character
  recognition that PDF stays an image, and NOVA says so instead of inventing
  the numbers.
- **works** — Moving a table from one business system to another that has no
  API.

### The PC itself

- **works** — "Why is it slow?", by looking at the real state.
- **builds it** — Making room: the huge files, and the true duplicates — same
  content, not same name.
- **works** — Backing up a folder to an external drive, repeated weekly.
- **builds it** — Tidying up photos and downloads: by date, by type, by event.
- **missing** — "I think I have a virus". NOVA can look at processes, startup
  entries and connections, and say what it sees; it **is not an antivirus**
  and must not behave as if it were.

### Mail and people

- **works** — Triaging your inbox: what needs an answer, what can wait.
- **works** — Drafting the reply and sending it **only after confirmation**.
- **builds it** — The nudge: "if they haven't replied in five days, remind me".

### Studying

- **works** — Studying a stack of PDFs with citations you can check: file and
  page.
- **works** — Summarising a long document while showing where each piece comes
  from.
- **builds it** — Preparing revision questions from the material.

### People who struggle with computers

This one doesn't save half an hour: it changes who can use a computer.

- **works** — Using it **by voice**, calling it by name. "Nova, write to my
  son." "Nova, find me a bread recipe."
- **works** — Helping a parent remotely. The difference from a remote-control
  program is that NOVA **does not take the mouse**: it acts on the
  accessibility tree, so whoever is sitting at that computer keeps using it
  while NOVA does its part.

### Selling and buying

- **works** — Writing the listing and uploading the photos.
- **works** — Searching several marketplaces for a second-hand item and
  putting the results in a table.

---

## The same cases, from the inside

A list of tools doesn't tell you what happens when you line them up. This
does: every case below is a single request that turns into a chain, and under
each one the real chain is written out, with the names of the tools that do
the work.

The examples aren't imagined: the families come from the procedure archive of
a machine in daily use. Twenty-eight entries, and half of them are one thing
carried out from beginning to end.

### Looking for a job, and applying

This is the case that pushed more features than any other, because it is long
and boring exactly where an assistant earns its keep:

> «Look for AI engineer openings, see which ones make sense for me, and apply.»

NOVA searches the portals, opens the listings, reads your **dossier** — CV,
experience, texts you wrote yourself — and takes the facts from there. It
fills the form, including the dropdowns and the React fields that refuse to be
filled on their own, submits, and then checks your mail that the confirmation
arrived.

Two things have to be said, and they live in NOVA's prompt, not in its good
intentions: **what isn't in the dossier gets asked, not inferred** — an
invented job is not a mistake, it is a false statement with your signature on
it — and every submission is an action that cannot be undone, so it goes into
the log.

### Filling sheets and forms with a lot of data

> «Prepare a Google Sheet with these forty-three players, split by position.»

Actually done. The difference between NOVA and a macro is that it doesn't
press keys: it opens the sheet, finds the positions where they are written,
and **pastes in blocks** — five values into three fields in 35 milliseconds.
A lot of data doesn't go in one item at a time.

### Studying a stack of documents

> «Which of these six PDFs talks about entropy, and on what page?»

The harness opens the folder, searches every file at once and answers with
file and page, then scrolls onto it and highlights it. What you need is a
citation you can check, not a summary you have to trust.

### Writing and correcting a document

> «Read this report again and propose the corrections.»

The proposals appear **inside the text**, in colour. You correct them where
you read them and apply them when you decide. On a `.docx` it changes the
paragraph and leaves the layout untouched.

### Mail, and everyday things

Checking the mail, saving a contact, preparing a draft and sending it after
confirmation, opening a shared document, verifying that a site is up. These
are the requests that repeat, and that is where the procedure archive pays
off: the second time you don't start from scratch.

### Things that repeat by themselves

- **Scheduled tasks**: «every day at 8, check whether there are new openings».
- **Sentinels**: they speak only when a value **changes**, not on every round.
  A reminder that talks every day gets switched off after a week.
- **Automations it writes itself**: when a procedure repeats often enough,
  NOVA turns it into a tool and stops rebuilding it by hand.

### Moving data between two systems that don't talk to each other

> «Take the table from this management system and put it in the other one's
> sheet.»

This is the work that exists precisely because *there is no API*, and that
normally takes an hour by hand. `web_tabella` reads a whole table in a single
call, already as TSV; `web_incolla` puts it back on the other side in blocks;
`web_carica` hands a file to an upload field without opening any dialog. No
key pressed, no window jumping in front of you.

**The chain:** `web_apri` -> `web_tabella` -> `web_incolla` / `web_carica`

### Research with sources you can check

> «Give me the state of the art on open models, with the sources.»

`web_cerca` finds things without opening the browser, `web_prendi` downloads a
page as text in half a second instead of six, and the documents already on
your disk go into the harness. The difference from having a chat summarise
things for you is that the answer says **where**: file and page, not «I
believe that».

**The chain:** `web_cerca` -> `web_prendi` -> `harness_apri` ->
`harness_cerca_progetto` -> `harness_proponi` (the text is born inside the
document)

### Watching something and speaking only when it changes

> «Check every morning whether new openings appear and tell me only if there
> are any.»

A sentinel is not a reminder: it compares today's value with yesterday's and
stays quiet if they're the same. An alert that arrives every day gets switched
off after a week; one that arrives when something has changed gets read.

**The chain:** `pianifica_crea` (sentinel) -> ... -> `avvisi_recenti` when you
come back

### Signing into a service without the password passing through the model

> «Get into the portal and download this month's invoices.»

Credentials live in a store encrypted with DPAPI. A reference goes into the
prompt, the value goes into the field: the model never sees the password, and
the action log doesn't write it either. It is the only way «the assistant
knows my passwords» can be an acceptable sentence.

**The chain:** credential store -> `web_scrivi` -> `azione_registra`

### Asking a more capable model for a second opinion

> «This one is delicate: have someone better look at it.»

NOVA is not a single model. The one at home orchestrates — it's fast and costs
nothing — and when the task deserves it, it **delegates**: hard reasoning,
delicate code, a decision that weighs something. Whoever receives the task
can't see the conversation, so NOVA rewrites it for them in full.

**The chain:** `modelli` (who's available) -> `delega` -> the answer comes back
inside the same conversation

### Understanding why the PC is slow

> «Why is it slow?»

It reads the real state instead of guessing: memory, processes, disks, how
many layers of the model are actually in VRAM. On Windows, when VRAM runs out,
the driver silently falls back to shared RAM and the model runs ten times
slower without saying anything — NOVA sees it and says so.

**The chain:** `system_info` -> `list_processes` -> `run_powershell`

### Not redoing by hand something already done three times

> «This is the third time: do it yourself.»

When a procedure repeats often enough, NOVA turns it into a tool of its own
and from then on it stops rebuilding it one step at a time. The gain is not
theoretical: a request solved by an automation costs two model turns instead
of ten.

**The chain:** recipes (the learned route) -> `automazione_crea` ->
`automazioni_elenco`

### And its own code, too

The archive contains «git tag and push». NOVA works on the project that
contains it: it opens its own sources in the harness, reads them with colours,
proposes changes and applies them when you say so. The **bench**
(`nova/banco.py`) lets it try a repair on a copy before touching the original.

---

### What all these cases have in common

Three things, and they're the same three everywhere:

**If one road doesn't give, it tries another.** And if the right road doesn't
exist, it builds one — an automation, a script, a different way round. It's
written in the prompt as a principle, not as a suggestion.

**It works behind you, not in front of you.** No window jumping to the
foreground, no key pressed in your place, no black console appearing. You can
keep working while it does its part.

**What can't be undone gets written down.** An application sent, a mail gone
out, a file deleted: NOVA doesn't ask permission every time — it asks
according to the autonomy level you chose — but what it did and can't undo
stays written, and you can read it back.

---

## Recipes: how it avoids doing the same work twice

When NOVA solves something non-trivial, it doesn't keep only the result: it
keeps **the route**. Title, steps, and the words you used to ask for it. Next
time, before starting over, it checks whether one of those routes resembles
the new request.

The real problem is «resembles». Comparing strings is useless: nobody asks for
the same thing twice with the same words, and people typing fast write
*inobx*. The solution, borrowed from the work on **engrams** — DeepSeek's and
Qwen's n-gram memory — is that retrieval has to be **cheap**, and the final
choice belongs to the model:

- **Rare words weigh more.** A word that appears in every procedure
  distinguishes nothing; the weight is `1 + N/(1+n)`, a rarity without a
  logarithm. «Mail» is worth little if you have ten procedures about mail;
  «fantasy football» is worth a lot.
- **It measures how much of the question is covered**, not how much the two
  sentences resemble each other. A procedure rich in detail must not lose to a
  poor one just because it has more words: it is **asymmetric containment**,
  not a cosine.
- **Words are compared as trigrams.** «inobx» and «inbox» share almost all
  their three-letter pieces, so they stand for each other. With two guards,
  learned the hard way: same first letter, and lengths that differ by no more
  than one — without them, «recipe» resembled «receipt».
- **It casts a wide net.** The threshold is 0.30 and not 0.42, because one
  candidate too many costs a few hundred tokens, while one missed costs the ten
  turns it takes to rebuild the route from scratch. The recipe block enters the
  prompt as a **note, not an order**: the model is allowed to discard it.
- **There are aliases too**: the other ways of asking for the same thing,
  which the model lists when the procedure is born. They count almost as much
  as the real words — almost, because they are someone else's guess about how
  you will speak.

It is not neural memory and doesn't pretend to be: it is a lexical retrieval
layer that costs microseconds. The idea taken from engrams is not the
architecture, it is the division of labour — **searching must be cheap,
deciding belongs to whoever has the context.**

The archive keeps itself clean: sixty entries at most, duplicates get merged,
the least used ones fall away. An archive that grows forever becomes noise, and
noise makes you propose the wrong route.

---

## The harness: where you study and where you write

It is the most recent part and the least obvious. A document or a project is
not a chat message: they last longer than one turn, and they need to be looked
at while you talk about them. The harness is a window with the document on the
left, the file tree when there is a project, and the conversation on the right
— **the same conversation** as the rest of NOVA, not a second one.

### Documents

| Format | How it opens |
|---|---|
| `.pdf` | the **real pages**, drawn as images, not the extracted text |
| `.docx` | read-only, with the structure |
| `.md` `.txt` | on a white sheet you can write on, with the usual tools |
| `.html` | **rendered**, with Chromium: it's an artifact, you look at what it does |

Asking «where does it talk about entropy» doesn't return a sentence: it
returns a **position** — file and page — and the document scrolls onto it and
highlights it. With a folder open as a project the search covers the whole
stack, which is the real question when the documents are six PDFs for an exam:
not «where is it in this file» but «which file is it in».

### Code

Thirty-two extensions, from Python to Rust to Vue. Code opens on a dark
background, with Pygments colours — five hundred languages, not the four we
would have hand-written — and line numbers, because that's how you name an
error: file and line. An `.html` shows the result, and the source is one click
away: you change it, you save, and the page redraws.

### And NOVA writes inside, but not behind your back

This is the part worth explaining properly, because it is a choice and not a
limitation.

**There is no function that modifies a document.** There is a proposal. It
appears **inside the text**, in its place, wearing its colour: what arrives on
an ember background, what leaves in struck-through grey. You can correct it
where you read it — and what gets applied is what you saw, even if you changed
it in the meantime. You press the button, and before anything is overwritten
an untouched copy stays beside it.

The weaker the model, the more this cycle is worth: a strong model that writes
directly is acceptable, a weak model that writes directly is unmanageable, a
weak model that **proposes** is usable.

On formats it promises nothing it can't keep:

- **`.md`, `.txt`, code**: rewritten in full, no conversion in between. The
  asterisks in a Python comment don't turn into italics.
- **`.docx`**: modified **one paragraph at a time**, and bold, size, style and
  layout stay as whoever wrote it left them. Rebuilding the file from the
  extracted text would have been far easier, and would have thrown away the
  user's work.
- **`.pdf`**: the text **is not rewritten**, and NOVA says so. A PDF doesn't
  contain paragraphs but letters placed at a point on the page. It highlights
  and annotates for real — annotations that stay in the file and open in any
  reader.

What it does **not** do yet: it doesn't run the project's tests and doesn't
apply a change «only if they pass». The verifier is the missing piece, and it
is what would turn the harness from a good place to read into a good place to
program.

## Installation

### Requirements

| | |
|---|---|
| System | **Windows 10/11, 64 bit** |
| Python | 3.10 or newer |
| Disk | 3 GB for the minimum; 15-30 GB if you choose a local model |
| GPU | optional: only needed for the local model |

NOVA is tied to Windows deeply: automation uses UI Automation and the
credential store uses DPAPI. On macOS and Linux the code compiles but does
nothing. **Neither Rust nor Visual Studio is needed**: the core ships already
compiled.

### Steps

```powershell
git clone https://github.com/CastermustOfficial/NOVA.git
cd NOVA
.\install.ps1
```

| Option | What it does |
|---|---|
| `.\install.ps1` | installs everything and sets up autostart |
| `.\install.ps1 -ConCuda` | also downloads llama.cpp CUDA, for the local model |
| `.\install.ps1 -DaSorgente` | builds the core instead of downloading it (needs Rust + MSVC) |
| `.\install.ps1 -SenzaAvvioAuto` | doesn't start at boot |
| `.\install.ps1 -Disinstalla` | removes autostart and the shortcut |

Then launch NOVA from the Desktop shortcut: an orb will appear in a corner of
the screen. Click it to type, or call it by name.

## The brain: who does the thinking

NOVA is not tied to one model, and doesn't expect you to download its own.
Whoever already has one doesn't start from scratch: the installer first looks
at what is on the machine, and only then offers to download.

| Route | For whom | Note |
|---|---|---|
| **API key** | maximum quality, pay per use | OpenAI, OpenRouter, Groq, any compatible endpoint |
| **A subscription you already pay for** | those paying for Claude, ChatGPT, Gemini or Qwen | the installer looks for `claude`, `codex`, `gemini`, `qwen` in the PATH; see the warning below |
| **A model you already have** | anyone with a `.gguf` lying around | the installer looks in LM Studio, Jan, GPT4All, koboldcpp, the HuggingFace cache, Downloads and the Desktop; or you point at the path |
| **A server already running** | those with Ollama or LM Studio up | detected on ports 11434, 1234, 8080, 5001; no key required |
| **I download a model for you** | those starting from zero | Qwen3.8 27B, with the quantisation that fits your VRAM — but you can pick another, and choose which disk it lands on |

None of these is mandatory at install time: you can answer «I'll decide later»
and change your mind from the **Brain** menu, or from `brains.active` in
`config.json`. The recognised CLIs are described in `nova/routing.py`
(`cli_predefinite`): adding one needs no code, just an entry under
`brains.cli`.

A model you point at by hand is actually checked: the first four bytes of a
GGUF are `GGUF`, and an interrupted download doesn't have them. If there is an
`mmproj` projector next to the file, NOVA uses it and the model can see; if
there isn't, the installer tells you instead of letting you find out in a
month.

Changing route later doesn't require reinstalling anything: it's the **Brain**
menu in the interface, or `brains.active` in `config.json`.

> **A warning about subscriptions.** Using a consumer subscription's CLI as
> the engine of a third-party application is outside most providers' terms of
> service, and the risk falls on your account. NOVA supports this route
> because it is convenient, but it is not the default and it is not
> recommended.

The catalogue of local models lives in [`models.json`](models.json): it is
data, not code, so updating the ranking doesn't need a release.

## Permissions

NOVA starts with **«always confirm»**: it asks permission before every action
that touches the system, and the request says *what* it is about to do, not a
generic «allow operation?». You can loosen the constraint when you trust it —
it's your dial, not its decision.

What stays on your disk and never leaves: memory, credentials, configuration.
They live in `%APPDATA%\NOVA`.

## Documentation

- [Architecture document](docs/architettura.md) — the decisions taken and why,
  including the ones that were discarded.
- [How to contribute](CONTRIBUTING.md)

## For developers

```powershell
.\build.ps1              # builds the Rust core (release)
.\build.ps1 -Test        # runs the Rust tests
python -m pytest -q       # runs the Python tests
```

The rest of this document is the detailed technical documentation.

---

## Why Rust, and why a daemon

The right question is not «why Rust» but **why a process that lives in the
system instead of an application you open**. NOVA has to be able to speak
while it isn't open, supervise llama-server, keep the capability registry and
survive the closing of any window. The interfaces — the orb, the harness, the
CLI, the voice, an agentic brain — are thin clients: they can die and restart
without stopping NOVA.

Rust comes second, and it is chosen for three concrete things:

**Because the daemon can't fall over.** It is the only process that has to
stay up always. A memory error in a service that owns the long-running
processes is not an error message, it's an assistant that shuts down while
you're working.

**Because the capabilities you need are the same thing under three names.**
The daemon is built as *one trait, three backends*:

| Needed for | Windows | macOS | Linux |
|---|---|---|---|
| Controlling any app | UI Automation | Accessibility API | AT-SPI2 |
| Observing the whole system | ETW | EndpointSecurity | eBPF |
| Undoing what was done | VSS | APFS snapshots | overlayfs / btrfs |
| Local channel | named pipe | unix socket | unix socket |

The real constraint is portability, not ring 0: these capabilities already
exist in userspace on every system. You don't need an operating system, you
need a process written around that shape.

**Because a binary is a binary.** The daemon is downloaded pre-compiled:
whoever installs NOVA needs neither Rust nor Visual Studio.

What stays in Python is the agent loop, the tools and the memory — where the
ideas change every week and the speed of changing them is worth more than the
speed of running them. The border between the two is deliberate and is written
down in [`core/README.md`](core/README.md).

## Architecture

```
bin/nova-shell.exe    the orb and the windows (started at boot)
install.ps1           installation, CUDA runtime, autostart, shortcut
nova/
  main.py             entrypoint, GUI or CLI
  config.py           persistent configuration (%APPDATA%\NOVA\config.json)
  setup_wizard.py     automatic detection of GGUF model and runtime
  runtime.py          starts/supervises/stops llama-server.exe (+ GPU auto-tuning)
  agent.py            agent loop: model <-> tools, safety, approvals
  processi.py         no NOVA process ever opens a black window
  browser.py          drives Chrome over CDP: paste, tables, uploads
  cerca.py            web search without opening a browser on screen
  ricette.py          the learned procedures, found again even with a typo
  registro.py         what can't be undone gets written down
  pianificazione.py   recurring tasks and sentinels
  fascicolo.py        the true facts about the user: CV, experience, own texts
  harness.py          documents and projects: open, search, point at
  harness_modifica.py propose changes, and apply them only on request
  harness_finestra.py the window: document, tree, chat
  evidenzia.py        code colours (Pygments) and line numbers
  markdown_qt.py      faithful Markdown, there and back
  mcp_kb.py           the 31 tools exposed to an agentic brain
  tools/
    base.py           registry, OpenAI schemas, risk levels
    files.py          read, write, search, move, open
    apps.py           launch apps, list/focus/close windows
    shell.py          PowerShell, CMD, Python
    web.py            web search, page reading, opening in the browser
    system.py         clipboard, keys, volume, notifications, reminders, PC info
    schermo.py        screenshots, when reading the system isn't enough
    automazioni.py    tools NOVA writes itself
    procedure.py      how it solved a request, so it can do it again
    riparazione.py    the bench: it repairs itself without breaking itself
  ui/main_window.py   chat window + action log + tray + hotkey
  voice/              listening and voice: Kokoro, whisper.cpp, ElevenLabs, SAPI
core/crates/
  nova-core/          the daemon: bus, capabilities, long processes, RPC
  nova-voce/          audio, Kokoro, whisper, Scribe: no Python
  nova-shell/         the orb and the windows (Tauri)
```

## Autonomy levels

Settable on the fly from the top-right menu (or in `config.json`):

| Level | Behaviour |
|---|---|
| `always_ask` | confirmation for **every** action, even pure reads |
| `ask_risky` | confirmation only for `DANGEROUS` actions (shell, delete, closing apps, keystrokes) |
| `autonomous` | no confirmation, everything traced in the action log |

Every tool is classified `SAFE` / `MODERATE` / `DANGEROUS`. Beyond autonomy,
two guards always apply and the model cannot get around them:

- `safety.protected_paths` — paths never writable (Windows, Program Files, ...)
- `safety.forbidden_command_patterns` — regexes of blocked commands (format, diskpart, ...)
- `safety.write_roots` — if set, writes are confined to those folders

## Model runtime

At startup NOVA looks for a `llama-server.exe`, in this order:

1. `NOVA\runtime\` (CUDA build downloaded by `get_cuda_runtime.ps1`)
2. the backends already present in `%USERPROFILE%\.lmstudio\extensions\backends`
3. `LLAMA_CPP_HOME`

Then it launches the server as a child process and shuts it down on exit. If
the model doesn't fit in VRAM, `auto_tune_gpu_layers` retries on its own,
scaling down the offloaded layers until it starts.

## Adding a tool

```python
from nova.tools.base import Risk, tool

@tool(
    "invia_email",
    "Invia una email tramite Outlook.",
    {"to": {"type": "string", "description": "Destinatario"},
     "subject": {"type": "string", "description": "Oggetto"},
     "body": {"type": "string", "description": "Testo"}},
    Risk.DANGEROUS, category="mail",
    preview=lambda a: f"Invia email a {a['to']}: {a['subject']}",
)
def invia_email(to: str, subject: str, body: str) -> str:
    ...
    return "Email inviata."
```

Import the module in `nova/tools/__init__.py` and the model sees it
immediately.

## Voice

`nova/voice/` is already in place: `stt.py` (faster-whisper, push-to-talk or
wake word) and `tts.py` (Windows SAPI, zero dependencies). To enable them:

```powershell
pip install faster-whisper sounddevice
```

then `voice.enabled = true` in `config.json`.

## Performance and tuning

NOVA works out on its own how many layers fit in VRAM (`nova/gguf.py` reads
the model's metadata, `estimate_gpu_layers` compares them with free VRAM).
This matters because on Windows, when VRAM runs out, the NVIDIA driver quietly
falls back to shared memory: the model still starts but runs ~10x slower.

Measurements on an RTX 4060 Ti 16 GB with Qwen3.8-27B Q4_K_M (15.7 GB):

| Configuration | Layers on GPU | Generation |
|---|---|---|
| CUDA, `-ngl 99` (VRAM saturated) | 65 | ~2 t/s, prompt 40 t/s |
| Vulkan, auto | 56 | ~8 t/s |
| CUDA, auto (VRAM estimate) | 53 | ~7-9 t/s |

A 27B at Q4 doesn't fit entirely in 16 GB: about 12 layers stay on the CPU and
that is the bottleneck. To go much faster there are two roads, both one line
away in `config.json`:

- a smaller quant of the same model (Q3_K_M ~12.5 GB fits entirely in VRAM:
  3-4x faster, slightly lower quality);
- a smaller model (8-14B) as a "fast brain" for everyday commands, keeping the
  27B for complex tasks.

To force a value by hand: `server.n_gpu_layers` in `config.json` (any value
< 99 disables the automatic estimate).

### Which model to use: the choice that matters most

The catalogue (`models.json`) suggests **Qwen3.8 27B**, and it is a prudent
choice: dense, strong, and on a 16 GB card it **doesn't fit**. The numbers
above are those of a model with twelve layers on the CPU.

There is a road that turns those numbers around, and it is worth explaining
because it isn't obvious: **MoE** models. In a dense 27B model, every token
puts all 27 billion parameters to work. In a mixture-of-experts, only a
fraction lights up per token — the rest sits in memory and stays quiet.

| Model | Total | Active per token | Context | Notes |
|---|---|---|---|---|
| Qwen3.8 27B (in the catalogue) | 27B | 27B — dense | 256K | the strongest, the slowest |
| [Gemma 4 26B-A4B](https://huggingface.co/google/gemma-4-26B-A4B) | 25.2B | **3.8B** | 256K | Apache 2.0, **multimodal**, native function calling |
| [Nemotron 3 Nano 30B-A3B](https://unsloth.ai/docs/models/nemotron-3) | ~30B | **3B** | 1M | hybrid MoE, designed for agentic work |

**The trade-off, said plainly:** on hard reasoning a dense 27B stays ahead.
But a MoE with four billion active parameters *fits entirely in VRAM* on a
16 GB card, and there you don't gain a fraction — you change category. An
assistant that answers in two seconds and is wrong once in twenty is more
useful than one that answers in thirty and is wrong once in twenty-five,
because you never open the second one.

For NOVA in particular, two details of Gemma 4 weigh more than the benchmarks:
it is **multimodal** — so screenshots work with the brain at home too, not
only with the one on the network — and it has **native function calling**,
which is exactly how NOVA talks to its sixty tools.

**How to pick a different one.** `models.json` isn't code, it's data: the best
one changes every month, and if it lived in the code every new model would be
a release. You add a family to the file, or point straight at a `.gguf` you
already have:

```jsonc
// config.json
"server": { "model_path": "D:/modelli/gemma-4-26B-A4B-Q4_K_M.gguf" }
```

And the rule for quantisation is a single one, the same one LM Studio uses:
**pick the largest that FITS, not the largest that will load.** If it doesn't
fit entirely, llama.cpp puts some of the layers in RAM and it still works —
ten times slower, without saying a word.

### If you don't have a graphics card

NOVA runs anyway, and it runs slowly: you go from tens of tokens a second to a
few. Better said up front than discovered later. In that case there are two
sensible roads, and neither is a fallback: **a subscription you already have**
(Claude Code, Codex, Gemini, Qwen — NOVA drives them as brains) or an **API
key**. The local model is a choice about privacy and cost, not the only way.

---

## Memory: a graph knowledge base

NOVA has a long-term memory that survives sessions: a **markdown vault in
`NOVA\vault`, openable in Obsidian as it is** (frontmatter + `[[wikilink]]`,
so Obsidian's graph view works with no plugin).

The retrieval pipeline is the Python port of
`knowledge-lab/backend/src/retrival`:

```
query
  1. exact code bypass         identical slug or tag -> boost
  2a. sparse  (BM25)           title x2.5, tags x2.0
  2b. dense   (embedding)      cosine similarity
  3. RRF fusion (k=60)         a single ordering
  4. filter                    BEFORE the top-K cut, never after
  5. 1-hop graph expansion     the neighbours of the best, re-filtered
  6. cut to top-K
  7. audit                     vault\.nova\audit.jsonl
```

### Structure

```
nova/kb/
  schema.py     node + frontmatter (own parser, like nodeLoader.ts)
  store.py      vault on disk, index, bidirectional relations, dedup, audit
  retrieval.py  BM25 + embedder + RRF + graph expansion + KBEngine
  memory.py     automatic learning from conversations
  seed.py       initial mapping of the PC
nova/kb_setup.py  wiring: vault + engine + memory
nova/tools/kb.py  the 6 tools the model uses to work the memory
```

### The vault

```
vault/
  _INDICE.md          navigation hub, regenerated on every write
  01-profilo/         user profile, preferences
  02-persone/         collaborators (inferred from git co-authors)
  03-progetti/        one node per repo or working folder
  04-ambiente/        hardware, installed apps, models, runtimes
  05-abitudini/
  06-fatti/           everything else
  .nova/audit.jsonl   every search and every write, with a timestamp
```

Every node carries `origine` (`scansione` | `auto` | `utente`) and
`confidenza`: what NOVA inferred is always distinguishable from what you told
it. A fact confirmed a second time raises its own confidence; `utente` always
beats `auto`.

### How it learns

- **Seed**: on the first run it maps profile, projects, environment and
  people.
- **Automatic**: after every exchange a background thread extracts the
  *durable* facts (preferences, projects, people, decisions) and writes them
  down. It does not store one-off requests, command output or timestamps.
- **Explicit**: the `kb_note`, `kb_link`, `kb_forget` tools, for when you say
  "remember that...".
- **Injection**: before every turn the relevant nodes end up in the system
  prompt, so NOVA doesn't ask you again for things it already knows.

### Tools exposed to the model

| Tool | What it does |
|---|---|
| `kb_search` | searches the memory (full hybrid pipeline) |
| `kb_note` | creates or updates a node |
| `kb_link` | links two nodes (undirected graph) |
| `kb_neighbors` | explores a node's links |
| `kb_forget` | archives an outdated node (the file stays on disk) |
| `kb_stats` | nodes, types, links, isolated nodes |

### From the command line

```powershell
python -m nova --kb "orario di lavoro"    # queries the memory
python -m nova --kb-stats                 # state of the KB
python -m nova --seed-kb                  # re-maps the PC (idempotent)
```

### Configuration (`kb` in config.json)

| Key | Default | What it does |
|---|---|---|
| `enabled` | `true` | enables the memory |
| `vault_path` | `NOVA\vault` | where the nodes live |
| `auto_seed` | `true` | initial mapping of the PC |
| `auto_learn` | `true` | automatic writing after every exchange |
| `inject_context` | `true` | context injection before the turn |
| `top_k` | `5` | how many nodes enter the prompt |
| `min_confidence` | `0.25` | below this threshold a node is not used |
| `embedder` | `hash` | `hash` (offline) or `llama` |
| `embedder_url` | `:8421` | a second llama-server with an embedding model |

With `embedder: "llama"` NOVA uses a real embedding model served on another
port (e.g. `nomic-embed-text`), gaining on rephrasings. If it doesn't answer,
it falls back on its own to the local embedder: the KB never breaks because a
server is off.

## A note on reasoning

Qwen3.8 is a *thinking* model: left free it produces 1000+ reasoning tokens per
turn, which at 7 t/s means a two-minute wait. That's why the server starts with
`--reasoning-budget 512`. Raise it in `server.extra_args` if you prefer more
reasoned and slower answers, set it to `0` to disable reasoning entirely.

---

## Three interchangeable brains

Whatever *thinks* sits behind the `nova/brains` abstraction. You switch it hot
from the **Brain** menu at the top, without losing the conversation or the
memory.

| Brain | What it is | Agentic |
|---|---|---|
| `locale` | the GGUF served by llama-server on your PC | no |
| `claude` | Claude Code CLI in headless mode | yes |
| `api` | any OpenAI-compatible endpoint | no |

**Agentic** is the difference that matters. `locale` and `api` *propose* tool
calls and NOVA executes them, applying the guards and the autonomy levels.
`claude` has hands of its own: NOVA acts as intermediary, passes it the
context and the memory, and reports what it did, in how many turns and what it
cost.

```powershell
python -m nova --brains                    # who's there and who's ready
python -m nova --brain claude              # switch and start
python -m nova --brain claude --ask "..."  # a single request
```

### Claude Code as the brain

You need `npm install -g @anthropic-ai/claude-code` and a `claude` that is
already authenticated. NOVA:

- launches it headless (`-p --output-format json`), prompt via stdin
- keeps the session between turns with `--resume <session_id>`
- translates **your** autonomy levels into its permissions:

  | NOVA autonomy | `--permission-mode` |
  |---|---|
  | Always confirm | `plan` (analyses and proposes, touches nothing) |
  | Confirm risky actions | `acceptEdits` |
  | Autonomous | `bypassPermissions` |

- exposes the graph memory to it as an **MCP server** (`nova/mcp_kb.py`), so
  Claude uses `mcp__nova__kb_search` and `mcp__nova__kb_note`: the same
  retrieval pipeline as the local model, the same node format. If the MCP
  doesn't start, it falls back to reading the vault's .md files directly.
- reports cost and tokens for every turn in the action log.

Careful with `brains.claude_model`: on older CLIs the `opus` alias points at
`claude-opus-4-1`, which has been retired and answers 404. The default is
`sonnet`, which works.

### External API

`brains.api_base_url` + `brains.api_model` + a key (in `brains.api_key` or in
the environment variable named by `brains.api_key_env`). It works with OpenAI,
OpenRouter, Groq, Together and anyone speaking the same dialect. It uses
NOVA's tool loop, so the guards and the autonomy stay identical.

### What leaves the PC

With `locale`, nothing, ever. With `claude` and `api`, what leaves is the
request, the conversation context and the memory nodes relevant to the
message: it is the choice that makes those brains useful, but it should be
made knowing what it involves. The selector is there for exactly that: for
sensitive work, go back to `locale`.

---

## The model belongs to the daemon

Since `core/` exists (see `core/README.md`), NOVA no longer spawns
llama-server as a child process: it hands it to **nova-core**, which
supervises it.

```
llama-server pid 2760 -> parent: novad
```

Practical consequences:

| | before | now |
|---|---|---|
| You close the window | the model unloads | it stays loaded |
| You reopen NOVA | ~2 minutes of loading | **2 seconds** |
| The server falls over | it stays down | the daemon brings it back up |
| Model logs | a file nobody reads | `proc.output` events on the bus, plus a ring buffer |

At startup NOVA tries this sequence: *does the daemon already own the model?*
→ it **adopts** it (recovering even how many `-ngl` it started with, by asking
the daemon); *is there something on the port?* → it reuses it; otherwise it
asks nova-core to start it, and only if the daemon is missing does it fall
back to the old child process. If `core/` isn't compiled, NOVA works exactly
as before.

Keys under `server` in `config.json`:

| Key | Default | What it does |
|---|---|---|
| `use_daemon` | `true` | hands the model to nova-core |
| `daemon_autostart` | `true` | starts nova-core if it isn't running |
| `stop_model_on_exit` | `false` | closing NOVA does **not** unload the model |

The window subscribes to `proc.*` and shows the model's logs in the action log
taken from the bus, no longer from a process it owns itself.

---

## Who answers what: the router

The local model **orchestrates**. It's free, it's private, it's already in
VRAM, and to understand what you want and call the right tools it is more than
enough. When a task is beyond it, it doesn't try anyway: it passes the ball
and takes the result back in hand.

The tiers live in `brains.routing.tiers` in `config.json`, in order of power.
The defaults:

| Tier | Brain | Model | When |
|---|---|---|---|
| `locale` | GGUF on the PC | Qwen3.8-27B | orchestration and simple tasks |
| `standard` | Claude Code | `sonnet` | the workhorse |
| `difficile` | Claude Code | `claude-opus-4-5-20251101` | when the task deserves it |
| `alternativo` | Gemini CLI | `gemini-2.5-pro` | second opinion |

```powershell
python -m nova --modelli     # tiers, state, spent / cap
```

### How it passes the ball

Three roads, in order of intelligence:

1. **`delega`** — the model chooses. It writes the task out in full (whoever
   receives it can't see the conversation) and passes the file **paths** in
   `file`: NOVA attaches them, for free. Then it picks the answer back up.
2. **Automatic escalation** — if NOVA fails twice in a row, or makes six calls
   without reaching an answer, it moves up a tier on its own and slots the
   result into the conversation. These are two different ways of not managing:
   hitting a wall, and going round in circles.
3. **`secondo_parere`** — the same question to two tiers, to compare.

### Guards

| Key (`brains.routing`) | Default | What it does |
|---|---|---|
| `orchestratore` | `locale` | who drives the conversation |
| `escalation_automatica` | `true` | moves up on its own when needed |
| `fallimenti_prima_di_salire` | `2` | failed attempts |
| `passi_prima_di_salire` | `6` | calls without an answer |
| `salite_massime` | `1` | how many times per turn |
| `tetto_usd_sessione` | `5.0` | past this, paid delegations stop |
| `solo_locale` | `false` | `true` = nothing leaves the PC, full stop |

### Adding a model without writing code

External agentic CLIs are declared in `brains.cli`; then you name them in a
tier. `{model}` is substituted.

```json
"cli": {
  "deepseek": {
    "etichetta": "DeepSeek",
    "binary": "deepseek",
    "args": ["--model", "{model}"],
    "model": "deepseek-reasoner",
    "prompt": "stdin"
  }
}
```

### Measured numbers

| | time | cost |
|---|---|---|
| `standard` (Sonnet), a plain question | 7.1 s | $0.016 |
| `alternativo` (Gemini), a plain question | 21.7 s | $0 |
| `difficile` (Opus), review of a 300-line file | 112.7 s | **$0.89** |

Opus costs: with the cap at $5 that's five reviews like that one. It is the
reason the orchestrator is the local model and not it.

### What the test taught

In the first version the local model **didn't delegate**: faced with «a severe
architectural critique of this file» it made ten tool calls gathering context
without ever passing the ball. Two corrections:

- the prompt now lists the concrete cases where it should delegate
  *immediately* (judging code, designing, long reasoning, many files at once)
  instead of saying vaguely «if it's beyond you»;
- automatic escalation also looks at the number of steps, not only at the
  failures — because going round in circles is the other way of not managing.

After the corrections, with the same request: it reads the file, announces
«now I'll delegate the critique to a more capable model», picks **`difficile`**
by itself and gives its reason — *«it requires fine reasoning about tokio race
conditions and concurrent correctness; beyond what I can analyse reliably»* —
attaches the files and takes control back with the answer.

## The screenshot is an accessory

There is a `screenshot` tool, and it exists for questions about how things
look («what do you think of this interface?»). **It is not a foundation**: to
*act* on an application NOVA uses the accessibility tree, which is precise,
instant and costs nothing. Giving a model sight so it can press a button is
slow and expensive; having it so it can express a judgement is a bonus.

### A subscription, not an expense

NOVA reads `~/.claude/.credentials.json` and recognises the type of access. On
this PC:

```
accesso: ('abbonamento', 'max_5x')
```

With a subscription, the `total_cost_usd` Claude Code reports is an **API
equivalent**: it says how heavy a request is, not how much you spent. The
dollar cap therefore **does not apply** to tiers covered by a subscription —
it applies only to those paying per token (`brain: "api"`, or a CLI declared
with `"a_consumo": true`).

```
orchestratore: locale   nessun gradino a consumo:
0.0 $ è l'equivalente API, non una spesa

* locale       locale   predefinito                locale       pronto
  standard     claude   sonnet                     abbonamento  pronto
  difficile    claude   claude-opus-4-5-...        abbonamento  pronto
  alternativo  gemini   gemini-2.5-pro             incluso      pronto
```

### When the quota runs out

With a subscription the real constraint isn't money, it's the **usage limits**.
That is a different thing from an error: it doesn't mean «I can't do it», it
means «try again later». NOVA treats it as such:

1. it recognises the quota-exhausted message (`usage limit`, `rate limit`,
   429, …) and raises `LimiteUso`, not a generic error;
2. it puts **that tier on hold** for the time indicated;
3. it **falls back on another provider** — not on another model from the same
   one, because the limit is on the account, not on the model — and as a last
   resort returns to the local one.

```
«difficile» in pausa per 30 minuti: quota esaurita
«difficile» è a quota: ripiego su «alternativo»
esito finale: da «alternativo»
motivo: prova (ripiego: «difficile» a quota)
```

You disable it with `ripiego_su_limite: false`, if you prefer it to stop and
tell you instead of switching model on its own.
