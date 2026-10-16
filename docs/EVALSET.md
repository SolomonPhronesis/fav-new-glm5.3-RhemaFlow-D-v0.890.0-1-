# Measuring speech accuracy on YOUR clips (evalset)

A number you can trust beats a guess. This takes 20 minutes and tells you which words to fix first.

## 1. Record 10 to 15 clips (20 to 60 seconds each)
From real services or rehearsals, in the same room, with the same microphone:
- the preacher reading or quoting scripture ("turn to Romans chapter eight verse twenty eight")
- the preacher talking normally, with names ("Pastor ...", "Rhapsody of Realities")
- a few with the band playing or the congregation responding
- the commands you use ("next verse", "previous", "zoom in")

Save each as a WAV (any WAV RhemaFlow's tools can read; it is converted to 16 kHz mono).

## 2. Write what was actually said
Next to `clip1.wav` put `clip1.txt` with the exact words. Plain words are fine; capitals and punctuation are ignored.

Optional: `clip1.expect` with the scripture the clip should find, for example `Romans 8:28`.

## 3. Run it
    rhemaflow-d evalset /path/to/folder

You get, per engine (Whisper, Parakeet): overall word error rate, how often the right scripture was found
(and how often it only asked "did you mean?"), the worst clips, and the mix-ups that came back.
It writes `evalset-report.md` and `vocabulary-suggestions.txt` into the folder.

## 4. Use the result
- Words in `vocabulary-suggestions.txt`: add them to the vocabulary, and read them in `rhemaflow-d calibrate`
  so the personal lexicon learns how you are heard.
- Run it again after every change (new model, new microphone position, new vocabulary). If the number goes down,
  keep the change.
