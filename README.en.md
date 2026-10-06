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

## The first five minutes

This document is long. If you came here to work out whether NOVA is for you,
this is the short part.

**You install it** (Windows, three minutes; Python 3.10+ optional):

```powershell
git clone https://github.com/CastermustOfficial/NOVA.git
cd NOVA
.\install.ps1
```

> **Windows will say it doesn't know the publisher.** NOVA's binaries are
> not code-signed: a certificate costs a few hundred euros a year and, for a
> new publisher, **doesn't remove** the warning until it has built up
> reputation. So the choice is stated rather than hidden. What to expect:
> SmartScreen may show "Windows protected your PC" on first launch — "More
> info" > "Run anyway"; and your antivirus may quarantine a freshly
> downloaded executable, in which case the installer tells you so and where
> to restore it. The installer checks the SHA256 hashes published with the
> release before installing anything: that is a check Windows' warning does
> not do, and it is the one that tells you the file is really ours.

**An orb appears** in a corner of the screen. Click it, or call it by name.
The first time, it offers three things to try — these. None of them touches
a file of yours: the first two only read, the third writes itself a note in
memory, which is the point, and it goes away if you tell it to forget.

| Ask it | And you see that |
|---|---|
| «Why is the PC slow?» | it reads memory, processes and disks for real, instead of guessing |
| «What's in the Downloads folder?» | it looks at a folder of yours and tells you about it |
| «Remember that I work better early in the morning» | it writes it into memory, and next time you switch on it still knows |

The third is the one worth waiting for: close everything, open it tomorrow,
and ask it when you work best.

**Two commands that answer without starting anything**, useful before you
trust it and useful if one day NOVA won't start:

```powershell
.\bin\novad --dati          # where it keeps your things, how big they are, what happens if you delete them
.\bin\novad --registro      # what it did that can't be undone
```

If something doesn't work, the rest of the document explains why.


## What it can do

A list of adjectives says nothing. These are the numbers, counted from the
code: **129 tools** for an agentic brain like Claude Code, and **58** for the
model running on your PC and for the APIs, always the same ones, because all
of them together wouldn't fit in its context; **38 file formats** it can open
and show.

### It acts on the system, and doesn't take your seat

Files, applications, windows, PowerShell, clipboard, volume, notifications.
The difference that matters is not *what* it touches but **how**: NOVA acts on
the accessibility tree, not on mouse and keyboard. It can fill in a form in a
background window while you type in another one, and no window jumps to the
front to steal your focus.

That is a rule written into its prompt, not a side effect: *work behind, not
in front*.

### It uses the browser the way you would, but in blocks

NOVA drives **Edge or Chrome** by talking to them over CDP — it looks for Edge
first, which is always there on Windows, then Chrome. **Not Firefox, not
Safari**: they speak a different protocol, and pretending otherwise would be a
promise that breaks on somebody else's PC. Without either, the rest of NOVA
works and the browser commands say they can't find a browser to drive.

It does not simulate keystrokes:
it pastes. Filling five fields of an online spreadsheet costs **one** call
instead of five, and reading a whole table costs one.

| Operation | Measured |
|---|---|
| `web_incolla` (paste) — five rows into three columns | 2.5 ms |
| `web_tabella` (table) — a whole 5x4 table read at once | 1.4 ms |
| `rete_cerca` (search) — searching without opening the browser | 0.46 s |

Measured on the daemon on 29 September with `misure/banco_web_demone.py`,
round trip over the local channel: the median of twenty calls for the first
two, and of five real searches for the last one. The slowest of the five took
3 s: the first one also has to start the search browser. In the Python version
they were 35 ms, 33 ms and about 0.9 s.

The last row is the one that changes the assistant's character: **before
opening a page, NOVA searches**. A browser that opens is a window appearing on
your screen; a search that goes through a faceless browser is not. The daemon
searches like the Python version: with a windowless browser, on a port and a
profile of its own, searching on Bing. If there's neither Edge nor Chrome it
tries DuckDuckGo with a plain request, which however today answers with pages
that have no results. Until 29 September the daemon had only this second road,
and five searches out of five came back empty (D362, D363).

### It remembers, and what it learns stays yours

A graph memory made of `.md` files — openable in Obsidian, versionable in git,
readable without NOVA. It learns durable facts after each exchange, and
**procedures**: how it solved a request, so it doesn't have to work it out
again next time. Procedures are found again even when the request is worded
differently or contains a typo, because the comparison goes through character
tri-grams rather than string equality.

On the disk of the person writing this, on 28 September: 143 notes and 31
learned procedures.

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

`rete_cerca` finds things without opening the browser, `rete_leggi` downloads a
page as text in half a second instead of six, and the documents already on
your disk go into the harness. The difference from having a chat summarise
things for you is that the answer says **where**: file and page, not «I
believe that».

**The chain:** `rete_cerca` -> `rete_leggi` -> `harness_apri` ->
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

**The chain:** `cervelli_stato` (who's available) -> `cervelli_delega` -> the answer comes back
inside the same conversation

### Understanding why the PC is slow

> «Why is it slow?»

It reads the real state instead of guessing: memory, processes, disks, how
many layers of the model are actually in VRAM. On Windows, when VRAM runs out,
the driver silently falls back to shared RAM and the model runs ten times
slower without saying anything — NOVA sees it and says so.

**The chain:** `sys_info` -> `app_processi` -> `shell_exec`

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
(`ripara_apri`, D349) lets it try a repair on a copy, with `cargo test`,
before touching the original: it applies only if no green test turns red.

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
| `.pdf` | the **real pages**, drawn, with selectable text and the yellow notes |
| `.docx` | paragraph by paragraph, and you can write: the rest of the file stays as it was |
| `.md` `.txt` | in the editor, with the preview next to it |
| `.html` | **rendered**: it's an artifact, you look at what it does |

Asking «where does it talk about entropy» doesn't return a sentence: it
returns a **position** — file and page — and the document scrolls onto it and
highlights it. With a folder open as a project the search covers the whole
stack, which is the real question when the documents are six PDFs for an exam:
not «where is it in this file» but «which file is it in».

### Code

Thirty-two extensions, from Python to Rust to Vue. Code opens on a dark
background, in the same editor as Visual Studio Code, with line numbers,
because that's how you name an error: file and line. Below there is a real
terminal. An `.html` shows the result, and the source is one click away: you
change it, you save, and the page redraws.

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

### And on code, it tests before it applies

The verifier is the part that turns the harness from a good place to read into
a place where you program. `harness_prova` works out on its own how a project
is tested — `cargo`, `npm`, `go`, pytest, or the `test_*.py` scripts — and
picks the right suite for the file being touched: running the whole Rust suite
because one line of Python changed is wasted time.

With «Apply and test» the change is written only if the tests don't get worse.
And the comparison is with **before**, not with absolute green: on a real
project some test is nearly always red, and a verifier that demands green
never switches on. What counts is whether something that used to pass now
fails — in that case the file goes back to what it was, the proposal **stays**
(a red test is something to fix, not a reason to start over) and the tests'
output goes back to NOVA, which then knows what to correct.

An exit code of `2` means «this can't be tested here» — it needs the daemon,
it needs a browser — and doesn't count as a failure: counting it would block
every change.

## Installation

### Requirements

| | |
|---|---|
| System | **Windows 10/11, 64 bit** |
| Python | optional, 3.10 or newer: needed for the automations NOVA writes for itself |
| Disk | 3 GB for the minimum; 15-30 GB if you choose a local model |
| GPU | optional: only needed for the local model |

NOVA is tied to Windows deeply: automation uses UI Automation and the
credential store uses DPAPI. On macOS and Linux the daemon runs and does what
doesn't need windows, but the installer exists only for Windows: the table in
«Why Rust, and why a daemon» says what is there and what is missing.
**Neither Rust nor Visual Studio is needed**: the core ships already compiled.

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
`config.json`. `.\bin\nova cli-predefinite` lists the recognised CLIs: adding
one needs no code, just an entry under `brains.cli`, or the settings panel.

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
They live in `%APPDATA%\NOVA` — but «they live in a folder» is not an answer,
so there is a command that gives the whole of it:

```powershell
.\bin\novad --dati                    # what's there, where, how big, and what happens if you delete it
.\bin\novad --registro                # what NOVA did that can't be undone
.\bin\novad --registro fattura        # the actions that mention an invoice
.\bin\novad --registro --giorni 7     # the ones from the last week
```

Both just read the disk: no configuration, no brain running. The moment you
need to know is often the one **before** you trust it enough to start the
rest — or the one where NOVA won't start any more. And no credential's value
appears in either: its name does, and the fact that the store exists.

## Documentation

- [Architecture document](docs/architettura.md) — the decisions taken and why,
  including the ones that were discarded.
- [Work diary](docs/diario.md) — what was found **while** doing it. The
  changelog is `git log`; this keeps the things a commit message doesn't say,
  and that can't be reconstructed six months later.
- [Towards beta](docs/verso_la_beta.md) — what's missing, in three lists, and
  the five sentences that must be true before the word «alpha» comes off.
- [Where I got it wrong](docs/dove_ho_sbagliato.md) — the mistakes of whoever
  writes the code, kept on purpose to be re-read. Not the project's bugs: the
  times I believed something false, and how I found out. *(in Italian)*
- [How to contribute](CONTRIBUTING.md)

At the root there are five folders that act as an index, the same in every
project: [`errori/`](errori/README.md) (mistakes, and the rule each one
leaves), [`piano/`](piano/README.md) (what's left to do, in order),
[`idea/`](idea/README.md) (possible improvements, even purely theoretical
ones), [`test/`](test/README.md) (every test, one by one) and
[`analisi/`](analisi/README.md) (the confirmed decisions). *(in Italian)*

## For developers

```powershell
.\build.ps1              # builds the Rust core (release) and publishes it to bin\
.\build.ps1 -Test        # runs the Rust tests
foreach ($f in Get-ChildItem -Path prove -Recurse -Filter "test_*.py") { python $f.FullName }
```

The Python tests are **programs**, not `pytest` cases: each one runs on its
own, prints what it checks, and ends with an exit code — 0 passed, 1 failed,
**2 "cannot be done here"**. They live in `prove/`, grouped by what they need
in order to run: see [`prove/README.md`](prove/README.md).

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
The daemon is built as *one trait, three backends*: the same question —
«which windows are open?», «put this in the bin» — with one answer per
system. Today only one backend is complete, the Windows one, and the table says
what is really there:

| Needed for | Windows | Linux | macOS |
|---|---|---|---|
| Controlling any app | UI Automation | **missing** (AT-SPI2) | **missing** (Accessibility API) |
| Fencing a command in | container (AppContainer) | Landlock | **missing** |
| Undoing what was done | journal + Recycle Bin | journal + freedesktop trash | journal + `~/.Trash` |
| Keeping credentials | DPAPI | **missing** | **missing** |
| Local channel | named pipe | unix socket | unix socket |

On Linux and macOS the daemon builds, runs, and does what doesn't need
windows: files, memory, brains, web. The core's tests run there too, on every
push, in CI. Observing the whole system (ETW, eBPF) and disk snapshots (VSS,
APFS) were written here as a plan: there is no code for them, and until there
is they stay out of the table.

**Because a binary is a binary.** The daemon is downloaded pre-compiled:
whoever installs NOVA needs neither Rust nor Visual Studio.

**And Python?** NOVA was born in Python, and for months the two halves lived
together: the Rust daemon for the long-running processes, the agent loop, the
tools and the memory in Python. Since September 2026 Rust does everything —
the questions, the tools, the memory, the brains, the installer — and NOVA
works without Python. Every piece was ported with a twin bench that compares
the two versions on the same cases, and `nova/` stays for that: it is what the
tests compare against, and anyone who wants can still use it from a terminal
with `python -m nova`. Python on the PC has one job left: it is the language
NOVA writes its automations in. The journey is told in
[`docs/verso_la_beta.md`](docs/verso_la_beta.md), and the map of the Python
version is in [`nova/README.md`](nova/README.md) *(both in Italian)*.

## Architecture

```
bin/                  the binaries: the orb, the daemon, the command line, the helpers
install.ps1           installation, CUDA runtime, autostart, shortcut
build.ps1             builds the core and publishes it to bin/
core/crates/
  novad/              the daemon: bus, capabilities, long processes, local RPC
  nova-shell/         the orb and the windows (Tauri): chat, settings, harness
  nova-cli/           `nova`: talking to the daemon, and configuring without it; `novaw`, the same without a window
  nova-core/          the engine: capability registry, turn, permissions, journal
  nova-proto/         JSON-RPC over a named pipe or unix socket, shaped like MCP
  nova-mcp/           NOVA as an MCP server for an agentic brain
  nova-mcp-cliente/   NOVA as a client of other people's MCP servers
  nova-platform/      the system: UI Automation, windows, audio, bin, GPU
  nova-ciclo/         the turn: ask, execute, re-read
  nova-contesto/      how much of the conversation fits, and what goes when it doesn't
  nova-scala/         who answers what: tiers, delegations, quota fallbacks
  nova-salita/        when to go up a tier, and when it's going in circles
  nova-cervelli/      what NOVA says to a brain that lives outside: CLI, Claude Code, API
  nova-strumenti/     the tools and the autonomy guards
  nova-nodi/          the memory: one node per .md file, and the rules for learning
  nova-memoria/       BM25, home embedding, fusion, selection
  nova-ricette/       the learned procedures, found again even with a typo
  nova-registro/      searching and telling the action log
  nova-potatura/      no journal grows forever: two MB, one file of history
  nova-modelli/       finding GGUFs and llama-server, and the VRAM sums
  nova-catalogo/      which model makes sense on this machine
  nova-componenti/    what each feature needs, and how to get it
  nova-cartelle/      whether a folder is being cloud-synced by someone else
  nova-voce/          speaking and listening at home: Kokoro, whisper.cpp; ElevenLabs optional
  nova-browser/       the code that runs inside the page
  nova-cdp/           the protocol for talking to Edge and Chrome
  nova-harness/       documents and projects: find a position, propose, test
  nova-docx/          changing a .docx without rebuilding it
  nova-fogli/         spreadsheets: cells, references, lossless writing
  nova-documenti/     reading inside pdf, docx, spreadsheets
  nova-configurazione/ the configuration: the user's file over the factory one
  nova-dati/          where NOVA keeps your things, and what happens if you delete them
  nova-guasti/        a failure said in plain words, and keys that never leak
  nova-calendario/    date arithmetic, with the time passed in from outside
  nova-pianificazione/ «every day at 8», and the Windows task that does it
  nova-pitone/        the habits of Python the port had to respect
  nova-decisioni/     which decisions may leave the PC (CANT-12)
  nova-giudizio/      typed decisions read from the logits (CANT-12)
nova/                 the first version, in Python: what the benches compare against
prove/                the tests, grouped by what they need in order to run
```

Thirty-eight crates. The last two are the ground for CANT-12.
`nova_core::giudizio_casa` (D371) asks the home model for the logits and hands
them to `nova-giudizio`, and the first decision to use it is delegation: which
brain is needed (D373). No binary uses `nova-decisioni`, and the same goes for
`nova-mcp-cliente`.
To train CLM's heads the daemon also keeps its decisions in `decisioni.jsonl`
(D374): the request, the tools used, the tier, the judge's choice. A request
with a credential inside is not written, and `kb.decisioni` turns it all off.
The heads, trained on synthetic tasks labelled by the big model
(`misure/clm_addestra.py`), get between 27 and 29 out of 34 on which brain is
needed and pick the right tool among 58 between 27 and 32 times out of 40
(D375); NOVA does not use them yet. On the same questions Claude Code with
Opus 5 and Gemini 3.1 Pro through Antigravity get 34 out of 34 and 40 out of
40, in seconds rather than milliseconds (`misure/banco_cervelli_fuori.py`).

## Autonomy levels

Settable on the fly from the top-right menu (or in `config.json`):

| Level | Behaviour |
|---|---|
| `always_ask` | confirmation for **every** action, even pure reads |
| `ask_risky` | confirmation only for `DANGEROUS` actions (shell, delete, closing apps, keystrokes) |
| `autonomous` | no confirmation, everything traced in the action log |

Every capability is classified `safe` / `moderate` / `dangerous`. Beyond
autonomy, three guards always apply and the model cannot get around them:

- `safety.protected_paths` — paths never writable (Windows, Program Files, ...)
- `safety.forbidden_command_patterns` — regexes of blocked commands (format, diskpart, ...)
- `safety.write_roots` — if set, writes are confined to those folders

The boundary holds after the command has started. On Linux Landlock keeps it;
on Windows a system container (AppContainer) with a job object, which sees the
folders of `write_roots` for writing, the listed tools read-only, and reads
nothing in the rest of the profile (D367). If the daemon runs as
administrator, sandboxed commands still start without its rights (D369). On
Windows it is also configured in `core.json`, next to `config.json`:

- `tool_roots` — the tool folders (python, node, cargo) that commands may read
  and run, never write. Grant the narrowest folder: `.cargo\bin`, not `.cargo`,
  where the credentials are
- `shell_senza_rete` — turns the network off for commands; by default it is
  on, as on Linux

Four commands are run by hand: `novad --recinto --proponi` says which `PATH`
folders the container cannot read; `--prepara` opens, with the administrator
confirmation Windows asks for, those the user cannot open alone (`C:\Users`
for a project in the profile, `C:\Python313`); `--controlla` says which
third-party folders the container can write because they are open to all
Windows packages, and they cannot be closed for it alone, so every command's
report lists them with the date of the check; and `--togli` removes every
permission written on the folders and the container's profile, at uninstall
too.

## Model runtime

The daemon starts the home model when it's needed: if the first tier of the
ladder is the local model, before answering it checks whether anything
answers at `server.host`/`server.port`. If so, it uses that — LM Studio or
Ollama too. If not, it starts it and waits until it's ready (D358).

`llama-server` is looked for in this order, unless `server.binary` names it:

1. `NOVA\runtime\` (CUDA build downloaded by `get_cuda_runtime.ps1`)
2. the backends already present in `%USERPROFILE%\.lmstudio\extensions\backends`
3. `LLAMA_CPP_HOME` or `LLAMACPP_HOME`

Before starting it, it estimates how many layers fit in video memory, and if
the model doesn't fit it restarts it with six layers fewer, until it starts.
The process belongs to the daemon: closing the window doesn't unload it.

## Adding a capability

A capability is something the daemon can do, with a name, a description for
the model, a risk and a schema of its arguments. The same capability is seen
by the home model, by an agentic brain over MCP, by the chat and by the
command line.

```rust
struct OraCap;

#[async_trait]
impl Capability for OraCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.ora".into(),
            description: "Data e ora correnti del PC.".into(),
            risk: Risk::Safe,
            category: "sys".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        Ok(json!("lunedi 28/09/2026 21:00"))
    }
}
```

It is registered with `reg.add(Arc::new(OraCap))` in the `register` function
of a `caps_*.rs` file in `nova-core`, and the model sees it as `sys_ora`. A
capability that changes something also implements `anteprima`: it's what the
confirmation request shows. Without it, the daemon refuses anyone who asks to
try it «for pretend» instead of actually running it.

## Voice

Everything at home, without Python: the voice is Kokoro (ONNX, with espeak-ng
for pronunciation), listening is whisper.cpp. ElevenLabs is a choice for those
who want it, and if its quota runs out NOVA goes back to the home voice. The
pieces are downloaded when needed:

```powershell
.\bin\nova componenti elenco               # what's there and what's missing
.\bin\nova componenti scarica voce_locale  # Kokoro, and likewise onnx, espeak, ascolto_locale
```

Then it's switched on from the settings panel, under **Voice**. NOVA answers
to its name: there is no dedicated wake-word model, it's whisper transcribing
and the name opening the sentence.

## Performance and tuning

NOVA works out on its own how many layers fit in VRAM (`nova-modelli` reads
the GGUF's metadata and compares them with free VRAM).
This matters because on Windows, when VRAM runs out, the NVIDIA driver quietly
falls back to shared memory: the model still starts but runs ~10x slower.

Measurements on an RTX 4060 Ti 16 GB with Qwen3.8-27B Q4_K_M (15.7 GB), using
the Python version's real prompt: 12,492 tokens of rules and the schemas of
its sixty tools. The ones about flags and layers depend on llama-server, not
on who calls it. The bench is `misure/banco_modello.py`, and it measures them
itself.

The daemon's prompt was measured by `misure/banco_prompt_demone.py` on 29
September, on the same card but with Gemma 4 26B-A4B IQ3_XXS and 30 layers on
the GPU (the model is explained further down). The daemon sends 58 schemas and
the Python version sixty, and for the model they cost almost the same: 11,486 tokens
against 11,685, 5.9 s cold for both, 146 ms warm against 123.

**The first number to look at isn't the speed, it's the gap between cold and
warm:**

| | cold prompt | warm prompt |
|---|---|---|
| first message of a conversation | **25.8 s** | — |
| every one after it | — | **1.5 s** |

Seventeen times less, and it's the prefix cache doing its job: rules and
schemas don't change between turns, so they're processed once. That is why
the memory context and the recipes sit **at the tail of the question** and not
in the system message; moving them «where they belong» would cost
twenty-five seconds per message, silently.

**Then the flags.** Measured, not deduced:

| Configuration | Layers on GPU | Warm prompt | Generation |
|---|---|---|---|
| as before | 53 | 1504 ms | 6.0 t/s |
| `-fa on` | 53 | 1541 ms | 6.1 t/s |
| 8-bit KV | 53 | 1281 ms | 6.5 t/s |
| 8-bit KV, 60 layers | 60 | **691 ms** | **9.0 t/s** |
| 8-bit KV, 62 layers | 62 | 600 ms | 7.7 t/s |
| 8-bit KV, 64 layers | 64 | — | VRAM saturates, collapses |

Two things you couldn't have known by reading. **Flash attention was already
on**: in this build the default is `auto`, and auto means on — setting it by
hand changes nothing. And **the 8-bit KV cache isn't about computing faster**:
it's about taking half the memory, and on a card where the model doesn't fit
entirely, that half becomes layers moving back onto the GPU. That's where the
real gain is, not in the arithmetic.

NOVA now uses the 8-bit KV by default (`server.kv_cache_type`), and the layer
estimate knows the cache is smaller. The estimate stays cautious though — 55
layers instead of the 60 measured — because erring on the high side doesn't
give you an error: it gives you a model that starts and runs ten times slower
without saying so. If you want the 60, set them by hand:

```jsonc
// config.json
"server": { "n_gpu_layers": 60, "kv_cache_type": "q8_0" }
```

And measure again, because free VRAM depends on what else is running:

```powershell
python misure/banco_modello.py            # every configuration
python misure/banco_modello.py kv8-60     # just one
python misure/banco_prompt_demone.py --demone core\target\release\novad.exe
                                          # the daemon's prompt against the Python one
python misure/banco_web_demone.py         # web_incolla, web_tabella and rete_cerca, asked of the running daemon
python misure/banco_taglio.py             # how much shortening the conversation costs
python misure/banco_cervello.py           # can it pick the right tool? (the Python version's tools)
python misure/banco_giudizio_llama.py     # the judgement's letters: n_probs, cache_prompt, /tokenize, how many right (CANT-12; --modello for another GGUF)
python misure/banco_clm.py confronta      # CLM from a GGUF against bf16 (CANT-12; the steps before it are at the top of the file)
python misure/banco_quale_cervello.py     # which brain: today's words against the letters, or CLM with --clm (CANT-12)
python misure/banco_giudizio_slot.py      # does a judgement on the same llama-server cost the conversation's cache?
python misure/banco_strumento_clm.py      # does CLM pick the tool among the home model's 58? Against BM25
python misure/clm_addestra.py compiti     # CLM's heads trained on the big model's choices (CANT-12; the steps are at the top of the file)
python misure/banco_cervelli_fuori.py chiedi --braccio claude   # the same questions to Claude Code and Antigravity, and the cascade with CLM
```

`banco_cervello.py` measures something different from the others: not how fast
a model is, but whether it **can use the tools**. A model can do forty tokens a
second and not know how to call a tool, and then those tokens are useless.
Gemma 4 26B-A4B and Qwen3.8 27B both score 7 out of 8, without inventing tools
and without calling one when an answer in words is enough.

`banco_taglio.py` measures something you don't see: when the conversation gets
long NOVA shortens it, and shortening it throws away the prompt cache. Doing
it at every turn, which is what used to happen, means that from some point on
every answer reprocesses everything from scratch and never recovers. Now it
cuts rarely: three times in sixty turns instead of thirty-one, and the turn
after a cut goes back to costing 231 ms instead of 1,748.

A 27B at Q4 doesn't fit entirely in 16 GB, and five layers on the CPU stay the
bottleneck. To go much faster there are two roads, both one line away in
`config.json`:

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

**And now it is no longer a prediction: it is a measurement.** Same machine
(RTX 4060 Ti, 16 GB), same configuration, same real 12,000-token prompt, the
two models back to back in the same session:

| | layers on GPU | cold prompt | warm prompt | generation |
|---|---|---|---|---|
| Qwen3.8 27B Q4_K_M (15.7 GB) | 53 of 65 | 26.5 s | 1,363 ms | **6.0 tok/s** |
| Gemma 4 26B-A4B Q3_K_XL (12.0 GB) | **30 of 30** | 6.1 s | **145 ms** | **42.4 tok/s** |

**Seven times** on generation, nine on the warm prompt. The column that
explains all the others is the first one: 30 of 30 against 53 of 65. It isn't
the MoE doing the magic — it's that one of them fits and the other doesn't,
and the twelve layers Qwen leaves in RAM cost more than everything else put
together. What the MoE buys is that "fits": 3.8 billion active parameters
instead of 27 are what let a 26B model live in twelve gigabytes without
becoming useless.

Two honest warnings about that table. The quantisations are not matched
(Q3_K_XL against Q4_K_M): that is deliberate, because the rule is to pick
**the largest one that fits**, and that is precisely the choice being
measured. And what is being measured is **speed**, not answer quality: that
one doesn't yield to a stopwatch, and on hard reasoning the dense model stays
ahead.

For NOVA in particular, two details of Gemma 4 weigh more than the benchmarks:
it is **multimodal** — so screenshots work with the brain at home too, not
only with the one on the network — and it has **native function calling**,
which is exactly how NOVA talks to its tools.

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

This too was written and never measured, and measuring it showed that the
answer depends almost entirely on **which** model, not on how fast the
processor is. Same machine, same prompt, zero layers on the GPU:

| on CPU alone | warm prompt | generation |
|---|---|---|
| Gemma 4 26B-A4B (MoE, 3.8B active) | 1.3 s | **7.6 tok/s** |
| Qwen3.8 27B (dense) | 5.7 s | **1.8 tok/s** |

**Four and a half times between two models of the same size**, because on the
processor you pay for the parameters that switch on, not for the ones that
exist. And the number that surprises most is another one: the MoE **on CPU**
(7.6 tok/s) beats the dense model **on the GPU** (6.0 tok/s) of this machine.

So, said honestly: 7.6 tokens a second is faster than a person reads, and NOVA
without a graphics card **is usable**. At 1.8 an eighty-token answer takes
forty-five seconds, and it isn't. If you have no GPU, the choice that matters
isn't local versus not: it's **pick a MoE**.

Two roads remain, and neither is a fallback: **a subscription you already
have** (Claude Code, Codex, Gemini, Qwen — NOVA drives them as brains) or an
**API key**. The local model is a choice about privacy and cost, not the only
way.

---

## Memory: a graph knowledge base

NOVA has a long-term memory that survives sessions: a **markdown vault in
`NOVA\vault`, openable in Obsidian as it is** (frontmatter + `[[wikilink]]`,
so Obsidian's graph view works with no plugin).

The retrieval pipeline comes from `knowledge-lab/backend/src/retrival`, and
lives in `nova-memoria`:

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
core/crates/
  nova-nodi/      node + frontmatter, the vault on disk, the secrets guard
  nova-memoria/   BM25 + home embedding + RRF + selection
  nova-ricette/   the learned procedures
  nova-core/      memoria.rs (search inside the turn), imparare.rs (durable
                  facts), caps_memoria.rs (the kb_* tools)
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

- **Seed**: the first time it starts, the daemon creates the vault and writes
  into it what it finds on the PC: the profile (with the git name, no email),
  the preferences in the chosen language, the environment, the apps and the
  projects in the user's folders. Not people. `novad --semina` does it again
  by hand.
- **Automatic**: after every exchange a background queue extracts the
  *durable* facts (preferences, projects, people, decisions) and writes them
  down. It does not store one-off requests, command output or timestamps.
  It doesn't learn from a turn that looked at the screen.
- **Explicit**: the `kb_nota`, `kb_collega`, `kb_dimentica` tools, for when you
  say "remember that...". Text that looks like a credential doesn't get in.
- **Injection**: before every turn the relevant nodes end up **at the end of
  the question**, not in the system prompt: that keeps the prompt cache good,
  and NOVA doesn't ask you again for things it already knows.

### Tools exposed to the model

| Tool | What it does |
|---|---|
| `kb_cerca` | searches the memory (full hybrid pipeline) |
| `kb_nota` | creates or updates a node |
| `kb_collega` | links two nodes (undirected graph) |
| `kb_vicini` | explores a node's links |
| `kb_dimentica` | archives an outdated node (the file stays on disk) |
| `kb_stato` | nodes, types, links, isolated nodes |
| `kb_procedure` | the learned procedures |
| `kb_procedura_dimentica` | removes a wrong procedure |

### From the command line

```powershell
.\bin\nova call kb.cerca query="orario di lavoro"    # queries the memory
.\bin\nova call kb.stato                             # state of the memory
```

### Configuration (`kb` in config.json)

| Key | Default | What it does |
|---|---|---|
| `enabled` | `true` | enables the memory: when off, no vault is created, nothing is searched, written or learned |
| `vault_path` | `NOVA\vault` | where the nodes live |
| `auto_seed` | `true` | initial mapping of the PC, the first time the daemon starts |
| `auto_learn` | `true` | automatic writing after every exchange |
| `inject_context` | `true` | context injection before the turn |
| `top_k` | `5` | how many nodes enter the prompt |
| `min_confidence` | `0.25` | below this threshold a node is not used |
| `embedder` | `hash` | `hash`, the home embedding: offline, no models |

The home embedding doesn't understand synonyms: it's word hashing, and BM25
does the real work. The Python version could also ask an embedding model on
another port for vectors (`embedder: "llama"`); in the daemon that route is
**missing**, and a configuration asking for it uses the home one.

## A note on reasoning

Qwen3.8 is a *thinking* model: left free it produces 1000+ reasoning tokens per
turn, which at 7 t/s means a two-minute wait. That's why the server starts with
`--reasoning-budget 512`. Raise it in `server.extra_args` if you prefer more
reasoned and slower answers, set it to `0` to disable reasoning entirely.

---

## Three interchangeable brains

Whatever *thinks* sits behind `nova-cervelli`. You switch it hot from the
**Brain** menu, without losing the conversation or the memory.

| Brain | What it is | Agentic |
|---|---|---|
| `locale` | the GGUF served by llama-server on your PC | no |
| `claude` | Claude Code CLI in headless mode | yes |
| a CLI | Codex, Gemini, Qwen, or one declared in `brains.cli` | yes |
| `api` | any OpenAI-compatible endpoint | no |

**Agentic** is the difference that matters. `locale` and `api` *propose* tool
calls and NOVA executes them, applying the guards and the autonomy levels.
`claude` has hands of its own: NOVA acts as intermediary, passes it the
context and the memory, and reports what it did, in how many turns and what it
cost.

```powershell
.\bin\nova call cervelli.stato    # who's there and who's ready
.\bin\nova chiedi "..."           # a single request, through the daemon
```

### Claude Code as the brain

You need `npm install -g @anthropic-ai/claude-code` and a `claude` that is
already authenticated. NOVA:

- launches it headless (`-p --output-format json`): the question goes through
  standard input and the system prompt through a file, because on Windows a
  command line stops at 8191 characters
- keeps the session between turns with `--resume <session_id>`
- translates **your** autonomy levels into its permissions:

  | NOVA autonomy | `--permission-mode` |
  |---|---|
  | Always confirm | `default` (asks for every action, through NOVA's counter) |
  | Confirm risky actions | `acceptEdits` |
  | Autonomous | `bypassPermissions` |

- connects it to the daemon as an **MCP server** (`nova mcp`, a bridge between
  standard input and the daemon's channel): Claude sees all 129 tools, from
  `mcp__nova-core__kb_cerca` down, with the same guards as the home model,
  which gets 58 because they don't all fit in its context.
  Confirmations go through NOVA's counter (`--permission-prompt-tool`), that is
  the button in the chat.
- reports cost and tokens for every turn in the action log.

Careful with `brains.claude_model`: on an older CLI an alias can point at a
retired model, which answers 404. When in doubt, write the full name.

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

llama-server isn't a process of the window: it belongs to the daemon, which
starts it when needed and supervises it.

```
llama-server pid 2760 -> parent: novad
```

Practical consequences:

| | before | now |
|---|---|---|
| You close the window | the model unloads | it stays loaded |
| You reopen NOVA | ~2 minutes of loading | **2 seconds** |
| The server falls over | it stays down | it comes back at the next question |
| Model logs | a file nobody reads | `proc.output` events on the bus, plus a ring buffer |

Before a turn that starts from the home model, the daemon tries this
sequence: *does anything already answer on the port?* → it uses it, its own or
someone else's; *is it its own and still loading?* → it waits; otherwise it
starts it (D358).

Keys under `server` in `config.json`:

| Key | Default | What it does |
|---|---|---|
| `autostart_model` | `true` | starts the model when a turn needs it |
| `binary` | empty | which llama-server; empty = the best one found |
| `n_gpu_layers` | `999` | below 99 it's your choice, from 99 up NOVA estimates it |
| `startup_timeout` | `600` | how long to wait for loading, in seconds |

The window subscribes to `proc.*` and gets the model's logs from the bus.

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
| `standard` | Claude Code | `claude-sonnet-5` | the workhorse |
| `difficile` | Claude Code | `claude-opus-5` | when the task deserves it |
| `alternativo` | Gemini CLI | `gemini-2.5-pro` | second opinion |

```powershell
.\bin\nova call cervelli.stato     # tiers, state, spent / cap
```

### How it passes the ball

Three roads, in order of intelligence:

1. **`cervelli_delega`** — the model chooses. It writes the task out in full (whoever
   receives it can't see the conversation) and passes the file **paths** in
   `file`: NOVA attaches them, for free. Then it picks the answer back up.
2. **Automatic escalation** — if NOVA fails twice in a row, or makes four calls
   without reaching an answer, it moves up a tier on its own and slots the
   result into the conversation. These are two different ways of not managing:
   hitting a wall, and going round in circles.
3. **`cervelli_secondo_parere`** — the same question to two tiers, to compare.

### Guards

| Key (`brains.routing`) | Default | What it does |
|---|---|---|
| `orchestratore` | `locale` | who drives the conversation |
| `escalation_automatica` | `true` | moves up on its own when needed |
| `fallimenti_prima_di_salire` | `2` | failed attempts |
| `passi_prima_di_salire` | `4` | calls without an answer |
| `salite_massime` | `2` | how many times per turn |
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

There is a tool, `schermo_cattura`, and it exists for questions about how things
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
   429, …) as «not now», not as a generic error;
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
