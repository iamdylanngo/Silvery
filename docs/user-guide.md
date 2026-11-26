# Silvery user guide

Silvery records your meetings, turns speech into text on your computer and uses a local AI to
write the notes. Nothing is sent to the internet.

- [Installing Silvery](#installing-silvery)
- [Before you start](#before-you-start)
- [The window](#the-window)
- [Recording a meeting](#recording-a-meeting)
- [Transcript](#transcript)
- [My notes](#my-notes)
- [Summary](#summary)
- [Ask](#ask)
- [Finding, copying and saving meetings](#finding-copying-and-saving-meetings)
- [Menus and keyboard shortcuts](#menus-and-keyboard-shortcuts)
- [Settings](#settings)
- [Where your meetings are kept](#where-your-meetings-are-kept)
- [Troubleshooting](#troubleshooting)

---

## Installing Silvery

Download Silvery from the project's **Releases** page on GitHub (the **Releases** link on the right side of
the project page, or `…/releases/latest`). Open **Assets** under the newest version and pick your file.

### Mac: `…-macOS-AppleSilicon.zip` or `…-macOS-Intel.zip`
Not sure which Mac you have? Apple menu › **About This Mac**. "Chip: Apple M…" means **Apple Silicon**,
and "Processor: … Intel" means **Intel**. Both need macOS 11 or newer.
1. Double-click the zip. It unpacks to **Silvery.app**.
2. Drag **Silvery.app** into your **Applications** folder.
3. Double-click **Silvery**. The first time, macOS may say *"Silvery.app" Not Opened: Apple could not
   verify "Silvery.app" is free of malware…*. That's because Silvery isn't signed with an Apple developer
   certificate yet, not because something is wrong. To open it:
   - press **Done** (not *Move to Bin*);
   - open **System Settings › Privacy & Security** and scroll down to the **Security** section;
   - next to *"Silvery.app" was blocked to protect your Mac*, press **Open Anyway**;
   - press **Open Anyway** again and enter your Mac password.

   macOS remembers this, so after that Silvery opens normally. On macOS 14 or older, right-click
   Silvery › **Open** › **Open** also works.

### Windows: `…-Windows-x64.zip` or `…-Windows-arm64.zip`
Choose **x64** for ordinary Intel/AMD PCs and **arm64** for ARM laptops (for example Snapdragon). Not sure?
Settings › System › About › *System type*.
1. Right-click the zip › **Extract All…**, and put the folder somewhere you like (for example Documents).
2. Double-click **Silvery.exe**. If Windows shows "Windows protected your PC", choose **More info ›
   Run anyway**. This is only needed the first time, because Silvery isn't signed with a Microsoft certificate.
3. If no sound comes in when recording, see [Troubleshooting](#troubleshooting).

### Linux: `…-Linux-x86_64.tar.gz` or `…-Linux-arm64.tar.gz`
Choose **x86_64** for ordinary Intel/AMD PCs and **arm64** for ARM computers (Ubuntu in UTM/Parallels on an
Apple-chip Mac, Raspberry Pi 4/5). Not sure? Run `uname -m`: `x86_64` or `aarch64` (= arm64).
Works on Ubuntu 22.04 or newer, Debian 12 or newer and similar systems.

```sh
tar xzf Silvery-*-Linux-x86_64.tar.gz    # or …-arm64
cd Silvery-*-Linux-x86_64
./install.sh                             # adds Silvery to your applications menu
```

If it complains about missing libraries:
`sudo apt install libasound2t64 libxkbcommon-x11-0 libgl1` (`libasound2` on Ubuntu 22.04).

### Check the download (optional)
Each release has a `SHA256SUMS.txt`. Compare it with the result of `shasum -a 256 <file>` on Mac,
`sha256sum <file>` on Linux, or `Get-FileHash <file>` in Windows PowerShell.

---

## Before you start

**1. Install Ollama (the local AI).** Download it from [ollama.com](https://ollama.com) and open it once.
Then download a model in Terminal (or Command Prompt on Windows):

```sh
ollama pull qwen3:8b
```

| Model | Size | Good for |
|---|---|---|
| `qwen3:4b`, `qwen3.5:4b` | ~3 GB | Fast, fine for most meetings, computers with 8 GB memory |
| `qwen3:8b` | ~5 GB | Better notes, 16 GB memory recommended |
| `gpt-oss`, larger models | 13 GB+ | Best quality, needs a powerful computer |

Silvery picks a model automatically. You can change it in **Settings**. If Ollama isn't running,
Silvery tries to start it.

**2. The speech model.** The first time you record, Silvery downloads its speech-to-text model once
(about 150 MB). You can also do this ahead of time in **Settings › Speech to text › Download now**.

**3. Microphone permission (macOS).** Open Silvery as an app (`Silvery.app`). The first time you record,
macOS asks *"Silvery would like to access the microphone"*. Choose **Allow**.
On Windows, the microphone switch is in Settings › Privacy & security › Microphone.

---

## The window

<img src="images/screenshot-summary.png" width="820" alt="The Silvery window">

- **Left:** your meetings, newest first, grouped by day, with **New meeting**, **Search** and, at the
  bottom, the AI status (green = ready) and **Settings**.
- **Right:** the open meeting, with its title (click to rename), date and length, the **Record** bar and
  four tabs: **Summary**, **Transcript**, **My notes** and **Ask**.

The first time you open Silvery, an example meeting shows what finished notes look like. You can
delete it from the **•••** menu.

---

## Recording a meeting

1. Press **New meeting** (or open an existing meeting to record more).
2. Press **Record**.

<img src="images/screenshot-recording.png" width="820" alt="Recording with the live sound wave">

While recording you see:

- a pulsing **red dot**, the word **Recording** and the **timer**;
- a **live sound wave** of the last few seconds. Grey bars mean quiet, green means a good level,
  and red means too loud. Speak so the bars are mostly green;
- **Pause / Resume**: nothing is recorded while paused;
- **Stop**: ends the recording. The last words are transcribed, and then the summary is written for you
  automatically.

If no sound reaches Silvery for a few seconds, a red message explains what to check
(microphone permission, a muted microphone, or choosing another microphone).

> Tip: Silvery records **your microphone**. In a video call it hears the other people through your
> speakers, so headphones lower their volume in the recording.

---

## Transcript

<img src="images/screenshot-transcript.png" width="820" alt="The transcript tab">

Each line shows the **time** in the meeting and **what was said**. While recording, new text appears
about every 12 seconds. Silvery waits for a pause, so words aren't cut in half. **Copy** puts the whole
transcript on the clipboard.

Recording again in the same meeting **adds** to the transcript.

### Playing back a recording
When a meeting has a recording, the bar next to **Record** shows a **play button**, the time and a
**slider**:
- press **▶** to listen (and again to pause), or drag the slider to move through the meeting;
- in the **Transcript**, the times are blue: **click a time** to hear that moment;
- the line being played is highlighted and stays in view.

All recordings of a meeting play as one, in the same order as the transcript. Playback stops when you
start recording or open another meeting.

---

## My notes

Type your own notes at any time, during or after the meeting. They're saved automatically, and the AI
reads them together with the transcript when it writes the summary, so you can add names, context or
"remember to…" items it should include.

---

## Summary

After you press **Stop**, the summary is written automatically on your computer, usually within
5–30 seconds depending on the model.

- **Style:** choose how the notes are organised:

  | Style | Sections |
  |---|---|
  | General meeting | Overview · Key points · Decisions · Action items |
  | One-on-one | Overview · Updates · Challenges & support · Feedback · Next steps |
  | Stand-up | Done · Today · Blockers · Follow-ups |
  | Customer call | Overview · Needs & pain points · Questions & objections · Next steps |
  | Interview | Overview · Background · Strengths · Concerns · Quotes · Next steps |

- **Write summary / Rewrite summary:** writes it again, for example after you change the style or add notes.
  **Stop writing** stops it partway.
- **Action items** have **checkboxes**: tick them off as you go.
- **Edit:** change the text yourself, then press **Done**.
- **Copy:** copies the summary.
- If the meeting is still **Untitled**, the AI also gives it a short title.

The AI only uses what was said and what you typed. If something is missing, add it in **My notes** and
press **Rewrite summary**.

---

## Ask

Ask questions about **this meeting only**, for example:

- *What did we decide?*
- *What are my action items?*
- *Write a short follow-up email*

Press one of the suggestions or type your own and press **Enter**. If the answer isn't in the meeting,
the AI says so.

---

## Finding, copying and saving meetings

- **Search** (top left) looks through titles, transcripts, notes and summaries.
- The **•••** menu (top right of a meeting) has:
  - **Copy as text:** title, summary, notes and transcript as Markdown;
  - **Save to Downloads:** the same as a `.md` file in your Downloads folder;
  - **Delete meeting:** removes the meeting and its recording (asks first).

---

## Menus and keyboard shortcuts

On a Mac the menus are in the menu bar at the top of the screen. On Windows and Linux they're at the top
of the Silvery window, and the shortcuts use **Ctrl** instead of **⌘**.

| Menu | Item | Mac | Windows / Linux |
|---|---|---|---|
| File | New Meeting | ⌘N | Ctrl+N |
| File | Copy Meeting as Text | ⇧⌘C | Ctrl+Shift+C |
| File | Save to Downloads | ⌘S | Ctrl+S |
| File | Delete Meeting… | ⌘⌫ | Ctrl+Backspace |
| Recording | Record / Stop Recording | ⌘R | Ctrl+R |
| Recording | Pause / Resume Recording | ⇧⌘R | Ctrl+Shift+R |
| Recording | Play / Pause the recording | ⇧⌘P | Ctrl+Shift+P |
| View | Summary · Transcript · My Notes · Ask | ⌘1 – ⌘4 | Ctrl+1 – Ctrl+4 |
| View | Search Meetings | ⌘F | Ctrl+F |
| Help | Silvery User Guide (this guide, inside the app) | ⇧⌘/ | Ctrl+Shift+/ |
| Silvery (Mac) / File | Settings… | ⌘, | Ctrl+, |
| Silvery (Mac) / File | Quit Silvery | ⌘Q | Ctrl+Q |

Items that can't be used right now are greyed out. For example, **Pause Recording** only works while
recording, and **Play Recording** only works when the meeting has a recording.

---

## Settings

<img src="images/screenshot-settings.png" width="820" alt="Settings">

| Setting | What it does |
|---|---|
| **AI model** | Which Ollama model writes summaries and answers. *Check again* reconnects. *Address* is normally `http://localhost:11434`. |
| **Microphone** | Which microphone to record from. *Test microphone* shows the live sound wave without recording. |
| **Speech to text** | *English, fast* (recommended), *English, more accurate*, or *Any language*, with a language choice or automatic detection. Larger models are more accurate but slower. |
| **Show my meeting files** | Opens the folder where Silvery keeps everything. |

---

## Where your meetings are kept

| System | Folder |
|---|---|
| macOS | `~/Library/Application Support/Silvery` |
| Windows | `%APPDATA%\Silvery` |
| Linux | `~/.local/share/Silvery` |

Inside it:

- `meetings/`: one `.json` file per meeting (title, notes, transcript, summary, Ask conversation);
- `recordings/`: the audio of each recording as a `.wav` file (16 kHz mono);
- `models/`: the speech model;
- `settings.json`: your settings.

To back up your meetings, copy this folder. To move to a new computer, copy it to the same place there.

---

## Troubleshooting

**"No sound is coming in" / the sound wave stays flat**
- macOS: open Silvery as an app (`Silvery.app`). If you're running it another way, press **Open Silvery app**.
  Make sure it's allowed in System Settings › Privacy & Security › Microphone, then press Stop and Record again.
- Windows: Settings › Privacy & security › Microphone › turn on *Let desktop apps access your microphone*.
- Check that the microphone isn't muted, or choose another one in **Settings › Microphone**.

**"AI offline — open Ollama"**
Start the Ollama app, then press **Settings › Check again**. If it says no models are installed, run
`ollama pull qwen3:8b`.

**The summary is slow**
Choose a smaller model in Settings (for example `qwen3:4b`). The first summary after starting Ollama is
slower because the model is loading.

**The transcript is slow or misses words**
Speak closer to the microphone (aim for green bars). On a fast computer, try **English, more accurate**.
For other languages, choose **Any language** and set the language.

**macOS says "Silvery.app" Not Opened / Apple could not verify… / unidentified developer**
Press **Done**, then **System Settings › Privacy & Security** › **Open Anyway** next to the Silvery message,
and confirm with your password (see [Installing Silvery](#installing-silvery)). This is only needed the first
time. If the Open Anyway button doesn't appear, open Terminal and run:
`xattr -dr com.apple.quarantine /Applications/Silvery.app` (this removes macOS's "downloaded from the
internet" mark from Silvery), then open Silvery again.

**Linux: "version `GLIBC_2.38' not found"**
That build was made on a newer Linux than yours. Build Silvery on your own machine (see the
[build guide](building.md)) or use a build made for older Linux.

**After updating Silvery, macOS asks for the microphone again**
That's expected for apps without an Apple developer certificate. Choose **Allow** again.
